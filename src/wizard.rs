use std::env;
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use eframe::egui;
use rfd::FileDialog;
use serde_json::Value;

use crate::config::{AppConfig, AppPaths, JobConfig, default_config_template, load_config};
use crate::platform;

const WIZARD_FLAG: &str = "--wizard";
const LOADING_SIGNAL_FLAG: &str = "--loading-signal=";
const CONTROL_HEIGHT: f32 = 26.0;

#[derive(Debug, Default, Clone)]
pub struct WizardLaunchContext {
    pub error_message: Option<String>,
    pub recovery_backup_path: Option<PathBuf>,
    pub recovered_default: bool,
    pub loading_signal_path: Option<PathBuf>,
}

pub fn maybe_run_from_args(paths: &AppPaths) -> Result<bool> {
    let mut args = env::args().skip(1);
    let Some(first) = args.next() else {
        return Ok(false);
    };
    if first != WIZARD_FLAG {
        return Ok(false);
    }

    paths.ensure_wizard_layout()?;
    append_wizard_log(paths, "Wizard process starting");

    let mut context = WizardLaunchContext::default();
    while let Some(argument) = args.next() {
        if let Some(value) = argument.strip_prefix("--error-message=") {
            context.error_message = Some(value.to_string());
        } else if let Some(value) = argument.strip_prefix("--recovery-backup=") {
            context.recovery_backup_path = Some(PathBuf::from(value));
        } else if argument == "--recovered-default" {
            context.recovered_default = true;
        } else if let Some(value) = argument.strip_prefix(LOADING_SIGNAL_FLAG) {
            context.loading_signal_path = Some(PathBuf::from(value));
        }
    }

    start_loading_indicator(paths, &mut context)?;
    run_setup_wizard(paths.clone(), context)?;
    Ok(true)
}

pub fn open_setup_wizard(paths: &AppPaths) -> Result<()> {
    open_setup_wizard_with_context(paths, &WizardLaunchContext::default())
}

pub fn open_setup_wizard_with_context(
    paths: &AppPaths,
    context: &WizardLaunchContext,
) -> Result<()> {
    let mut context = context.clone();
    start_loading_indicator(paths, &mut context)?;
    let loading_signal_path = context
        .loading_signal_path
        .clone()
        .context("missing wizard loading signal path")?;

    let exe = platform::current_exe()?;
    let mut command = Command::new(exe);
    command.arg(WIZARD_FLAG);
    command.arg(format!(
        "{LOADING_SIGNAL_FLAG}{}",
        loading_signal_path.display()
    ));

    if let Some(error_message) = context.error_message.as_ref() {
        command.arg(format!("--error-message={error_message}"));
    }
    if let Some(recovery_backup_path) = context.recovery_backup_path.as_ref() {
        command.arg(format!(
            "--recovery-backup={}",
            recovery_backup_path.display()
        ));
    }
    if context.recovered_default {
        command.arg("--recovered-default");
    }

    if let Err(error) = command.spawn() {
        let _ = fs::remove_file(&loading_signal_path);
        return Err(error).context("failed to launch setup wizard");
    }
    append_wizard_log(paths, "Wizard launch requested from tray app");
    Ok(())
}

pub fn prepare_recovery_context(paths: &AppPaths, error_message: &str) -> Result<WizardLaunchContext> {
    append_wizard_log(paths, format!("Preparing recovery context: {error_message}"));
    let mut context = WizardLaunchContext {
        error_message: Some(error_message.to_string()),
        ..WizardLaunchContext::default()
    };

    let raw = match fs::read_to_string(&paths.config_file) {
        Ok(raw) => raw,
        Err(_) => {
            fs::write(&paths.config_file, default_config_template())
                .with_context(|| format!("failed to repair {}", paths.config_file.display()))?;
            append_wizard_log(paths, "Config missing or unreadable; restored default config");
            context.recovered_default = true;
            return Ok(context);
        }
    };

    let trimmed = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let parsed = serde_json::from_str::<Value>(trimmed);
    if parsed.is_ok() {
        return Ok(context);
    }

    let backup_path = backup_invalid_config(paths, &raw)?;
    fs::write(&paths.config_file, default_config_template())
        .with_context(|| format!("failed to repair {}", paths.config_file.display()))?;
    append_wizard_log(
        paths,
        format!("Invalid config backed up to {}", backup_path.display()),
    );

    context.recovered_default = true;
    context.recovery_backup_path = Some(backup_path);
    Ok(context)
}

fn backup_invalid_config(paths: &AppPaths, raw: &str) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let backup_path = paths.app_dir.join(format!("config.invalid.{timestamp}.json"));
    fs::write(&backup_path, raw)
        .with_context(|| format!("failed to write {}", backup_path.display()))?;
    Ok(backup_path)
}

fn create_loading_signal_path(paths: &AppPaths) -> Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    Ok(paths.app_dir.join(format!("wizard-loading-{timestamp}.signal")))
}

fn start_loading_indicator(paths: &AppPaths, context: &mut WizardLaunchContext) -> Result<()> {
    if context.loading_signal_path.is_some() {
        return Ok(());
    }

    let loading_signal_path = create_loading_signal_path(paths)?;
    fs::write(&loading_signal_path, b"loading")
        .with_context(|| format!("failed to create {}", loading_signal_path.display()))?;

    if let Err(error) = platform::show_wizard_loading_indicator(&loading_signal_path) {
        append_wizard_log(paths, format!("Wizard loading splash failed: {error}"));
    }

    context.loading_signal_path = Some(loading_signal_path);
    Ok(())
}

fn wizard_log_path(paths: &AppPaths) -> PathBuf {
    paths.app_dir.join("wizard.log")
}

fn append_wizard_log(paths: &AppPaths, line: impl AsRef<str>) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let entry = format!("[{timestamp}] {}\r\n", line.as_ref());
    let _ = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(wizard_log_path(paths))
        .and_then(|mut file| std::io::Write::write_all(&mut file, entry.as_bytes()));
}

fn run_setup_wizard(paths: AppPaths, context: WizardLaunchContext) -> Result<()> {
    append_wizard_log(&paths, "Wizard UI booting");
    let app = WizardApp::load(paths, context);
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 760.0])
        .with_min_inner_size([980.0, 680.0])
        .with_title("ShadowSync Setup")
        .with_transparent(false)
        .with_has_shadow(true);
    if let Some(icon) = wizard_window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "ShadowSync Setup",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
    .map_err(|error| anyhow::anyhow!("failed to run setup wizard: {error}"))
}

fn wizard_window_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/wizard_icon_256.png"))).ok()
}

struct WizardApp {
    paths: AppPaths,
    context: WizardLaunchContext,
    config: AppConfig,
    status: String,
    missing_target_prompt: Option<MissingTargetPrompt>,
    current_step: WizardStep,
    theme_applied: Option<egui::Theme>,
    loading_signal_path: Option<PathBuf>,
    startup_signal_cleared: bool,
}

#[derive(Debug, Clone)]
struct MissingTargetPrompt {
    job_index: usize,
    target_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardStep {
    Welcome,
    Drive,
    SyncBehavior,
    Jobs,
    Review,
}

#[derive(Clone, Copy)]
struct WizardPalette {
    accent: egui::Color32,
    accent_soft: egui::Color32,
    surface: egui::Color32,
    surface_strong: egui::Color32,
    surface_muted: egui::Color32,
    stroke: egui::Color32,
    text_muted: egui::Color32,
    danger: egui::Color32,
}

impl WizardApp {
    fn load(paths: AppPaths, context: WizardLaunchContext) -> Self {
        let config = fs::read_to_string(&paths.config_file)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppConfig>(raw.strip_prefix('\u{feff}').unwrap_or(&raw)).ok())
            .unwrap_or_default();
        let missing_target_prompt = first_missing_target_prompt(&config);
        let loading_signal_path = context.loading_signal_path.clone();

        Self {
            paths,
            context,
            config,
            status: "Use the steps below to set up ShadowSync.".to_string(),
            missing_target_prompt,
            current_step: WizardStep::Welcome,
            theme_applied: None,
            loading_signal_path,
            startup_signal_cleared: false,
        }
    }

    fn validate_and_save(&mut self) -> Result<()> {
        append_wizard_log(&self.paths, "Validating wizard configuration");
        let config = self.normalized_config_for_save()?;
        let serialized = serde_json::to_string_pretty(&config)
            .context("failed to serialize config.json")?;

        let temp_file = self.paths.app_dir.join("config.validation.json");
        fs::write(&temp_file, &serialized)
            .with_context(|| format!("failed to write {}", temp_file.display()))?;

        let validation_paths = AppPaths {
            app_dir: self.paths.app_dir.clone(),
            config_file: temp_file.clone(),
            manifest_file: self.paths.manifest_file.clone(),
            log_file: self.paths.log_file.clone(),
            shadow_root: self.paths.shadow_root.clone(),
        };

        let validation = load_config(&validation_paths);
        let _ = fs::remove_file(&temp_file);
        validation?;

        fs::write(&self.paths.config_file, serialized)
            .with_context(|| format!("failed to write {}", self.paths.config_file.display()))?;
        append_wizard_log(
            &self.paths,
            format!("Saved wizard config to {}", self.paths.config_file.display()),
        );
        Ok(())
    }

    fn normalized_config_for_save(&self) -> Result<AppConfig> {
        let mut config = self.config.clone();
        normalize_optional_string(&mut config.drive.letter);
        normalize_optional_string(&mut config.drive.path);
        normalize_optional_string(&mut config.cache.root);
        let drive_root = drive_root_from_config(&config);

        for job in &mut config.jobs {
            job.name = job.name.trim().to_string();
            job.source = normalize_job_source_text(&job.source, drive_root.as_deref())
                .with_context(|| format!("job '{}' source is invalid", job.name.trim()))?;
            job.target = job.target.trim().to_string();
        }

        Ok(config)
    }

    fn current_drive_root(&self) -> Option<PathBuf> {
        if let Some(path) = self
            .config
            .drive
            .path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(PathBuf::from(path));
        }

        #[cfg(target_os = "windows")]
        {
            let letter = self
                .config
                .drive
                .letter
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            return Some(PathBuf::from(format!("{}:\\", letter.trim_end_matches(':'))));
        }

        #[allow(unreachable_code)]
        None
    }

    fn browse_mount_path(&mut self) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.config.drive.path = Some(folder.display().to_string());
        }
    }

    fn browse_cache_root(&mut self) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.config.cache.root = Some(folder.display().to_string());
        }
    }

    fn browse_job_source(&mut self, index: usize) {
        let Some(root) = self.current_drive_root() else {
            self.status = "Set the USB root location first, then browse a folder inside it.".to_string();
            return;
        };

        let picker = FileDialog::new().set_directory(&root);
        if let Some(folder) = picker.pick_folder() {
            match normalize_job_source_from_path(&folder, &root) {
                Ok(value) => {
                    self.config.jobs[index].source = value;
                    self.status = format!("Selected {}", folder.display());
                }
                Err(error) => {
                    self.status = format!("Source browse failed: {error}");
                }
            }
        }
    }

    fn browse_job_target(&mut self, index: usize) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.config.jobs[index].target = folder.display().to_string();
            if index == 0 {
                self.missing_target_prompt = first_missing_target_prompt(&self.config);
            }
        }
    }

    fn create_job_target_folder(&mut self, index: usize) {
        let Some(job) = self.config.jobs.get(index) else {
            return;
        };

        let target = PathBuf::from(job.target.trim());
        if target.as_os_str().is_empty() {
            self.status = "The local target is blank. Choose a folder first.".to_string();
            return;
        }
        if !target.is_absolute() {
            self.status = "The local target must be an absolute path.".to_string();
            return;
        }

        match fs::create_dir_all(&target) {
            Ok(()) => {
                self.status = format!("Created local target {}", target.display());
                if index == 0 {
                    self.missing_target_prompt = first_missing_target_prompt(&self.config);
                }
            }
            Err(error) => {
                self.status = format!("Failed to create {}: {error}", target.display());
            }
        }
    }

    fn effective_shadow_cache_root(&self) -> PathBuf {
        match self
            .config
            .cache
            .root
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => {
                let path = PathBuf::from(value);
                if path.is_absolute() {
                    path
                } else {
                    self.paths.app_dir.join(path)
                }
            }
            None => self.paths.shadow_root.clone(),
        }
    }

    fn banner_text(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(error_message) = self.context.error_message.as_ref() {
            parts.push(format!("Config issue: {error_message}"));
        }
        if self.context.recovered_default {
            parts.push("A safe default config was restored so the wizard could open.".to_string());
        }
        if let Some(backup_path) = self.context.recovery_backup_path.as_ref() {
            parts.push(format!("Broken config backup: {}", backup_path.display()));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    fn clear_startup_signal_if_needed(&mut self) {
        if self.startup_signal_cleared {
            return;
        }

        if let Some(path) = self.loading_signal_path.take() {
            let _ = fs::remove_file(&path);
        }
        self.startup_signal_cleared = true;
        append_wizard_log(&self.paths, "Wizard UI ready");
    }

    fn open_wizard_log(&mut self) {
        let path = wizard_log_path(&self.paths);
        match platform::open_path(&path) {
            Ok(()) => append_wizard_log(&self.paths, "Wizard log opened"),
            Err(error) => {
                self.status = format!("Open wizard log failed: {error}");
                append_wizard_log(&self.paths, &self.status);
            }
        }
    }
}

fn drive_root_from_config(config: &AppConfig) -> Option<PathBuf> {
    if let Some(path) = config
        .drive
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(PathBuf::from(path));
    }

    #[cfg(target_os = "windows")]
    {
        let letter = config
            .drive
            .letter
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())?;
        return Some(PathBuf::from(format!("{}:\\", letter.trim_end_matches(':'))));
    }

    #[allow(unreachable_code)]
    None
}

fn normalize_job_source_from_path(path: &Path, root: &Path) -> Result<String> {
    let relative = strip_root_prefix_normalized(path, root).ok_or_else(|| {
        anyhow::anyhow!(
            "the selected folder must stay inside the configured drive root {}",
            root.display()
        )
    })?;
    native_relative_path_string(&relative)
}

fn normalize_job_source_text(value: &str, root: Option<&Path>) -> Result<String> {
    let trimmed = value.trim();
    anyhow::ensure!(!trimmed.is_empty(), "source path must not be empty");

    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        let root = root.ok_or_else(|| {
            anyhow::anyhow!("set the USB root first before using an absolute source path")
        })?;
        return normalize_job_source_from_path(&candidate, root);
    }

    let mut normalized = PathBuf::new();
    for part in trimmed.split(['/', '\\']).filter(|part| !part.is_empty()) {
        match part {
            "." => {}
            ".." => anyhow::bail!("source path must stay inside the USB root"),
            _ => normalized.push(part),
        }
    }

    anyhow::ensure!(
        !normalized.as_os_str().is_empty(),
        "source path must not collapse to an empty value"
    );
    native_relative_path_string(&normalized)
}

fn native_relative_path_string(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => anyhow::bail!("path contains a non-normal component: {}", path.display()),
        }
    }

    anyhow::ensure!(!parts.is_empty(), "path must not be empty");
    Ok(parts.join(std::path::MAIN_SEPARATOR_STR))
}

fn strip_root_prefix_normalized(path: &Path, root: &Path) -> Option<PathBuf> {
    let path_parts = normalized_path_parts(path)?;
    let root_parts = normalized_path_parts(root)?;
    if path_parts.len() < root_parts.len() {
        return None;
    }
    if !path_parts
        .iter()
        .zip(root_parts.iter())
        .all(|(path_part, root_part)| path_part_matches(path_part, root_part))
    {
        return None;
    }

    Some(
        path_parts[root_parts.len()..]
            .iter()
            .fold(PathBuf::new(), |mut path, part| {
                path.push(part);
                path
            }),
    )
}

fn normalized_path_parts(path: &Path) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => parts.push(prefix.as_os_str().to_string_lossy().to_string()),
            Component::RootDir => {}
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::ParentDir => return None,
        }
    }
    Some(parts)
}

#[cfg(target_os = "windows")]
fn path_part_matches(path_part: &str, root_part: &str) -> bool {
    path_part.eq_ignore_ascii_case(root_part)
}

#[cfg(not(target_os = "windows"))]
fn path_part_matches(path_part: &str, root_part: &str) -> bool {
    path_part == root_part
}

impl eframe::App for WizardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.clear_startup_signal_if_needed();
        self.apply_system_theme(ctx);
        let palette = self.palette();

        let mut browse_missing_target = None;
        let mut create_missing_target = None;
        let mut dismiss_missing_target = false;

        egui::TopBottomPanel::top("wizard_header")
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(egui::Stroke::new(1.0, palette.stroke))
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("SHADOWSYNC")
                                .size(11.0)
                                .color(palette.accent)
                                .strong(),
                        );
                        ui.heading("Setup Wizard");
                    });
                    ui.add_space(10.0);
                    self.render_info_pill(ui, "Fast shadow-backed USB sync", false);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            append_wizard_log(&self.paths, "Wizard closed from header");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Open Wizard Log").clicked() {
                            self.open_wizard_log();
                        }
                    });
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(self.status.clone())
                        .size(12.0)
                        .color(palette.text_muted),
                );
                ui.add_space(8.0);
                self.render_stepper(ui);
            });

        egui::TopBottomPanel::bottom("wizard_footer")
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(egui::Stroke::new(1.0, palette.stroke))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let at_first = self.step_index() == 0;
                    let at_last = self.current_step == WizardStep::Review;
                    if ui.add_enabled(!at_first, egui::Button::new("Back")).clicked() {
                        self.go_back();
                    }
                    if ui.add_enabled(!at_last, egui::Button::new("Next")).clicked() {
                        self.go_next();
                    }
                    ui.separator();
                    ui.small(format!(
                        "Step {} of {}",
                        self.step_index() + 1,
                        Self::step_sequence().len()
                    ));
                });
            });

        if let Some(prompt) = self.missing_target_prompt.as_ref() {
            egui::Window::new("Local target folder missing")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Job {} points to a local target that does not exist yet:",
                        prompt.job_index + 1
                    ));
                    ui.monospace(prompt.target_path.display().to_string());
                    ui.add_space(8.0);
                    ui.label("Choose an existing folder, create this one now, or dismiss and handle it later.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Browse").clicked() {
                            browse_missing_target = Some(prompt.job_index);
                        }
                        if ui.button("Create folder").clicked() {
                            create_missing_target = Some(prompt.job_index);
                        }
                        if ui.button("Dismiss").clicked() {
                            dismiss_missing_target = true;
                        }
                    });
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    if let Some(banner) = self.banner_text() {
                        self.card_frame(true)
                            .inner_margin(egui::Margin::same(18))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("CONFIG RECOVERY")
                                        .size(11.0)
                                        .color(palette.danger)
                                        .strong(),
                                );
                                ui.add_space(6.0);
                                ui.colored_label(palette.danger, banner);
                            });
                        ui.add_space(12.0);
                    }

                    self.render_overview_hero(ui);
                    ui.add_space(14.0);
                    match self.current_step {
                        WizardStep::Welcome => self.render_welcome_step(ui),
                        WizardStep::Drive => self.render_drive_step(ui),
                        WizardStep::SyncBehavior => self.render_behavior_step(ui),
                        WizardStep::Jobs => self.render_jobs_step(ui),
                        WizardStep::Review => self.render_review_step(ui, ctx),
                    }
                });
            });

        if let Some(index) = browse_missing_target {
            self.browse_job_target(index);
        }
        if let Some(index) = create_missing_target {
            self.create_job_target_folder(index);
        }
        if dismiss_missing_target {
            self.missing_target_prompt = None;
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        let color = if visuals.dark_mode {
            egui::Color32::from_rgb(11, 16, 21)
        } else {
            egui::Color32::from_rgb(241, 247, 252)
        };
        color.to_normalized_gamma_f32()
    }
}

impl WizardApp {
    fn apply_system_theme(&mut self, ctx: &egui::Context) {
        let Some(theme) = ctx.system_theme() else {
            return;
        };
        if self.theme_applied == Some(theme) {
            return;
        }

        let mut visuals = match theme {
            egui::Theme::Dark => egui::Visuals::dark(),
            egui::Theme::Light => egui::Visuals::light(),
        };
        let stroke_color = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(58, 72, 88)
        } else {
            egui::Color32::from_rgb(198, 209, 220)
        };
        visuals.window_corner_radius = egui::CornerRadius::same(14);
        visuals.menu_corner_radius = egui::CornerRadius::same(12);
        visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.active.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.open.corner_radius = egui::CornerRadius::same(10);
        visuals.panel_fill = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(21, 28, 36)
        } else {
            egui::Color32::from_rgb(247, 250, 253)
        };
        visuals.extreme_bg_color = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(24, 31, 40)
        } else {
            egui::Color32::from_rgb(248, 251, 254)
        };
        visuals.widgets.inactive.bg_fill = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(29, 38, 49)
        } else {
            egui::Color32::from_rgb(248, 251, 254)
        };
        visuals.widgets.hovered.bg_fill = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(34, 43, 54)
        } else {
            egui::Color32::from_rgb(252, 254, 255)
        };
        visuals.widgets.active.bg_fill = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(39, 49, 61)
        } else {
            egui::Color32::from_rgb(240, 246, 252)
        };
        visuals.widgets.open.bg_fill = visuals.widgets.active.bg_fill;
        visuals.window_stroke = egui::Stroke::new(1.0, stroke_color);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, stroke_color);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, stroke_color);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, stroke_color);
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, stroke_color);
        visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, stroke_color);

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 5.0);
        style.spacing.indent = 16.0;
        style.spacing.interact_size = egui::vec2(40.0, 22.0);
        style.spacing.slider_width = 200.0;
        ctx.set_style(style);

        self.theme_applied = Some(theme);
    }

    fn palette(&self) -> WizardPalette {
        match self.theme_applied.unwrap_or(egui::Theme::Dark) {
            egui::Theme::Dark => WizardPalette {
                accent: egui::Color32::from_rgb(69, 170, 255),
                accent_soft: egui::Color32::from_rgba_unmultiplied(69, 170, 255, 110),
                surface: egui::Color32::from_rgb(21, 28, 36),
                surface_strong: egui::Color32::from_rgb(24, 31, 40),
                surface_muted: egui::Color32::from_rgb(29, 38, 49),
                stroke: egui::Color32::from_rgb(58, 72, 88),
                text_muted: egui::Color32::from_rgb(168, 182, 196),
                danger: egui::Color32::from_rgb(232, 108, 90),
            },
            egui::Theme::Light => WizardPalette {
                accent: egui::Color32::from_rgb(0, 116, 217),
                accent_soft: egui::Color32::from_rgba_unmultiplied(0, 116, 217, 90),
                surface: egui::Color32::from_rgb(248, 251, 254),
                surface_strong: egui::Color32::from_rgb(252, 254, 255),
                surface_muted: egui::Color32::from_rgb(242, 246, 250),
                stroke: egui::Color32::from_rgb(198, 209, 220),
                text_muted: egui::Color32::from_rgb(88, 103, 118),
                danger: egui::Color32::from_rgb(201, 79, 61),
            },
        }
    }

    fn step_sequence() -> &'static [WizardStep] {
        const STEPS: [WizardStep; 5] = [
            WizardStep::Welcome,
            WizardStep::Drive,
            WizardStep::SyncBehavior,
            WizardStep::Jobs,
            WizardStep::Review,
        ];
        &STEPS
    }

    fn step_index(&self) -> usize {
        Self::step_sequence()
            .iter()
            .position(|step| *step == self.current_step)
            .unwrap_or(0)
    }

    fn step_title(step: WizardStep) -> &'static str {
        match step {
            WizardStep::Welcome => "Get Started",
            WizardStep::Drive => "Drive",
            WizardStep::SyncBehavior => "Behavior",
            WizardStep::Jobs => "Folders",
            WizardStep::Review => "Review",
        }
    }

    fn step_description(step: WizardStep) -> &'static str {
        match step {
            WizardStep::Welcome => "Confirm how ShadowSync works before saving anything.",
            WizardStep::Drive => "Choose the USB root location ShadowSync should mirror from.",
            WizardStep::SyncBehavior => "Choose how often it watches, syncs, and clears cache data.",
            WizardStep::Jobs => "Point each folder inside that USB root at the local folder you actually use.",
            WizardStep::Review => "Review the effective paths, then save the config.",
        }
    }

    fn go_next(&mut self) {
        let index = self.step_index();
        if let Some(step) = Self::step_sequence().get(index + 1).copied() {
            self.current_step = step;
        }
    }

    fn go_back(&mut self) {
        let index = self.step_index();
        if index > 0 {
            self.current_step = Self::step_sequence()[index - 1];
        }
    }

    fn drive_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(root) = self.current_drive_root() {
            parts.push(format!("root {}", root.display()));
        }
        if let Some(letter) = self
            .config
            .drive
            .letter
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            parts.push(format!("watch letter {}", letter.trim_end_matches(':')));
        }
        if parts.is_empty() {
            "No USB root configured yet".to_string()
        } else {
            parts.join(" | ")
        }
    }

    fn render_stepper(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        ui.horizontal_wrapped(|ui| {
            for (index, step) in Self::step_sequence().iter().copied().enumerate() {
                let selected = step == self.current_step;
                let label = format!("{}. {}", index + 1, Self::step_title(step));
                let button = egui::Button::new(
                    egui::RichText::new(label)
                        .size(13.0)
                        .color(if selected {
                            egui::Color32::WHITE
                        } else {
                            ui.visuals().text_color()
                        }),
                )
                .fill(if selected {
                    palette.accent
                } else {
                    palette.surface_muted
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if selected { palette.accent_soft } else { palette.stroke },
                ))
                .corner_radius(egui::CornerRadius::same(12));
                let response = ui.add(button);
                if response.clicked() {
                    self.current_step = step;
                }
            }
        });
    }

    fn render_overview_hero(&self, ui: &mut egui::Ui) {
        let palette = self.palette();
        self.card_frame(true)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("STEP {}", self.step_index() + 1))
                                .size(11.0)
                                .color(palette.accent)
                                .strong(),
                        );
                        ui.heading(Self::step_title(self.current_step));
                        ui.label(
                            egui::RichText::new(Self::step_description(self.current_step))
                                .color(palette.text_muted),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        self.render_info_pill(
                            ui,
                            &format!("{} jobs", self.config.jobs.len()),
                            true,
                        );
                    });
                });
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    self.render_info_pill(
                        ui,
                        &format!("USB {}", self.drive_summary()),
                        false,
                    );
                    self.render_info_pill(
                        ui,
                        &format!("Cache {}", self.effective_shadow_cache_root().display()),
                        false,
                    );
                });
            });
    }

    fn render_path_badge(ui: &mut egui::Ui, path: &Path) {
        let (text, color) = if path.exists() {
            ("Exists", egui::Color32::from_rgb(75, 163, 113))
        } else {
            ("Missing", egui::Color32::from_rgb(196, 95, 73))
        };
        ui.colored_label(color, text);
    }

    fn render_card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
        let mut frame = egui::Frame::group(ui.style());
        frame.fill = ui.visuals().widgets.inactive.bg_fill;
        frame.stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
        frame
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.strong(title);
                ui.add_space(8.0);
                add_contents(ui);
            });
    }

    fn render_text_field_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
        ui.vertical(|ui| {
            ui.label(label);
            ui.add_sized(
                [ui.available_width().max(180.0), CONTROL_HEIGHT],
                egui::TextEdit::singleline(value),
            );
        });
    }

    fn render_browse_field_row(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
        let mut clicked = false;
        ui.vertical(|ui| {
            ui.label(label);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let button_width = 88.0;
                let field_width = (ui.available_width() - button_width - ui.spacing().item_spacing.x)
                    .max(180.0);
                ui.add_sized(
                    [field_width, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(value),
                );
                clicked = ui
                    .add_sized([button_width, CONTROL_HEIGHT], egui::Button::new("Browse"))
                    .clicked();
            });
        });
        clicked
    }

    fn render_action_button(ui: &mut egui::Ui, label: &str, width: f32) -> egui::Response {
        ui.add_sized([width, CONTROL_HEIGHT], egui::Button::new(label))
    }

    fn card_frame(&self, emphasis: bool) -> egui::Frame {
        let palette = self.palette();
        egui::Frame::new()
            .fill(if emphasis {
                palette.surface_strong
            } else {
                palette.surface
            })
            .stroke(egui::Stroke::new(
                1.0,
                if emphasis {
                    palette.accent_soft
                } else {
                    palette.stroke
                },
            ))
            .corner_radius(egui::CornerRadius::same(18))
            .shadow(egui::epaint::Shadow {
                offset: [0, 10],
                blur: 28,
                spread: 0,
                color: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 52),
            })
    }

    fn render_info_pill(&self, ui: &mut egui::Ui, text: &str, accent: bool) {
        let palette = self.palette();
        let fill = if accent {
            palette.accent
        } else {
            palette.surface_muted
        };
        let text_color = if accent {
            egui::Color32::WHITE
        } else {
            ui.visuals().text_color()
        };
        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, palette.stroke))
            .corner_radius(egui::CornerRadius::same(255))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(12.0).color(text_color));
            });
    }

    fn render_welcome_step(&self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            self.card_frame(true)
                .inner_margin(egui::Margin::same(20))
                .show(&mut columns[0], |ui| {
                ui.label("ShadowSync normally pulls from the USB into the shadow cache and then into the local folder you work from.");
                ui.add_space(8.0);
                ui.label("Typical flow:");
                ui.monospace("USB root  ->  shadow cache  ->  local folder");
                ui.add_space(10.0);
                ui.label("If you enable push-back later, the same cache is reused in the opposite direction.");
            });
            columns[0].add_space(12.0);
            self.card_frame(false)
                .inner_margin(egui::Margin::same(18))
                .show(&mut columns[0], |ui| {
                ui.strong("What this setup covers");
                ui.add_space(8.0);
                ui.label("1. Pick the USB root location, either the whole drive or one folder on it.");
                ui.label("2. Confirm how often ShadowSync should react.");
                ui.label("3. Point a folder inside that USB root at a local target folder.");
                ui.label("4. Review the effective paths before saving.");
            });

            self.card_frame(false)
                .inner_margin(egui::Margin::same(18))
                .show(&mut columns[1], |ui| {
                ui.strong("Recommended defaults");
                ui.add_space(8.0);
                ui.label("Keep shadow cache enabled so local file locks do not block sync runs.");
                ui.label("Leave custom cache root blank unless you want it somewhere specific.");
                ui.label("Start with manual push-back disabled until you trust the workflow.");
            });
            columns[1].add_space(12.0);
            self.card_frame(false)
                .inner_margin(egui::Margin::same(18))
                .show(&mut columns[1], |ui| {
                ui.strong("What you will end up with");
                ui.add_space(8.0);
                ui.label("A USB root location to monitor.");
                ui.label("One or more folder mappings into local working folders.");
                ui.label("A reusable shadow cache that speeds up repeated sync runs.");
            });
        });
    }

    fn render_drive_step(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            Self::render_card(&mut columns[0], "USB root location", |ui| {
                ui.strong("USB root location");
                ui.label("Pick the top-level USB location ShadowSync should mirror from.");
                ui.label("Use the whole drive like `S:\\` or one folder on the drive like `S:\\SyncRoot`. Job sources below will be relative to this root.");
                ui.add_space(8.0);
                let path = self.config.drive.path.get_or_insert_default();
                if Self::render_browse_field_row(ui, "USB root folder or drive path", path) {
                    self.browse_mount_path();
                }
                ui.small("Examples: `S:\\`, `S:\\SyncRoot`, `/Volumes/USBDrive`, `/media/user/USBDrive/SyncRoot`");
                ui.add_space(8.0);
                let letter = self.config.drive.letter.get_or_insert_default();
                Self::render_text_field_row(ui, "Windows drive letter (optional)", letter);
                ui.small("On Windows, the drive letter helps with detection and eject. The path above still defines the actual USB root ShadowSync mirrors.");
                ui.checkbox(&mut self.config.drive.eject_after_sync, "Eject after sync");
            });

            Self::render_card(&mut columns[1], "Current resolution", |ui| {
                ui.label(format!("Effective USB root: {}", self.drive_summary()));
                ui.add_space(8.0);
                ui.label("Use one folder if you only want to mirror a single section of the drive.");
                ui.label("Use the drive root if you want job paths like `SyncRoot\\Documents` or `Media\\Photos` under the same device.");
            });
            columns[1].add_space(12.0);
            Self::render_card(&mut columns[1], "Examples", |ui| {
                ui.monospace("Whole drive:   S:\\");
                ui.monospace("One folder:    S:\\SyncRoot");
                ui.monospace("macOS:         /Volumes/USBDrive");
                ui.monospace("Linux:         /media/user/USBDrive/SyncRoot");
            });
        });
    }

    fn render_behavior_step(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            Self::render_card(&mut columns[0], "Behavior", |ui| {
                ui.strong("Behavior");
                ui.checkbox(&mut self.config.app.sync_on_insert, "Sync on insert");
                ui.checkbox(
                    &mut self.config.app.sync_while_mounted,
                    "Watch the mounted drive for changes",
                );
                ui.checkbox(
                    &mut self.config.app.auto_sync_to_usb,
                    "Auto-push local target changes back to the drive",
                );
                ui.horizontal(|ui| {
                    ui.label("Drive/config poll seconds");
                    ui.add(egui::DragValue::new(&mut self.config.app.poll_interval_seconds).range(1..=60));
                });
            });

            columns[0].add_space(12.0);

            Self::render_card(&mut columns[0], "Shadow cache", |ui| {
                ui.strong("Shadow cache");
                ui.checkbox(&mut self.config.cache.shadow_copy, "Enable shadow cache");
                ui.checkbox(
                    &mut self.config.cache.clear_shadow_on_eject,
                    "Clear shadow cache on eject",
                );
                let cache_root = self.config.cache.root.get_or_insert_default();
                if Self::render_browse_field_row(ui, "Custom cache root", cache_root) {
                    self.browse_cache_root();
                }
                ui.small(format!(
                    "Current shadow cache: {}",
                    self.effective_shadow_cache_root().display()
                ));
                ui.small("Leave custom cache root blank to use the default app cache folder.");
            });

            columns[1].add_space(0.0);
            Self::render_card(&mut columns[1], "How this behaves", |ui| {
                ui.label("`Sync on insert` runs an import as soon as the USB root appears.");
                ui.label("`Watch the mounted drive` keeps checking for changes while the device stays connected.");
                ui.label("`Auto-push` is separate. Leave it off if you only want to push back manually.");
            });
            columns[1].add_space(12.0);
            Self::render_card(&mut columns[1], "Current effective settings", |ui| {
                ui.label(format!(
                    "Poll every {} second(s)",
                    self.config.app.poll_interval_seconds
                ));
                ui.label(format!(
                    "Shadow cache root: {}",
                    self.effective_shadow_cache_root().display()
                ));
                ui.label(if self.config.cache.shadow_copy {
                    "Shadow cache is enabled."
                } else {
                    "Shadow cache is disabled."
                });
            });
            columns[1].add_space(12.0);

            Self::render_card(&mut columns[1], "Comparison", |ui| {
                ui.strong("Comparison");
                ui.checkbox(
                    &mut self.config.compare.hash_on_metadata_change,
                    "Hash files when metadata changes",
                );
            });
        });
    }

    fn render_jobs_step(&mut self, ui: &mut egui::Ui) {
        Self::render_card(ui, "Folder jobs", |ui| {
            ui.horizontal(|ui| {
                ui.strong("Folder jobs");
                if Self::render_action_button(ui, "Add job", 96.0).clicked() {
                    self.config.jobs.push(JobConfig::default());
                }
            });
            ui.label("Each job maps a folder inside the USB root above to a real local folder on this machine.");
        });

        ui.add_space(12.0);

        let mut remove_index = None;
        let mut browse_source_index = None;
        let mut browse_target_index = None;

        for index in 0..self.config.jobs.len() {
            let job = &mut self.config.jobs[index];
            egui::Frame::group(ui.style())
                .fill(ui.visuals().widgets.inactive.bg_fill)
                .stroke(egui::Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .inner_margin(egui::Margin::same(18))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Job {}", index + 1));
                        if Self::render_action_button(ui, "Remove", 96.0).clicked() {
                            remove_index = Some(index);
                        }
                    });
                    Self::render_text_field_row(ui, "Name", &mut job.name);
                    if Self::render_browse_field_row(ui, "Folder inside USB root", &mut job.source) {
                        browse_source_index = Some(index);
                    }
                    if Self::render_browse_field_row(ui, "Local target", &mut job.target) {
                        browse_target_index = Some(index);
                    }
                    ui.horizontal(|ui| {
                        let target_path = PathBuf::from(job.target.trim());
                        ui.small("Target folder:");
                        if !target_path.as_os_str().is_empty() && target_path.is_absolute() {
                            Self::render_path_badge(ui, &target_path);
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(196, 95, 73),
                                "Needs valid path",
                            );
                        }
                    });
                    ui.checkbox(&mut job.mirror_deletes, "Mirror deletes");
                });
            ui.add_space(10.0);
        }

        ui.columns(2, |columns| {
            Self::render_card(&mut columns[0], "Current mapping", |ui| {
                if self.config.jobs.is_empty() {
                    ui.label("No jobs yet. Add one above.");
                } else {
                    for (index, job) in self.config.jobs.iter().enumerate() {
                        ui.label(format!(
                            "{}. {} -> {}",
                            index + 1,
                            if job.source.trim().is_empty() {
                                "<USB folder>"
                            } else {
                                job.source.trim()
                            },
                            if job.target.trim().is_empty() {
                                "<local folder>"
                            } else {
                                job.target.trim()
                            }
                        ));
                    }
                }
            });

            Self::render_card(&mut columns[1], "Job rules", |ui| {
                ui.label("Job source stays relative to the USB root above.");
                ui.label("Local target should be an absolute folder on this machine.");
                ui.label("Mirror deletes only follows the active source side for that sync direction.");
            });
        });

        if let Some(index) = remove_index {
            self.config.jobs.remove(index);
            self.missing_target_prompt = first_missing_target_prompt(&self.config);
        }
        if let Some(index) = browse_source_index {
            self.browse_job_source(index);
        }
        if let Some(index) = browse_target_index {
            self.browse_job_target(index);
        }
    }

    fn render_review_step(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.columns(2, |columns| {
            Self::render_card(&mut columns[0], "Review", |ui| {
                ui.strong("Review");
                ui.label("Save this configuration once the paths below look correct.");
                ui.add_space(8.0);
                ui.label(format!("USB root: {}", self.drive_summary()));
                ui.label(format!(
                    "Shadow cache: {}",
                    self.effective_shadow_cache_root().display()
                ));
                ui.label(format!("Jobs: {}", self.config.jobs.len()));
            });

            columns[0].add_space(12.0);

            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(&mut columns[0], |ui| {
                    for (index, job) in self.config.jobs.iter().enumerate() {
                        egui::Frame::group(ui.style())
                            .inner_margin(egui::Margin::same(18))
                            .show(ui, |ui| {
                                ui.strong(format!("Job {}: {}", index + 1, job.name.trim()));
                                ui.label(format!("Folder inside USB root: {}", job.source.trim()));
                                ui.label(format!("Local target: {}", job.target.trim()));
                                let target = PathBuf::from(job.target.trim());
                                if !target.as_os_str().is_empty() && target.is_absolute() {
                                    ui.horizontal(|ui| {
                                        ui.small("Local target status:");
                                        Self::render_path_badge(ui, &target);
                                    });
                                }
                            });
                        ui.add_space(8.0);
                    }
                });

            columns[0].add_space(8.0);
            columns[0].horizontal(|ui| {
                if ui.button("Save").clicked() {
                    match self.validate_and_save() {
                        Ok(()) => {
                            self.status = format!("Saved {}", self.paths.config_file.display());
                        }
                        Err(error) => {
                            self.status = format!("Save failed: {error}");
                            append_wizard_log(&self.paths, &self.status);
                        }
                    }
                }
                if ui.button("Save and Close").clicked() {
                    match self.validate_and_save() {
                        Ok(()) => {
                            append_wizard_log(&self.paths, "Wizard saved and closed");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                        }
                        Err(error) => {
                            self.status = format!("Save failed: {error}");
                            append_wizard_log(&self.paths, &self.status);
                        }
                    }
                }
            });

            Self::render_card(&mut columns[1], "Final checklist", |ui| {
                ui.label("USB root points at the drive or folder you actually want to mirror.");
                ui.label("Job sources sit inside that USB root.");
                ui.label("Local targets are absolute folders on this machine.");
                ui.label("Shadow cache location looks correct.");
            });
            columns[1].add_space(12.0);
            Self::render_card(&mut columns[1], "Save result", |ui| {
                ui.label("Use `Save` if you want to stay in the wizard.");
                ui.label("Use `Save and Close` if the config looks final.");
                ui.label("If validation fails, the status line at the top will show the exact field problem.");
            });
        });
    }
}

fn first_missing_target_prompt(config: &AppConfig) -> Option<MissingTargetPrompt> {
    let first_job = config.jobs.first()?;
    let target_path = PathBuf::from(first_job.target.trim());
    if target_path.as_os_str().is_empty() || target_path.exists() || !target_path.is_absolute() {
        return None;
    }

    Some(MissingTargetPrompt {
        job_index: 0,
        target_path,
    })
}

fn normalize_optional_string(value: &mut Option<String>) {
    match value.take() {
        Some(current) => {
            let trimmed = current.trim();
            if trimmed.is_empty() {
                *value = None;
            } else {
                *value = Some(trimmed.to_string());
            }
        }
        None => *value = None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_relative(parts: &[&str]) -> String {
        parts.join(std::path::MAIN_SEPARATOR_STR)
    }

    #[test]
    fn prepare_recovery_rewrites_invalid_json_and_keeps_backup() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths {
            app_dir: root.path().join("app"),
            config_file: root.path().join("app").join("config.json"),
            manifest_file: root.path().join("app").join("manifest.json"),
            log_file: root.path().join("app").join("sync.log"),
            shadow_root: root.path().join("app").join("shadow"),
        };
        fs::create_dir_all(&paths.app_dir).unwrap();
        fs::write(&paths.config_file, "{ bad json").unwrap();

        let context = prepare_recovery_context(&paths, "failed to parse").unwrap();

        assert!(context.recovered_default);
        assert!(context.recovery_backup_path.is_some());
        assert_eq!(
            fs::read_to_string(&paths.config_file).unwrap(),
            default_config_template()
        );
        assert_eq!(
            fs::read_to_string(context.recovery_backup_path.unwrap()).unwrap(),
            "{ bad json"
        );
    }

    #[test]
    fn normalize_optional_string_clears_blank_values() {
        let mut value = Some("   ".to_string());
        normalize_optional_string(&mut value);
        assert_eq!(value, None);
    }

    #[test]
    fn normalize_optional_string_trims_non_blank_values() {
        let mut value = Some("  C:\\cache  ".to_string());
        normalize_optional_string(&mut value);
        assert_eq!(value, Some("C:\\cache".to_string()));
    }

    #[test]
    fn first_missing_target_prompt_detects_missing_first_job_target() {
        let temp = tempfile::tempdir().unwrap();
        let config = AppConfig {
            jobs: vec![JobConfig {
                name: "Documents".to_string(),
                source: "Backups/Documents".to_string(),
                target: temp.path().join("missing-target").display().to_string(),
                mirror_deletes: true,
            }],
            ..AppConfig::default()
        };

        let prompt = first_missing_target_prompt(&config).unwrap();
        assert_eq!(prompt.job_index, 0);
    }

    #[test]
    fn first_missing_target_prompt_ignores_existing_target() {
        let existing = tempfile::tempdir().unwrap();
        let config = AppConfig {
            jobs: vec![JobConfig {
                name: "Documents".to_string(),
                source: "Backups/Documents".to_string(),
                target: existing.path().display().to_string(),
                mirror_deletes: true,
            }],
            ..AppConfig::default()
        };

        assert!(first_missing_target_prompt(&config).is_none());
    }

    #[test]
    fn normalize_job_source_text_accepts_nested_relative_windows_path() {
        let value =
            normalize_job_source_text(r"Projects\Folder_Alpha", Some(Path::new(r"S:\"))).unwrap();
        assert_eq!(value, expected_relative(&["Projects", "Folder_Alpha"]));
    }

    #[test]
    fn normalize_job_source_text_converts_absolute_path_under_root() {
        #[cfg(target_os = "windows")]
        let (input, root) = (
            PathBuf::from(r"S:\Projects\Folder_Alpha"),
            PathBuf::from(r"S:\"),
        );
        #[cfg(not(target_os = "windows"))]
        let (input, root) = (
            PathBuf::from("/Volumes/UsbRoot/Projects/Folder_Alpha"),
            PathBuf::from("/Volumes/UsbRoot"),
        );

        let value = normalize_job_source_text(&input.display().to_string(), Some(&root)).unwrap();
        assert_eq!(value, expected_relative(&["Projects", "Folder_Alpha"]));
    }

    #[test]
    fn normalize_job_source_from_path_rejects_folder_outside_root() {
        #[cfg(target_os = "windows")]
        let (path, root) = (Path::new(r"T:\Other\Folder"), Path::new(r"S:\"));
        #[cfg(not(target_os = "windows"))]
        let (path, root) = (Path::new("/Volumes/Other/Folder"), Path::new("/Volumes/UsbRoot"));

        let result = normalize_job_source_from_path(path, root);
        assert!(result.is_err());
    }
}
