use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::config::{AppPaths, ResolvedConfig, append_log, config_modified, load_config};
use crate::sync_engine::{
    SyncPhase, SyncProgress, SyncReport, clear_shadow_cache, run_sync_to_usb_with_progress,
    run_sync_with_progress,
};
use crate::watcher::{ChangeWatcher, WatchKind};
use crate::wizard;
use crate::platform;

pub fn run() -> Result<()> {
    let paths = AppPaths::discover()?;
    paths.ensure_layout()?;

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .context("failed to create the application event loop")?;
    let proxy = event_loop.create_proxy();

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    let tick_interval_seconds = Arc::new(AtomicU64::new(2));
    spawn_tick_thread(proxy.clone(), tick_interval_seconds.clone());

    let mut app = App::new(paths, proxy, tick_interval_seconds);
    event_loop.run_app(&mut app).context("event loop failed")
}

fn spawn_tick_thread(proxy: EventLoopProxy<UserEvent>, tick_interval_seconds: Arc<AtomicU64>) {
    thread::spawn(move || {
        loop {
            let seconds = tick_interval_seconds.load(Ordering::Relaxed).max(1);
            thread::sleep(Duration::from_secs(seconds));
            if proxy.send_event(UserEvent::Tick).is_err() {
                break;
            }
        }
    });
}

#[derive(Debug)]
enum UserEvent {
    Tick,
    Menu(MenuEvent),
    SyncProgress(SyncProgress),
    SyncFinished(Result<SyncReport, String>),
    WatchedChange(WatchKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncDirection {
    FromUsb,
    ToUsb,
}

impl SyncDirection {
    fn syncing_text(self) -> &'static str {
        match self {
            Self::FromUsb => "Syncing from USB",
            Self::ToUsb => "Syncing to USB",
        }
    }

    fn last_sync_text(self) -> &'static str {
        match self {
            Self::FromUsb => "Last sync from USB",
            Self::ToUsb => "Last sync to USB",
        }
    }

    fn idle_watch_text(self) -> &'static str {
        match self {
            Self::FromUsb => "Watching USB for changes",
            Self::ToUsb => "Watching local folders for USB upload",
        }
    }

    fn failure_text(self, background: bool) -> &'static str {
        match (self, background) {
            (Self::FromUsb, true) => "Background sync from USB failed",
            (Self::FromUsb, false) => "Sync from USB failed",
            (Self::ToUsb, true) => "Background sync to USB failed",
            (Self::ToUsb, false) => "Sync to USB failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncTrigger {
    Manual,
    Insert,
    Watch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveSync {
    direction: SyncDirection,
    trigger: SyncTrigger,
}

struct App {
    paths: AppPaths,
    proxy: EventLoopProxy<UserEvent>,
    tray: Option<TrayIcon>,
    menu: Option<AppMenu>,
    config: Option<ResolvedConfig>,
    config_error: Option<String>,
    config_stamp: Option<std::time::SystemTime>,
    auto_opened_error_key: Option<String>,
    drive_present: bool,
    has_auto_synced_this_mount: bool,
    syncing: bool,
    active_sync: Option<ActiveSync>,
    sync_progress: Option<SyncProgress>,
    last_status: String,
    tick_interval_seconds: Arc<AtomicU64>,
    watcher: Option<ChangeWatcher>,
    watcher_key: Option<String>,
    pending_pull_sync: bool,
    pending_push_sync: bool,
}

impl App {
    fn new(
        paths: AppPaths,
        proxy: EventLoopProxy<UserEvent>,
        tick_interval_seconds: Arc<AtomicU64>,
    ) -> Self {
        Self {
            paths,
            proxy,
            tray: None,
            menu: None,
            config: None,
            config_error: None,
            config_stamp: None,
            auto_opened_error_key: None,
            drive_present: false,
            has_auto_synced_this_mount: false,
            syncing: false,
            active_sync: None,
            sync_progress: None,
            last_status: "Starting up".to_string(),
            tick_interval_seconds,
            watcher: None,
            watcher_key: None,
            pending_pull_sync: false,
            pending_push_sync: false,
        }
    }

    fn initialize(&mut self) {
        self.reload_config(true);
        self.update_drive_presence();
        self.maybe_auto_sync();
        self.refresh_watchers();
        self.update_ui();
    }

    fn reload_config(&mut self, force: bool) {
        let current_stamp = config_modified(&self.paths);
        if !force && current_stamp == self.config_stamp {
            return;
        }

        self.config_stamp = current_stamp;
        match load_config(&self.paths) {
            Ok(config) => {
                self.tick_interval_seconds
                    .store(config.app.poll_interval_seconds, Ordering::Relaxed);
                append_log(&self.paths, format!("Loaded config: {config}"));
                self.config = Some(config);
                self.config_error = None;
                self.auto_opened_error_key = None;
                if !self.syncing {
                    self.last_status = "Config loaded".to_string();
                }
            }
            Err(error) => {
                let message = error.to_string();
                self.tick_interval_seconds.store(2, Ordering::Relaxed);
                append_log(&self.paths, format!("Config error: {message}"));
                self.config = None;
                self.config_error = Some(message.clone());
                self.last_status = format!("Config error: {message}");
                self.watcher = None;
                self.watcher_key = None;
                self.maybe_open_wizard_for_config_error(current_stamp, &message);
            }
        }
    }

    fn maybe_open_wizard_for_config_error(
        &mut self,
        current_stamp: Option<std::time::SystemTime>,
        message: &str,
    ) {
        let error_key = format!("{current_stamp:?}|{message}");
        if self.auto_opened_error_key.as_deref() == Some(error_key.as_str()) {
            return;
        }

        match wizard::prepare_recovery_context(&self.paths, message)
            .and_then(|context| wizard::open_setup_wizard_with_context(&self.paths, &context))
        {
            Ok(()) => {
                self.auto_opened_error_key = Some(error_key);
                self.config_stamp = None;
                self.last_status = "Config issue detected. Setup Wizard opened.".to_string();
            }
            Err(error) => {
                let launch_message = format!("Setup wizard auto-open failed: {error}");
                append_log(&self.paths, &launch_message);
                self.last_status = launch_message;
            }
        }
    }

    fn update_drive_presence(&mut self) {
        let Some(config) = self.config.as_ref() else {
            self.drive_present = false;
            self.has_auto_synced_this_mount = false;
            self.pending_pull_sync = false;
            self.pending_push_sync = false;
            self.watcher = None;
            self.watcher_key = None;
            return;
        };

        let was_present = self.drive_present;
        let is_present = platform::drive_present(&config.drive_root);
        self.drive_present = is_present;

        if is_present && !was_present {
            self.has_auto_synced_this_mount = false;
            self.pending_pull_sync = false;
            self.pending_push_sync = false;
            if !self.syncing {
                self.last_status = format!("Drive {} detected", config.drive_label);
            }
        }

        if !is_present && was_present {
            if config.cache.shadow_copy && config.cache.clear_shadow_on_eject {
                if let Err(error) = clear_shadow_cache(config) {
                    append_log(&self.paths, format!("Cache cleanup error: {error}"));
                }
            }
            self.has_auto_synced_this_mount = false;
            self.pending_pull_sync = false;
            self.pending_push_sync = false;
            self.watcher = None;
            self.watcher_key = None;
            if !self.syncing {
                self.last_status = format!("Drive {} removed", config.drive_label);
            }
        }
    }

    fn maybe_auto_sync(&mut self) {
        let Some(config) = self.config.as_ref() else {
            return;
        };
        if self.syncing || !self.drive_present {
            return;
        }

        if config.app.sync_on_insert && !self.has_auto_synced_this_mount {
            if self.start_sync(SyncDirection::FromUsb, SyncTrigger::Insert) {
                self.has_auto_synced_this_mount = true;
            }
        }
    }

    fn start_sync(&mut self, direction: SyncDirection, trigger: SyncTrigger) -> bool {
        if self.syncing {
            return false;
        }

        let Some(config) = self.config.clone() else {
            self.last_status = "Sync blocked: fix config.json first".to_string();
            self.update_ui();
            return false;
        };

        if !platform::drive_present(&config.drive_root) {
            self.last_status = format!("Drive {} is not mounted", config.drive_label);
            self.update_ui();
            return false;
        }

        self.syncing = true;
        self.active_sync = Some(ActiveSync { direction, trigger });
        self.sync_progress = None;
        self.watcher = None;
        self.watcher_key = None;
        self.last_status = format!("{} {} job(s)...", direction.syncing_text(), config.jobs.len());
        self.update_ui();

        if !matches!(trigger, SyncTrigger::Watch) {
            append_log(&self.paths, self.last_status.clone());
        }

        let paths = self.paths.clone();
        let proxy = self.proxy.clone();
        thread::spawn(move || {
            let result = match direction {
                SyncDirection::FromUsb => {
                    run_sync_with_progress(&config, &paths, |snapshot| {
                        let _ = proxy.send_event(UserEvent::SyncProgress(snapshot));
                    })
                }
                SyncDirection::ToUsb => {
                    run_sync_to_usb_with_progress(&config, &paths, |snapshot| {
                        let _ = proxy.send_event(UserEvent::SyncProgress(snapshot));
                    })
                }
            }
            .map_err(|error| error.to_string());
            let _ = proxy.send_event(UserEvent::SyncFinished(result));
        });
        true
    }

    fn handle_watched_change(&mut self, kind: WatchKind) {
        if !self.drive_present {
            return;
        }

        let Some(config) = self.config.as_ref() else {
            return;
        };

        match kind {
            WatchKind::UsbSource => {
                if !config.app.sync_while_mounted {
                    return;
                }
                if self.syncing {
                    self.pending_pull_sync = true;
                } else {
                    let _ = self.start_sync(SyncDirection::FromUsb, SyncTrigger::Watch);
                }
            }
            WatchKind::LocalTarget => {
                if !config.app.auto_sync_to_usb {
                    return;
                }
                if self.syncing {
                    self.pending_push_sync = true;
                } else {
                    let _ = self.start_sync(SyncDirection::ToUsb, SyncTrigger::Watch);
                }
            }
        }
    }

    fn maybe_run_pending_syncs(&mut self) -> bool {
        let Some(config) = self.config.as_ref() else {
            self.pending_pull_sync = false;
            self.pending_push_sync = false;
            return false;
        };
        if self.syncing || !self.drive_present {
            return false;
        }

        if self.pending_pull_sync && config.app.sync_while_mounted {
            self.pending_pull_sync = false;
            return self.start_sync(SyncDirection::FromUsb, SyncTrigger::Watch);
        }

        if self.pending_push_sync && config.app.auto_sync_to_usb {
            self.pending_push_sync = false;
            return self.start_sync(SyncDirection::ToUsb, SyncTrigger::Watch);
        }

        false
    }

    fn refresh_watchers(&mut self) {
        if self.syncing {
            self.watcher = None;
            self.watcher_key = None;
            return;
        }

        let Some(config) = self.config.as_ref() else {
            self.watcher = None;
            self.watcher_key = None;
            return;
        };

        if !self.drive_present {
            self.watcher = None;
            self.watcher_key = None;
            return;
        }

        let watch_usb = config.app.sync_while_mounted;
        let watch_local = config.app.auto_sync_to_usb;
        if !watch_usb && !watch_local {
            self.watcher = None;
            self.watcher_key = None;
            return;
        }

        let watcher_key = self.build_watcher_key(config, watch_usb, watch_local);
        if self.watcher.is_some() && self.watcher_key.as_deref() == Some(watcher_key.as_str()) {
            return;
        }

        let proxy = self.proxy.clone();
        match ChangeWatcher::new(config, watch_usb, watch_local, move |kind| {
            let _ = proxy.send_event(UserEvent::WatchedChange(kind));
        }) {
            Ok(watcher) => {
                self.watcher = if watcher.is_active() { Some(watcher) } else { None };
                self.watcher_key = Some(watcher_key);
                if !self.syncing
                    && let Some(summary) = self.watch_summary_text()
                    && (self.last_status == "Config loaded"
                        || self.last_status.starts_with("Drive ")
                        || self.last_status.starts_with("Watching "))
                {
                    self.last_status = summary;
                }
            }
            Err(error) => {
                self.watcher = None;
                self.watcher_key = None;
                let message = format!("Watch setup failed: {error}");
                append_log(&self.paths, &message);
                if !self.syncing {
                    self.last_status = message;
                }
            }
        }
    }

    fn build_watcher_key(
        &self,
        config: &ResolvedConfig,
        watch_usb: bool,
        watch_local: bool,
    ) -> String {
        let mut key = format!(
            "{}|{}|{}",
            config.drive_label,
            if watch_usb { 1 } else { 0 },
            if watch_local { 1 } else { 0 }
        );

        for job in &config.jobs {
            key.push('|');
            key.push_str(&job.name);
            key.push('|');
            key.push_str(&job.usb_source_root(&config.drive_root).display().to_string());
            key.push('|');
            key.push_str(&job.local_target.display().to_string());
        }

        key
    }

    fn watch_summary_text(&self) -> Option<String> {
        let config = self.config.as_ref()?;
        if !self.drive_present {
            return None;
        }

        match (config.app.sync_while_mounted, config.app.auto_sync_to_usb) {
            (true, true) => Some("Watching USB and local folders".to_string()),
            (true, false) => Some(SyncDirection::FromUsb.idle_watch_text().to_string()),
            (false, true) => Some(SyncDirection::ToUsb.idle_watch_text().to_string()),
            (false, false) => None,
        }
    }

    fn eject_now(&mut self) {
        if self.syncing {
            self.last_status = "Cannot eject while a sync is running".to_string();
            self.update_ui();
            return;
        }

        let Some(config) = self.config.as_ref() else {
            self.last_status = "Eject blocked: fix config.json first".to_string();
            self.update_ui();
            return;
        };

        match platform::eject_drive(&config.drive_root) {
            Ok(()) => {
                self.last_status = format!("Drive {} ejected", config.drive_label);
                if config.cache.shadow_copy && config.cache.clear_shadow_on_eject {
                    if let Err(error) = clear_shadow_cache(config) {
                        append_log(&self.paths, format!("Cache cleanup error: {error}"));
                    }
                }
            }
            Err(error) => {
                self.last_status = format!("Eject failed: {error}");
                append_log(&self.paths, &self.last_status);
            }
        }
        self.update_drive_presence();
        self.refresh_watchers();
        self.update_ui();
    }

    fn open_config(&mut self) {
        if let Err(error) = platform::open_path(&self.paths.config_file) {
            self.last_status = format!("Open config failed: {error}");
            append_log(&self.paths, &self.last_status);
            self.update_ui();
        }
    }

    fn open_drive_root(&mut self) {
        let Some(config) = self.config.as_ref() else {
            self.last_status = "Open drive blocked: fix config.json first".to_string();
            self.update_ui();
            return;
        };

        if let Err(error) = platform::open_in_file_manager(&config.drive_root) {
            self.last_status = format!("Open drive failed: {error}");
            append_log(&self.paths, &self.last_status);
            self.update_ui();
        }
    }

    fn open_shadow_cache(&mut self) {
        let Some(config) = self.config.as_ref() else {
            self.last_status = "Open shadow cache blocked: fix config.json first".to_string();
            self.update_ui();
            return;
        };

        if let Err(error) = platform::open_in_file_manager(&config.cache.shadow_root) {
            self.last_status = format!("Open shadow cache failed: {error}");
            append_log(&self.paths, &self.last_status);
            self.update_ui();
        }
    }

    fn open_log(&mut self) {
        if let Err(error) = platform::open_path(&self.paths.log_file) {
            self.last_status = format!("Open log failed: {error}");
            append_log(&self.paths, &self.last_status);
            self.update_ui();
        }
    }

    fn open_setup_wizard(&mut self) {
        if let Err(error) = wizard::open_setup_wizard(&self.paths) {
            self.last_status = format!("Setup wizard failed: {error}");
            append_log(&self.paths, &self.last_status);
            self.update_ui();
        }
    }

    fn open_app_folder(&mut self) {
        if let Err(error) = platform::open_in_file_manager(&self.paths.app_dir) {
            self.last_status = format!("Open app folder failed: {error}");
            append_log(&self.paths, &self.last_status);
            self.update_ui();
        }
    }

    fn handle_menu_event(&mut self, event_loop: &ActiveEventLoop, event: MenuEvent) {
        let Some(menu) = self.menu.as_ref() else {
            return;
        };

        if event.id == *menu.sync_from_usb_now.id() {
            let _ = self.start_sync(SyncDirection::FromUsb, SyncTrigger::Manual);
        } else if event.id == *menu.sync_to_usb_now.id() {
            let _ = self.start_sync(SyncDirection::ToUsb, SyncTrigger::Manual);
        } else if event.id == *menu.eject_now.id() {
            self.eject_now();
        } else if event.id == *menu.setup_wizard.id() {
            self.open_setup_wizard();
        } else if event.id == *menu.open_drive.id() {
            self.open_drive_root();
        } else if event.id == *menu.open_shadow.id() {
            self.open_shadow_cache();
        } else if event.id == *menu.open_config.id() {
            self.open_config();
        } else if event.id == *menu.open_log.id() {
            self.open_log();
        } else if event.id == *menu.open_folder.id() {
            self.open_app_folder();
        } else if event.id == *menu.quit.id() {
            event_loop.exit();
        }
    }

    fn handle_sync_progress(&mut self, progress: SyncProgress) {
        self.sync_progress = Some(progress);
        self.update_ui();
    }

    fn handle_sync_finished(&mut self, result: Result<SyncReport, String>) {
        let active_sync = self.active_sync.take().unwrap_or(ActiveSync {
            direction: SyncDirection::FromUsb,
            trigger: SyncTrigger::Manual,
        });
        self.syncing = false;
        self.sync_progress = None;

        match result {
            Ok(report) => {
                if matches!(active_sync.trigger, SyncTrigger::Watch) && !report.has_activity() {
                    self.last_status = active_sync.direction.idle_watch_text().to_string();
                } else {
                    self.last_status =
                        format!("{}: {}", active_sync.direction.last_sync_text(), report.summary());
                    append_log(&self.paths, self.last_status.clone());
                }
                if report.drive_ejected {
                    self.drive_present = false;
                    self.pending_pull_sync = false;
                    self.pending_push_sync = false;
                }
            }
            Err(error) => {
                self.last_status = format!(
                    "{}: {error}",
                    active_sync
                        .direction
                        .failure_text(matches!(active_sync.trigger, SyncTrigger::Watch))
                );
                append_log(&self.paths, self.last_status.clone());
            }
        }

        self.update_drive_presence();
        self.refresh_watchers();
        if !self.maybe_run_pending_syncs() {
            self.update_ui();
        }
    }

    fn update_ui(&mut self) {
        if let Some(menu) = self.menu.as_ref() {
            let config_loaded = self.config.is_some();
            menu.status.set_text(self.menu_status_text());
            menu.progress.set_text(self.menu_progress_text());
            menu.sync_from_usb_now
                .set_enabled(config_loaded && self.drive_present && !self.syncing);
            menu.sync_to_usb_now
                .set_enabled(config_loaded && self.drive_present && !self.syncing);
            menu.eject_now
                .set_enabled(config_loaded && self.drive_present && !self.syncing);
            menu.open_drive.set_enabled(config_loaded && self.drive_present);
            menu.open_shadow.set_enabled(config_loaded);
        }

        if let Some(tray) = self.tray.as_ref() {
            let tooltip = self.tooltip_text();
            let _ = tray.set_tooltip(Some(tooltip));
            let _ = tray.set_icon(Some(status_icon(
                self.syncing,
                self.config_error.is_some(),
                self.drive_present,
            )));
        }
    }

    fn tooltip_text(&self) -> String {
        if let Some(progress) = self.sync_progress.as_ref() {
            return format!(
                "USB Mirror Sync\n{}\n{}",
                self.progress_headline(progress),
                self.progress_detail(progress)
            );
        }

        if let Some(error) = self.config_error.as_ref() {
            format!("USB Mirror Sync\nConfig error\n{}", truncate(error, 90))
        } else {
            let drive_line = self
                .config
                .as_ref()
                .map(|config| {
                    format!(
                        "Drive {}: {}",
                        config.drive_label,
                        if self.drive_present { "ready" } else { "missing" }
                    )
                })
                .unwrap_or_else(|| "Drive: not configured".to_string());
            format!(
                "USB Mirror Sync\n{}\n{}",
                drive_line,
                truncate(&self.last_status, 100)
            )
        }
    }

    fn menu_status_text(&self) -> String {
        if self.config_error.is_some() {
            "Status: Config error".to_string()
        } else if self.syncing {
            let active_sync = self.active_sync.unwrap_or(ActiveSync {
                direction: SyncDirection::FromUsb,
                trigger: SyncTrigger::Manual,
            });
            format!("Status: {}", active_sync.direction.syncing_text())
        } else if !self.drive_present {
            "Status: Waiting for drive".to_string()
        } else if self.last_status.starts_with("Watching ") {
            "Status: Watching".to_string()
        } else if self.last_status.starts_with("Last sync ") {
            "Status: Synced".to_string()
        } else {
            "Status: Ready".to_string()
        }
    }

    fn menu_progress_text(&self) -> String {
        if let Some(progress) = self.sync_progress.as_ref() {
            format!(
                "Progress: {}",
                self.progress_percent(progress)
                    .map(|percent| format!("{percent}%"))
                    .unwrap_or_else(|| {
                        format!("{}/{}", progress.operations_done, progress.operations_total)
                    })
            )
        } else {
            "Progress: Idle".to_string()
        }
    }

    fn progress_headline(&self, progress: &SyncProgress) -> String {
        let phase = match progress.phase {
            SyncPhase::Planning => "Planning",
            SyncPhase::Copying => "Copying",
            SyncPhase::Deleting => "Deleting",
            SyncPhase::Finalizing => "Finalizing",
        };
        let percent = self
            .progress_percent(progress)
            .map(|value| format!("{value}%"))
            .unwrap_or_else(|| "0%".to_string());
        let direction = self
            .active_sync
            .map(|active| active.direction.syncing_text())
            .unwrap_or("Syncing");
        format!(
            "{direction} | {phase} {}/{} ({percent})",
            progress.operations_done, progress.operations_total
        )
    }

    fn progress_detail(&self, progress: &SyncProgress) -> String {
        let path = progress
            .current_path
            .as_deref()
            .map(|value| truncate(value, 80))
            .unwrap_or_else(|| progress.current_job.clone());
        let bytes = if progress.bytes_total > 0 {
            format!(
                "{:.1}/{:.1} MB",
                progress.bytes_done as f64 / (1024.0 * 1024.0),
                progress.bytes_total as f64 / (1024.0 * 1024.0)
            )
        } else {
            "0.0/0.0 MB".to_string()
        };
        format!(
            "{} | job {}/{} | {}",
            path,
            progress.job_index.max(1),
            progress.job_count.max(1),
            bytes
        )
    }

    fn progress_percent(&self, progress: &SyncProgress) -> Option<u64> {
        if progress.operations_total == 0 {
            None
        } else {
            Some(((progress.operations_done as u64) * 100) / (progress.operations_total as u64))
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.tray.is_some() {
            return;
        }

        match AppMenu::build() {
            Ok((menu, tray)) => {
                self.menu = Some(menu);
                self.tray = Some(tray);
                self.initialize();
            }
            Err(error) => {
                self.last_status = format!("Tray init failed: {error}");
                append_log(&self.paths, self.last_status.clone());
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Tick => {
                self.reload_config(false);
                self.update_drive_presence();
                self.maybe_auto_sync();
                self.refresh_watchers();
                self.update_ui();
            }
            UserEvent::Menu(event) => self.handle_menu_event(event_loop, event),
            UserEvent::SyncProgress(progress) => self.handle_sync_progress(progress),
            UserEvent::SyncFinished(result) => self.handle_sync_finished(result),
            UserEvent::WatchedChange(kind) => self.handle_watched_change(kind),
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }
}

struct AppMenu {
    status: MenuItem,
    progress: MenuItem,
    sync_from_usb_now: MenuItem,
    sync_to_usb_now: MenuItem,
    eject_now: MenuItem,
    setup_wizard: MenuItem,
    open_drive: MenuItem,
    open_shadow: MenuItem,
    open_config: MenuItem,
    open_log: MenuItem,
    open_folder: MenuItem,
    quit: MenuItem,
}

impl AppMenu {
    fn build() -> Result<(Self, TrayIcon)> {
        let menu = Menu::new();
        let status = MenuItem::new("Status: Starting", false, None);
        let progress = MenuItem::new("Progress: Idle", false, None);
        let sync_from_usb_now = MenuItem::new("Sync from USB now", false, None);
        let sync_to_usb_now = MenuItem::new("Sync to USB now", false, None);
        let eject_now = MenuItem::new("Eject drive", false, None);
        let setup_wizard = MenuItem::new("Setup Wizard", true, None);
        let open_drive = MenuItem::new("Open mounted drive", false, None);
        let open_shadow = MenuItem::new("Open shadow cache", false, None);
        let open_config = MenuItem::new("Open raw config", true, None);
        let open_log = MenuItem::new("Open log", true, None);
        let open_folder = MenuItem::new("Open app folder", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let separator_1 = PredefinedMenuItem::separator();
        let separator_2 = PredefinedMenuItem::separator();
        let separator_3 = PredefinedMenuItem::separator();

        menu.append_items(&[
            &status,
            &progress,
            &separator_1,
            &sync_from_usb_now,
            &sync_to_usb_now,
            &eject_now,
            &separator_2,
            &setup_wizard,
            &open_drive,
            &open_shadow,
            &open_config,
            &open_log,
            &open_folder,
            &separator_3,
            &quit,
        ])?;

        let tray = TrayIconBuilder::new()
            .with_tooltip("USB Mirror Sync")
            .with_icon(status_icon(false, false, false))
            .with_menu(Box::new(menu))
            .build()?;

        Ok((
            Self {
                status,
                progress,
                sync_from_usb_now,
                sync_to_usb_now,
                eject_now,
                setup_wizard,
                open_drive,
                open_shadow,
                open_config,
                open_log,
                open_folder,
                quit,
            },
            tray,
        ))
    }
}

fn status_icon(syncing: bool, has_error: bool, drive_present: bool) -> Icon {
    let color = if has_error {
        [0xD1, 0x43, 0x43, 0xFF]
    } else if syncing {
        [0xD9, 0x90, 0x1D, 0xFF]
    } else if drive_present {
        [0x1F, 0x7A, 0xC2, 0xFF]
    } else {
        [0x64, 0x6E, 0x78, 0xFF]
    };

    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let edge = x < 2 || x > 13 || y < 2 || y > 13;
            let alpha = if edge { 0x90 } else { 0xFF };
            rgba.extend_from_slice(&[color[0], color[1], color[2], alpha]);
        }
    }
    Icon::from_rgba(rgba, 16, 16).expect("icon buffer should be valid")
}

fn truncate(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}
