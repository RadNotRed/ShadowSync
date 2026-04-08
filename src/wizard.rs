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

use crate::config::{AppConfig, AppPaths, JobConfig, default_config_template, load_config, sanitize_name};
use crate::platform;

const WIZARD_FLAG: &str = "--wizard";
const LOADING_SIGNAL_FLAG: &str = "--loading-signal=";
const CONTROL_HEIGHT: f32 = 26.0;
const CONTENT_MAX_WIDTH: f32 = 900.0;
const SECTION_CORNER: u8 = 14;
const CARD_CORNER: u8 = 12;
const ACCENT_BAR_WIDTH: f32 = 4.0;

// Phosphor icon font name (registered as a fallback in apply_system_theme)
const ICON_FONT_NAME: &str = "phosphor";
const ICON_FONT_BYTES: &[u8] = include_bytes!("../.github/assets/Phosphor.ttf");

// Phosphor Regular codepoints (Private Use Area)
const ICON_FOLDER_OPEN: &str = "\u{e256}";  // folder-open
const ICON_PLUS:        &str = "\u{e3d4}";  // plus
const ICON_X:           &str = "\u{e4f6}";  // x
const ICON_FLOPPY:      &str = "\u{e248}";  // floppy-disk
const ICON_WARNING:     &str = "\u{e4e0}";  // warning
const ICON_CHECK:       &str = "\u{e182}";  // check
const ICON_CLIPBOARD:   &str = "\u{e198}";  // clipboard-text

// ── Public launch API (unchanged) ──────────────────────────────────────────

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

// ── Internal helpers (unchanged) ───────────────────────────────────────────

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

// ── Window bootstrap ───────────────────────────────────────────────────────

fn run_setup_wizard(paths: AppPaths, context: WizardLaunchContext) -> Result<()> {
    append_wizard_log(&paths, "Wizard UI booting");
    let app = WizardApp::load(paths, context);
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([960.0, 780.0])
        .with_min_inner_size([720.0, 560.0])
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

// ── Palette ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Palette {
    accent: egui::Color32,
    accent_dim: egui::Color32,
    accent_surface: egui::Color32,
    surface: egui::Color32,
    surface_raised: egui::Color32,
    surface_inset: egui::Color32,
    stroke: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    text_muted: egui::Color32,
    success: egui::Color32,
    danger: egui::Color32,
    danger_surface: egui::Color32,
    on_accent: egui::Color32,
    shadow: egui::Color32,
    is_dark: bool,
}

impl Palette {
    fn dark() -> Self {
        Self {
            accent:          egui::Color32::from_rgb(86, 156, 244),
            accent_dim:      egui::Color32::from_rgba_unmultiplied(86, 156, 244, 50),
            accent_surface:  egui::Color32::from_rgba_unmultiplied(86, 156, 244, 18),
            surface:         egui::Color32::from_rgb(18, 22, 28),
            surface_raised:  egui::Color32::from_rgb(26, 32, 40),
            surface_inset:   egui::Color32::from_rgb(22, 27, 34),
            stroke:          egui::Color32::from_rgb(48, 58, 72),
            text_primary:    egui::Color32::from_rgb(230, 237, 243),
            text_secondary:  egui::Color32::from_rgb(168, 182, 200),
            text_muted:      egui::Color32::from_rgb(118, 134, 152),
            success:         egui::Color32::from_rgb(78, 186, 120),
            danger:          egui::Color32::from_rgb(232, 100, 82),
            danger_surface:  egui::Color32::from_rgba_unmultiplied(232, 100, 82, 22),
            on_accent:       egui::Color32::WHITE,
            shadow:          egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            is_dark:         true,
        }
    }

    fn light() -> Self {
        Self {
            accent:          egui::Color32::from_rgb(32, 112, 210),
            accent_dim:      egui::Color32::from_rgba_unmultiplied(32, 112, 210, 48),
            accent_surface:  egui::Color32::from_rgba_unmultiplied(32, 112, 210, 14),
            surface:         egui::Color32::from_rgb(250, 252, 255),
            surface_raised:  egui::Color32::from_rgb(255, 255, 255),
            surface_inset:   egui::Color32::from_rgb(244, 248, 252),
            stroke:          egui::Color32::from_rgb(210, 218, 228),
            text_primary:    egui::Color32::from_rgb(28, 36, 48),
            text_secondary:  egui::Color32::from_rgb(78, 92, 110),
            text_muted:      egui::Color32::from_rgb(128, 142, 158),
            success:         egui::Color32::from_rgb(44, 150, 88),
            danger:          egui::Color32::from_rgb(198, 68, 50),
            danger_surface:  egui::Color32::from_rgba_unmultiplied(198, 68, 50, 18),
            on_accent:       egui::Color32::WHITE,
            shadow:          egui::Color32::from_rgba_unmultiplied(0, 0, 0, 22),
            is_dark:         false,
        }
    }
}

// ── App state ──────────────────────────────────────────────────────────────

struct WizardApp {
    paths: AppPaths,
    context: WizardLaunchContext,
    config: AppConfig,
    status: String,
    status_is_error: bool,
    missing_target_prompt: Option<MissingTargetPrompt>,
    theme_applied: Option<egui::Theme>,
    loading_signal_path: Option<PathBuf>,
    startup_signal_cleared: bool,
    icon_texture: Option<egui::TextureHandle>,
}

#[derive(Debug, Clone)]
struct MissingTargetPrompt {
    job_index: usize,
    target_path: PathBuf,
}

impl WizardApp {
    fn load(paths: AppPaths, context: WizardLaunchContext) -> Self {
        let mut config = fs::read_to_string(&paths.config_file)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppConfig>(raw.strip_prefix('\u{feff}').unwrap_or(&raw)).ok())
            .unwrap_or_default();
        expand_legacy_job_sources_for_editor(&mut config);
        let missing_target_prompt = first_missing_target_prompt(&config);
        let loading_signal_path = context.loading_signal_path.clone();

        Self {
            paths,
            context,
            config,
            status: String::new(),
            status_is_error: false,
            missing_target_prompt,
            theme_applied: None,
            loading_signal_path,
            startup_signal_cleared: false,
            icon_texture: None,
        }
    }

    fn palette(&self) -> Palette {
        match self.theme_applied.unwrap_or(egui::Theme::Dark) {
            egui::Theme::Dark => Palette::dark(),
            egui::Theme::Light => Palette::light(),
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status = msg.into();
        self.status_is_error = is_error;
    }
}

// ── Business logic (unchanged from original) ──────────────────────────────

impl WizardApp {
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
        normalize_optional_string(&mut config.cache.root);

        for job in &mut config.jobs {
            job.name = job.name.trim().to_string();
            job.source = normalize_job_usb_source_text(&job.source)
                .with_context(|| format!("job '{}' USB source is invalid", job.name.trim()))?;
            job.target = job.target.trim().to_string();
            normalize_optional_string(&mut job.shadow_root);
        }

        let drive_root = infer_drive_root_from_jobs(&config.jobs)
            .ok_or_else(|| anyhow::anyhow!("set at least one USB source folder before saving"))?;
        apply_drive_root_to_config(&mut config.drive, &drive_root);
        config.cache.shadow_copy = config.jobs.iter().any(|job| job.use_shadow_cache);

        Ok(config)
    }

    fn current_drive_root(&self) -> Option<PathBuf> {
        self.normalized_config_for_save()
            .ok()
            .and_then(|config| drive_root_from_config(&config))
    }

    #[allow(dead_code)]
    fn browse_mount_path(&mut self) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.config.drive.path = Some(folder.display().to_string());
        }
    }

    fn browse_job_shadow_root(&mut self, index: usize) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.config.jobs[index].shadow_root = Some(folder.display().to_string());
        }
    }

    fn browse_job_source(&mut self, index: usize) {
        let picker = self
            .config
            .jobs
            .get(index)
            .and_then(|job| {
                let current = PathBuf::from(job.source.trim());
                current.parent().map(Path::to_path_buf)
            })
            .map(|folder| FileDialog::new().set_directory(folder))
            .unwrap_or_else(FileDialog::new);

        if let Some(folder) = picker.pick_folder() {
            self.config.jobs[index].source = folder.display().to_string();
            self.set_status(format!("Selected USB source: {}", folder.display()), false);
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
            self.set_status("The local target is blank. Choose a folder first.", true);
            return;
        }
        if !target.is_absolute() {
            self.set_status("The local target must be an absolute path.", true);
            return;
        }

        match fs::create_dir_all(&target) {
            Ok(()) => {
                self.set_status(format!("Created local target: {}", target.display()), false);
                if index == 0 {
                    self.missing_target_prompt = first_missing_target_prompt(&self.config);
                }
            }
            Err(error) => {
                self.set_status(format!("Failed to create {}: {error}", target.display()), true);
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
                self.set_status(format!("Open wizard log failed: {error}"), true);
                append_wizard_log(&self.paths, &self.status);
            }
        }
    }

    fn open_folder_shortcut(&mut self, path: PathBuf, label: &str) {
        if path.as_os_str().is_empty() {
            self.set_status(format!("{label} path is blank."), true);
            append_wizard_log(&self.paths, &self.status);
            return;
        }

        match platform::open_in_file_manager(&path) {
            Ok(()) => {
                self.set_status(format!("Opened {label}: {}", path.display()), false);
                append_wizard_log(&self.paths, &self.status);
            }
            Err(error) => {
                self.set_status(format!("Open {label} failed: {error}"), true);
                append_wizard_log(&self.paths, &self.status);
            }
        }
    }

    fn job_effective_shadow_root(&self, index: usize) -> Option<PathBuf> {
        let job = self.config.jobs.get(index)?;
        if !job.use_shadow_cache {
            return None;
        }

        let shadow_base = job
            .shadow_root
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    self.paths.app_dir.join(path)
                }
            })
            .unwrap_or_else(|| self.effective_shadow_cache_root());

        Some(shadow_base.join(sanitize_name(&job.name)))
    }

    fn open_job_source(&mut self, index: usize) {
        let Some(job) = self.config.jobs.get(index) else {
            return;
        };
        self.open_folder_shortcut(PathBuf::from(job.source.trim()), "USB source");
    }

    fn open_job_target(&mut self, index: usize) {
        let Some(job) = self.config.jobs.get(index) else {
            return;
        };
        self.open_folder_shortcut(PathBuf::from(job.target.trim()), "local target");
    }

    fn open_job_shadow_root(&mut self, index: usize) {
        let Some(path) = self.job_effective_shadow_root(index) else {
            self.set_status("Shadow cache is disabled for this job.", true);
            append_wizard_log(&self.paths, &self.status);
            return;
        };
        self.open_folder_shortcut(path, "shadow cache");
    }

    fn drive_summary(&self) -> String {
        self.current_drive_root()
            .map(|root| root.display().to_string())
            .unwrap_or_else(|| "Not detected — add a USB source folder".to_string())
    }

    fn ensure_icon_texture(&mut self, ctx: &egui::Context) {
        if self.icon_texture.is_some() {
            return;
        }
        let icon_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/wizard_icon_256.png"));
        if let Ok(icon_data) = eframe::icon_data::from_png_bytes(icon_bytes) {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [icon_data.width as usize, icon_data.height as usize],
                &icon_data.rgba,
            );
            self.icon_texture = Some(ctx.load_texture(
                "app_icon",
                image,
                egui::TextureOptions::LINEAR,
            ));
        }
    }
}

// ── Theme application ──────────────────────────────────────────────────────

impl WizardApp {
    fn apply_system_theme(&mut self, ctx: &egui::Context) {
        let Some(theme) = ctx.system_theme() else {
            return;
        };
        if self.theme_applied == Some(theme) {
            return;
        }

        // Register the Phosphor icon font (once)
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            ICON_FONT_NAME.to_owned(),
            egui::FontData::from_static(ICON_FONT_BYTES).into(),
        );
        // Add as fallback to both Proportional and Monospace families
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            family.push(ICON_FONT_NAME.to_owned());
        }
        if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            family.push(ICON_FONT_NAME.to_owned());
        }
        ctx.set_fonts(fonts);

        let pal = match theme {
            egui::Theme::Dark => Palette::dark(),
            egui::Theme::Light => Palette::light(),
        };

        let mut visuals = match theme {
            egui::Theme::Dark => egui::Visuals::dark(),
            egui::Theme::Light => egui::Visuals::light(),
        };

        let cr = egui::CornerRadius::same(8);
        visuals.window_corner_radius = egui::CornerRadius::same(14);
        visuals.menu_corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.noninteractive.corner_radius = cr;
        visuals.widgets.inactive.corner_radius = cr;
        visuals.widgets.hovered.corner_radius = cr;
        visuals.widgets.active.corner_radius = cr;
        visuals.widgets.open.corner_radius = cr;

        visuals.panel_fill = pal.surface;
        visuals.extreme_bg_color = pal.surface_inset;

        visuals.widgets.inactive.bg_fill = pal.surface_raised;
        visuals.widgets.hovered.bg_fill = if pal.is_dark {
            egui::Color32::from_rgb(34, 42, 52)
        } else {
            egui::Color32::from_rgb(248, 250, 254)
        };
        visuals.widgets.active.bg_fill = if pal.is_dark {
            egui::Color32::from_rgb(38, 47, 58)
        } else {
            egui::Color32::from_rgb(238, 244, 250)
        };
        visuals.widgets.open.bg_fill = visuals.widgets.active.bg_fill;

        let stroke = egui::Stroke::new(1.0, pal.stroke);
        visuals.window_stroke = stroke;
        visuals.widgets.noninteractive.bg_stroke = stroke;
        visuals.widgets.inactive.bg_stroke = stroke;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, pal.accent_dim);
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, pal.accent);
        visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, pal.accent);

        // Text colors
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, pal.text_primary);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, pal.text_primary);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, pal.text_primary);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, pal.text_primary);
        visuals.override_text_color = Some(pal.text_primary);

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(14.0, 6.0);
        style.spacing.indent = 16.0;
        style.spacing.interact_size = egui::vec2(40.0, 22.0);
        style.spacing.slider_width = 180.0;
        ctx.set_style(style);

        self.theme_applied = Some(theme);
    }
}

// ── eframe::App ────────────────────────────────────────────────────────────

impl eframe::App for WizardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.clear_startup_signal_if_needed();
        self.apply_system_theme(ctx);
        self.ensure_icon_texture(ctx);
        let pal = self.palette();

        // ── Deferred UI actions ────────────────────────────────────────
        let mut browse_missing_target = None;
        let mut create_missing_target = None;
        let mut dismiss_missing_target = false;

        // ── Header panel ───────────────────────────────────────────────
        egui::TopBottomPanel::top("wizard_header")
            .frame(
                egui::Frame::new()
                    .fill(pal.surface_raised)
                    .stroke(egui::Stroke::new(1.0, pal.stroke))
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // App icon
                    if let Some(tex) = self.icon_texture.as_ref() {
                        let icon_size = egui::vec2(28.0, 28.0);
                        ui.add(egui::Image::new(tex).fit_to_exact_size(icon_size));
                        ui.add_space(4.0);
                    }

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(
                            egui::RichText::new("SHADOWSYNC")
                                .size(13.0)
                                .color(pal.accent)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("Setup")
                                .size(11.0)
                                .color(pal.text_muted),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(format!("{ICON_X}  Close")).size(12.0))
                                    .corner_radius(egui::CornerRadius::same(8)),
                            )
                            .clicked()
                        {
                            append_wizard_log(&self.paths, "Wizard closed from header");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(format!("{ICON_CLIPBOARD}  Log")).size(12.0))
                                    .corner_radius(egui::CornerRadius::same(8)),
                            )
                            .clicked()
                        {
                            self.open_wizard_log();
                        }
                    });
                });
            });

        // ── Footer panel ───────────────────────────────────────────────
        egui::TopBottomPanel::bottom("wizard_footer")
            .frame(
                egui::Frame::new()
                    .fill(pal.surface_raised)
                    .stroke(egui::Stroke::new(1.0, pal.stroke))
                    .inner_margin(egui::Margin::symmetric(20, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("ShadowSync v{}", env!("CARGO_PKG_VERSION")))
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let jobs = self.config.jobs.len();
                        let shadow = self.config.jobs.iter().filter(|j| j.use_shadow_cache).count();
                        ui.label(
                            egui::RichText::new(format!(
                                "{jobs} job{}  ·  {shadow} cached  ·  {} direct",
                                if jobs == 1 { "" } else { "s" },
                                jobs - shadow,
                            ))
                            .size(11.0)
                            .color(pal.text_muted),
                        );
                    });
                });
            });

        // ── Missing target prompt dialog ───────────────────────────────
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

        // ── Central scrollable content ─────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(pal.surface).inner_margin(egui::Margin::same(0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let total = ui.available_width();
                        let content = total.min(CONTENT_MAX_WIDTH);
                        let side = ((total - content) * 0.5).max(16.0);

                        ui.horizontal(|ui| {
                            ui.add_space(side);
                            ui.vertical(|ui| {
                                ui.set_width(content);
                                ui.add_space(20.0);

                                // Recovery banner
                                if let Some(banner) = self.banner_text() {
                                    self.render_recovery_banner(ui, &banner);
                                    ui.add_space(16.0);
                                }

                                // § 1 — USB Drive
                                self.render_drive_section(ui);
                                ui.add_space(20.0);

                                // § 2 — Sync Jobs
                                self.render_jobs_section(ui);
                                ui.add_space(20.0);

                                // § 3 — Sync Behavior
                                self.render_behavior_section(ui);
                                ui.add_space(20.0);

                                // § 4 — Save
                                self.render_save_section(ui, ctx);
                                ui.add_space(28.0);
                            });
                            ui.add_space(side);
                        });
                    });
            });

        // ── Deferred actions ───────────────────────────────────────────
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
            egui::Color32::from_rgb(14, 18, 24)
        } else {
            egui::Color32::from_rgb(244, 248, 252)
        };
        color.to_normalized_gamma_f32()
    }
}

// ── Rendering helpers ──────────────────────────────────────────────────────

impl WizardApp {
    // ── Section frame with numbered header ─────────────────────────────

    fn section_frame(pal: &Palette) -> egui::Frame {
        egui::Frame::new()
            .fill(pal.surface_raised)
            .stroke(egui::Stroke::new(1.0, pal.stroke))
            .corner_radius(egui::CornerRadius::same(SECTION_CORNER))
            .shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: pal.shadow,
            })
            .inner_margin(egui::Margin::same(22))
    }

    fn render_section_number(ui: &mut egui::Ui, pal: &Palette, num: u8, title: &str, subtitle: &str) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            
            let badge_size = 30.0;
            let (badge_rect, _) = ui.allocate_exact_size(
                egui::vec2(badge_size, badge_size),
                egui::Sense::hover(),
            );
            let painter = ui.painter();
            painter.rect_filled(badge_rect, egui::CornerRadius::same(8), pal.accent);
            let num_text = format!("{num}");
            let galley = painter.layout_no_wrap(
                num_text,
                egui::FontId::proportional(15.0),
                pal.on_accent,
            );
            let text_pos = badge_rect.center() - galley.size() / 2.0;
            painter.galley(text_pos, galley, pal.on_accent);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(egui::RichText::new(title).size(17.0).strong().color(pal.text_primary));
                ui.label(egui::RichText::new(subtitle).size(12.0).color(pal.text_muted));
            });
        });
        ui.add_space(14.0);
    }

    // ── Recovery banner ────────────────────────────────────────────────

    fn render_recovery_banner(&self, ui: &mut egui::Ui, text: &str) {
        let pal = self.palette();
        egui::Frame::new()
            .fill(pal.danger_surface)
            .stroke(egui::Stroke::new(1.0, pal.danger))
            .corner_radius(egui::CornerRadius::same(SECTION_CORNER))
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(ICON_WARNING).size(18.0).color(pal.danger));
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("CONFIG RECOVERY")
                            .size(12.0)
                            .color(pal.danger)
                            .strong(),
                    );
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new(text).size(12.0).color(pal.danger));
            });
    }

    // ── § 1  USB Drive ─────────────────────────────────────────────────

    fn render_drive_section(&mut self, ui: &mut egui::Ui) {
        let pal = self.palette();
        Self::section_frame(&pal).show(ui, |ui| {
            Self::render_section_number(
                ui, &pal, 1,
                "USB Drive",
                "Auto-detected from job USB source folders.",
            );

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Detected drive root").size(13.0).strong().color(pal.text_primary));
                ui.add_space(8.0);
                let summary = self.drive_summary();
                let has_root = self.current_drive_root().is_some();
                ui.label(
                    egui::RichText::new(&summary)
                        .size(13.0)
                        .color(if has_root { pal.text_secondary } else { pal.text_muted })
                        .monospace(),
                );
            });

            ui.add_space(10.0);
            ui.checkbox(
                &mut self.config.drive.eject_after_sync,
                egui::RichText::new("Eject USB drive after sync completes").size(13.0),
            );
        });
    }

    // ── § 2  Sync Jobs ─────────────────────────────────────────────────

    fn render_jobs_section(&mut self, ui: &mut egui::Ui) {
        let pal = self.palette();

        Self::section_frame(&pal).show(ui, |ui| {
            Self::render_section_number(
                ui, &pal, 2,
                "Sync Jobs",
                "Each job maps one USB folder to one local folder. Add as many as you need.",
            );

            // Add job button
            ui.horizontal(|ui| {
                let add_btn = ui.add(
                    egui::Button::new(egui::RichText::new(format!("{ICON_PLUS}  Add Job")).size(13.0).strong())
                        .fill(pal.accent_surface)
                        .stroke(egui::Stroke::new(1.0, pal.accent_dim))
                        .corner_radius(egui::CornerRadius::same(8)),
                );
                if add_btn.clicked() {
                    self.config.jobs.push(JobConfig::default());
                }
            });

            if self.config.jobs.is_empty() {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("No jobs configured. Add one to get started.")
                        .size(13.0)
                        .color(pal.text_muted),
                );
            }
        });

        // Job cards rendered outside the section frame so they each stand alone
        let mut remove_index = None;
        let mut browse_source_index = None;
        let mut browse_target_index = None;
        let mut browse_shadow_index = None;
        let mut open_source_index = None;
        let mut open_target_index = None;
        let mut open_shadow_index = None;
        let default_shadow_root = self.effective_shadow_cache_root().display().to_string();

        let pal = self.palette();

        for index in 0..self.config.jobs.len() {
            ui.add_space(10.0);

            let job = &mut self.config.jobs[index];

            // Job card with accent left border
            egui::Frame::new()
                .fill(pal.surface_raised)
                .stroke(egui::Stroke::new(1.0, pal.stroke))
                .corner_radius(egui::CornerRadius::same(CARD_CORNER))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 10,
                    spread: 0,
                    color: pal.shadow,
                })
                .inner_margin(egui::Margin::same(0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Accent bar
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(ACCENT_BAR_WIDTH, 0.0),
                            egui::Sense::hover(),
                        );
                        // We'll paint the bar after layout; use a placeholder here.
                        // Instead, paint into the left margin via the painter.
                        let painter = ui.painter();
                        let full_bar = egui::Rect::from_min_max(
                            egui::pos2(bar_rect.min.x, bar_rect.min.y),
                            egui::pos2(bar_rect.min.x + ACCENT_BAR_WIDTH, ui.max_rect().max.y),
                        );
                        painter.rect_filled(
                            full_bar,
                            egui::CornerRadius {
                                nw: CARD_CORNER as u8,
                                sw: CARD_CORNER as u8,
                                ne: 0,
                                se: 0,
                            },
                            pal.accent,
                        );

                        ui.add_space(14.0);

                        ui.vertical(|ui| {
                            ui.add_space(18.0);

                            // Header row
                            ui.horizontal(|ui| {
                                let job_name = job.name.trim();
                                let display_name = if job_name.is_empty() {
                                    format!("Job {}", index + 1)
                                } else {
                                    format!("Job {} — {}", index + 1, job_name)
                                };
                                ui.label(
                                    egui::RichText::new(display_name)
                                        .size(15.0)
                                        .strong()
                                        .color(pal.text_primary),
                                );

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(18.0);
                                    let remove = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new(format!("{ICON_X}  Remove")).size(11.0).color(pal.danger),
                                        )
                                        .fill(pal.danger_surface)
                                        .stroke(egui::Stroke::new(1.0, pal.danger))
                                        .corner_radius(egui::CornerRadius::same(6)),
                                    );
                                    if remove.clicked() {
                                        remove_index = Some(index);
                                    }
                                });
                            });

                            ui.add_space(12.0);

                            // Name
                            Self::render_field_label(ui, &pal, "Name");
                            ui.add_sized(
                                [ui.available_width().min(400.0), CONTROL_HEIGHT],
                                egui::TextEdit::singleline(&mut job.name)
                                    .hint_text("e.g. Documents, Photos, Project A")
                                    .margin(egui::Margin::symmetric(8, 4)),
                            );
                            ui.add_space(10.0);

                            // USB source
                            Self::render_field_label(ui, &pal, "Sync from USB folder");
                            let source_clicked = Self::render_path_field(ui, &mut job.source, "e.g. E:\\Backups\\Documents");
                            if source_clicked {
                                browse_source_index = Some(index);
                            }
                            ui.add_space(10.0);

                            // Local target
                            Self::render_field_label(ui, &pal, "Sync to local folder");
                            let target_clicked = Self::render_path_field(ui, &mut job.target, "e.g. C:\\Users\\you\\Documents\\Important");
                            if target_clicked {
                                browse_target_index = Some(index);
                            }
                            ui.add_space(12.0);

                            // Checkboxes row
                            ui.horizontal_wrapped(|ui| {
                                ui.checkbox(
                                    &mut job.mirror_deletes,
                                    egui::RichText::new("Mirror deletes").size(13.0),
                                );
                                ui.add_space(12.0);
                                ui.checkbox(
                                    &mut job.use_shadow_cache,
                                    egui::RichText::new("Use shadow cache").size(13.0),
                                );
                            });

                            // Shadow cache sub-section
                            if job.use_shadow_cache {
                                ui.add_space(8.0);
                                egui::Frame::new()
                                    .fill(pal.accent_surface)
                                    .corner_radius(egui::CornerRadius::same(8))
                                    .inner_margin(egui::Margin::same(14))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new("Shadow Cache Settings")
                                                .size(12.0)
                                                .strong()
                                                .color(pal.accent),
                                        );
                                        ui.add_space(6.0);

                                        let shadow_root = job.shadow_root.get_or_insert_default();
                                        let has_custom = !shadow_root.trim().is_empty();

                                        Self::render_field_label(ui, &pal, "Custom cache location (optional)");
                                        let shadow_clicked = Self::render_path_field(ui, shadow_root, "Leave blank for default");
                                        if shadow_clicked {
                                            browse_shadow_index = Some(index);
                                        }
                                        ui.add_space(4.0);
                                        let hint = if has_custom {
                                            format!("Using custom: {}", shadow_root.trim())
                                        } else {
                                            format!("Default: {default_shadow_root}")
                                        };
                                        ui.label(
                                            egui::RichText::new(hint)
                                                .size(11.0)
                                                .color(pal.text_muted),
                                        );
                                    });
                            }

                            ui.add_space(12.0);

                            // Status row
                            ui.horizontal_wrapped(|ui| {
                                let source_path = PathBuf::from(job.source.trim());
                                ui.label(egui::RichText::new("USB:").size(11.0).strong().color(pal.text_muted));
                                if !source_path.as_os_str().is_empty() && source_path.is_absolute() {
                                    Self::render_path_badge(ui, &pal, &source_path);
                                } else {
                                    ui.label(egui::RichText::new("needs path").size(11.0).color(pal.danger));
                                }
                                ui.add_space(10.0);

                                let target_path = PathBuf::from(job.target.trim());
                                ui.label(egui::RichText::new("Local:").size(11.0).strong().color(pal.text_muted));
                                if !target_path.as_os_str().is_empty() && target_path.is_absolute() {
                                    Self::render_path_badge(ui, &pal, &target_path);
                                } else {
                                    ui.label(egui::RichText::new("needs path").size(11.0).color(pal.danger));
                                }
                                ui.add_space(10.0);

                                ui.label(egui::RichText::new("Mode:").size(11.0).strong().color(pal.text_muted));
                                let mode = if job.use_shadow_cache { "Shadow" } else { "Direct" };
                                ui.label(egui::RichText::new(mode).size(11.0).color(pal.accent));
                            });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);

                            // Action buttons row
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add(egui::Button::new(egui::RichText::new(format!("{ICON_FOLDER_OPEN} USB Source")).size(11.0)).corner_radius(egui::CornerRadius::same(6)))
                                    .clicked()
                                {
                                    open_source_index = Some(index);
                                }
                                if ui
                                    .add(egui::Button::new(egui::RichText::new(format!("{ICON_FOLDER_OPEN} Local Target")).size(11.0)).corner_radius(egui::CornerRadius::same(6)))
                                    .clicked()
                                {
                                    open_target_index = Some(index);
                                }
                                if job.use_shadow_cache {
                                    if ui
                                        .add(egui::Button::new(egui::RichText::new(format!("{ICON_FOLDER_OPEN} Cache")).size(11.0)).corner_radius(egui::CornerRadius::same(6)))
                                        .clicked()
                                    {
                                        open_shadow_index = Some(index);
                                    }
                                }
                            });

                            ui.add_space(18.0);
                        });
                    });
                });
        }

        // Deferred mutations
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
        if let Some(index) = browse_shadow_index {
            self.browse_job_shadow_root(index);
        }
        if let Some(index) = open_source_index {
            self.open_job_source(index);
        }
        if let Some(index) = open_target_index {
            self.open_job_target(index);
        }
        if let Some(index) = open_shadow_index {
            self.open_job_shadow_root(index);
        }
    }

    // ── § 3  Sync Behavior ─────────────────────────────────────────────

    fn render_behavior_section(&mut self, ui: &mut egui::Ui) {
        let pal = self.palette();
        Self::section_frame(&pal).show(ui, |ui| {
            Self::render_section_number(
                ui, &pal, 3,
                "Sync Behavior",
                "Global settings that apply across all jobs.",
            );

            // Two-column layout
            ui.columns(2, |cols| {
                // Left column: sync triggers
                cols[0].label(egui::RichText::new("Triggers").size(14.0).strong().color(pal.accent));
                cols[0].add_space(6.0);
                cols[0].checkbox(
                    &mut self.config.app.sync_on_insert,
                    egui::RichText::new("Sync on USB insert").size(13.0),
                );
                cols[0].checkbox(
                    &mut self.config.app.sync_while_mounted,
                    egui::RichText::new("Watch mounted drive for changes").size(13.0),
                );
                cols[0].checkbox(
                    &mut self.config.app.auto_sync_to_usb,
                    egui::RichText::new("Auto-push local changes to USB").size(13.0),
                );
                cols[0].add_space(8.0);
                cols[0].horizontal(|ui| {
                    ui.label(egui::RichText::new("Poll interval").size(13.0).strong());
                    ui.add(
                        egui::DragValue::new(&mut self.config.app.poll_interval_seconds)
                            .range(1..=60)
                            .suffix(" sec"),
                    );
                });

                // Right column: cache & comparison
                cols[1].label(egui::RichText::new("Cache & Comparison").size(14.0).strong().color(pal.accent));
                cols[1].add_space(6.0);
                cols[1].checkbox(
                    &mut self.config.cache.clear_shadow_on_eject,
                    egui::RichText::new("Clear shadow caches on eject").size(13.0),
                );
                cols[1].checkbox(
                    &mut self.config.compare.hash_on_metadata_change,
                    egui::RichText::new("Hash files on metadata change").size(13.0),
                );
                cols[1].add_space(6.0);
                let shadow_jobs = self.config.jobs.iter().filter(|j| j.use_shadow_cache).count();
                let direct_jobs = self.config.jobs.len() - shadow_jobs;
                cols[1].label(
                    egui::RichText::new(format!(
                        "{shadow_jobs} job(s) using cache · {direct_jobs} direct"
                    ))
                    .size(11.0)
                    .color(pal.text_muted),
                );
            });
        });
    }

    // ── § 4  Save ──────────────────────────────────────────────────────

    fn render_save_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = self.palette();
        Self::section_frame(&pal).show(ui, |ui| {
            Self::render_section_number(
                ui, &pal, 4,
                "Save Configuration",
                "Validate and write your config. The status line below will show any problems.",
            );

            ui.horizontal(|ui| {
                let save_btn = ui.add(
                    egui::Button::new(egui::RichText::new(format!("{ICON_FLOPPY}  Save")).size(14.0).strong().color(pal.on_accent))
                        .fill(pal.accent)
                        .stroke(egui::Stroke::new(1.0, pal.accent))
                        .corner_radius(egui::CornerRadius::same(8))
                        .min_size(egui::vec2(120.0, 34.0)),
                );
                if save_btn.clicked() {
                    match self.validate_and_save() {
                        Ok(()) => {
                            self.set_status(
                                format!("Saved to {}", self.paths.config_file.display()),
                                false,
                            );
                        }
                        Err(error) => {
                            self.set_status(format!("Save failed: {error}"), true);
                            append_wizard_log(&self.paths, &self.status);
                        }
                    }
                }

                let save_close_btn = ui.add(
                    egui::Button::new(egui::RichText::new("Save & Close").size(13.0))
                        .corner_radius(egui::CornerRadius::same(8))
                        .min_size(egui::vec2(120.0, 34.0)),
                );
                if save_close_btn.clicked() {
                    match self.validate_and_save() {
                        Ok(()) => {
                            append_wizard_log(&self.paths, "Wizard saved and closed");
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        Err(error) => {
                            self.set_status(format!("Save failed: {error}"), true);
                            append_wizard_log(&self.paths, &self.status);
                        }
                    }
                }
            });

            // Status line
            if !self.status.is_empty() {
                ui.add_space(10.0);
                let color = if self.status_is_error { pal.danger } else { pal.success };
                ui.label(egui::RichText::new(&self.status).size(12.0).color(color));
            }
        });
    }

    // ── Shared rendering primitives ────────────────────────────────────

    fn render_field_label(ui: &mut egui::Ui, pal: &Palette, label: &str) {
        ui.label(egui::RichText::new(label).size(12.0).strong().color(pal.text_secondary));
        ui.add_space(2.0);
    }

    fn render_path_field(ui: &mut egui::Ui, value: &mut String, hint: &str) -> bool {
        let mut browse_clicked = false;
        ui.horizontal(|ui| {
            let btn_width = 80.0;
            let field_width = (ui.available_width() - btn_width - ui.spacing().item_spacing.x - 20.0)
                .max(200.0);
            ui.add_sized(
                [field_width, CONTROL_HEIGHT],
                egui::TextEdit::singleline(value)
                    .hint_text(hint)
                    .margin(egui::Margin::symmetric(8, 4)),
            );
            browse_clicked = ui
                .add_sized(
                    [btn_width, CONTROL_HEIGHT],
                    egui::Button::new(format!("{ICON_FOLDER_OPEN} Browse")),
                )
                .clicked();
        });
        browse_clicked
    }

    fn render_path_badge(ui: &mut egui::Ui, pal: &Palette, path: &Path) {
        let (icon, label, color) = if path.exists() {
            (ICON_CHECK, "Exists", pal.success)
        } else {
            (ICON_X, "Missing", pal.danger)
        };
        egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 25))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(format!("{icon} {label}")).size(10.0).color(color).strong());
            });
    }
}

// ── Path utilities (unchanged) ─────────────────────────────────────────────

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

fn normalize_job_usb_source_text(value: &str) -> Result<String> {
    let trimmed = value.trim();
    anyhow::ensure!(!trimmed.is_empty(), "USB source path must not be empty");
    let path = PathBuf::from(trimmed);
    anyhow::ensure!(
        path.is_absolute(),
        "USB source must be an absolute folder path on the USB drive"
    );
    Ok(path.display().to_string())
}

fn expand_legacy_job_sources_for_editor(config: &mut AppConfig) {
    let Some(root) = drive_root_from_config(config) else {
        return;
    };

    for job in &mut config.jobs {
        let trimmed = job.source.trim();
        if trimmed.is_empty() {
            continue;
        }
        let source = PathBuf::from(trimmed);
        if source.is_absolute() {
            continue;
        }
        job.source = root.join(source).display().to_string();
    }
}

fn infer_drive_root_from_jobs(jobs: &[JobConfig]) -> Option<PathBuf> {
    let mut inferred: Option<PathBuf> = None;
    for job in jobs {
        let source = PathBuf::from(job.source.trim());
        if !source.is_absolute() {
            continue;
        }
        let root = infer_drive_root_from_path(&source)?;
        if let Some(existing) = inferred.as_ref() {
            if !paths_equivalent(existing, &root) {
                return None;
            }
        } else {
            inferred = Some(root);
        }
    }
    inferred
}

fn apply_drive_root_to_config(drive: &mut crate::config::DriveConfig, root: &Path) {
    #[cfg(target_os = "windows")]
    {
        let root_text = root.display().to_string();
        let bytes = root_text.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            drive.letter = Some(root_text[..1].to_string());
            drive.path = None;
            return;
        }
    }
    drive.letter = None;
    drive.path = Some(root.display().to_string());
}

fn infer_drive_root_from_path(path: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut root = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Prefix(prefix) => root.push(prefix.as_os_str()),
                Component::RootDir => {
                    root.push(std::path::MAIN_SEPARATOR.to_string());
                    return Some(root);
                }
                Component::Normal(_) => break,
                _ => {}
            }
        }
        return None;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut components = path.components();
        match components.next() {
            Some(Component::RootDir) => {}
            _ => return None,
        }
        let mut root = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
        let first = components.next()?;
        root.push(component_normal(first)?);
        let second = components.next();
        if let Some(component) = second {
            root.push(component_normal(component)?);
        }
        Some(root)
    }
}

#[cfg(not(target_os = "windows"))]
fn component_normal(component: Component<'_>) -> Option<&std::ffi::OsStr> {
    match component {
        Component::Normal(value) => Some(value),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn paths_equivalent(left: &Path, right: &Path) -> bool {
    left.display()
        .to_string()
        .eq_ignore_ascii_case(&right.display().to_string())
}

#[cfg(not(target_os = "windows"))]
fn paths_equivalent(left: &Path, right: &Path) -> bool {
    left == right
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

// ── Tests (unchanged) ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
                use_shadow_cache: true,
                shadow_root: None,
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
                use_shadow_cache: true,
                shadow_root: None,
            }],
            ..AppConfig::default()
        };

        assert!(first_missing_target_prompt(&config).is_none());
    }

    #[test]
    fn normalize_job_usb_source_text_requires_absolute_path() {
        assert!(normalize_job_usb_source_text("Projects\\Folder_Alpha").is_err());
    }

    #[test]
    fn normalize_job_usb_source_text_accepts_absolute_path() {
        #[cfg(target_os = "windows")]
        let input = PathBuf::from(r"S:\Projects\Folder_Alpha");
        #[cfg(not(target_os = "windows"))]
        let input = PathBuf::from("/Volumes/UsbRoot/Projects/Folder_Alpha");

        let value = normalize_job_usb_source_text(&input.display().to_string()).unwrap();
        assert_eq!(value, input.display().to_string());
    }

    #[test]
    fn infer_drive_root_from_jobs_rejects_mixed_sources() {
        #[cfg(target_os = "windows")]
        let jobs = vec![
            JobConfig {
                source: r"S:\Projects\One".to_string(),
                ..JobConfig::default()
            },
            JobConfig {
                source: r"T:\Projects\Two".to_string(),
                ..JobConfig::default()
            },
        ];
        #[cfg(not(target_os = "windows"))]
        let jobs = vec![
            JobConfig {
                source: "/Volumes/UsbOne/Projects/One".to_string(),
                ..JobConfig::default()
            },
            JobConfig {
                source: "/Volumes/UsbTwo/Projects/Two".to_string(),
                ..JobConfig::default()
            },
        ];

        assert!(infer_drive_root_from_jobs(&jobs).is_none());
    }
}
