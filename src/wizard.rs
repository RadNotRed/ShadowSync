use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use eframe::egui;
use rfd::FileDialog;
use serde_json::Value;

use crate::config::{AppConfig, AppPaths, JobConfig, default_config_template, load_config, rel_path_string};
use crate::platform;

const WIZARD_FLAG: &str = "--wizard";

#[derive(Debug, Default, Clone)]
pub struct WizardLaunchContext {
    pub error_message: Option<String>,
    pub recovery_backup_path: Option<PathBuf>,
    pub recovered_default: bool,
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

    let mut context = WizardLaunchContext::default();
    while let Some(argument) = args.next() {
        if let Some(value) = argument.strip_prefix("--error-message=") {
            context.error_message = Some(value.to_string());
        } else if let Some(value) = argument.strip_prefix("--recovery-backup=") {
            context.recovery_backup_path = Some(PathBuf::from(value));
        } else if argument == "--recovered-default" {
            context.recovered_default = true;
        }
    }

    run_setup_wizard(paths.clone(), context)?;
    Ok(true)
}

pub fn open_setup_wizard(paths: &AppPaths) -> Result<()> {
    open_setup_wizard_with_context(paths, &WizardLaunchContext::default())
}

pub fn open_setup_wizard_with_context(
    _paths: &AppPaths,
    context: &WizardLaunchContext,
) -> Result<()> {
    let exe = platform::current_exe()?;
    let mut command = Command::new(exe);
    command.arg(WIZARD_FLAG);

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

    command.spawn().context("failed to launch setup wizard")?;
    Ok(())
}

pub fn prepare_recovery_context(paths: &AppPaths, error_message: &str) -> Result<WizardLaunchContext> {
    let mut context = WizardLaunchContext {
        error_message: Some(error_message.to_string()),
        ..WizardLaunchContext::default()
    };

    let raw = match fs::read_to_string(&paths.config_file) {
        Ok(raw) => raw,
        Err(_) => {
            fs::write(&paths.config_file, default_config_template())
                .with_context(|| format!("failed to repair {}", paths.config_file.display()))?;
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

fn run_setup_wizard(paths: AppPaths, context: WizardLaunchContext) -> Result<()> {
    let app = WizardApp::load(paths, context);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 760.0])
            .with_min_inner_size([860.0, 620.0])
            .with_title("ShadowSync Setup"),
        ..Default::default()
    };

    eframe::run_native(
        "ShadowSync Setup",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
    .map_err(|error| anyhow::anyhow!("failed to run setup wizard: {error}"))
}

struct WizardApp {
    paths: AppPaths,
    context: WizardLaunchContext,
    config: AppConfig,
    status: String,
    missing_target_prompt: Option<MissingTargetPrompt>,
    current_step: WizardStep,
    theme_applied: Option<egui::Theme>,
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

impl WizardApp {
    fn load(paths: AppPaths, context: WizardLaunchContext) -> Self {
        let config = fs::read_to_string(&paths.config_file)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppConfig>(raw.strip_prefix('\u{feff}').unwrap_or(&raw)).ok())
            .unwrap_or_default();
        let missing_target_prompt = first_missing_target_prompt(&config);

        Self {
            paths,
            context,
            config,
            status: "Use the steps below to set up ShadowSync.".to_string(),
            missing_target_prompt,
            current_step: WizardStep::Welcome,
            theme_applied: None,
        }
    }

    fn validate_and_save(&mut self) -> Result<()> {
        let config = self.normalized_config_for_save();
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
        Ok(())
    }

    fn normalized_config_for_save(&self) -> AppConfig {
        let mut config = self.config.clone();
        normalize_optional_string(&mut config.drive.letter);
        normalize_optional_string(&mut config.drive.path);
        normalize_optional_string(&mut config.cache.root);

        for job in &mut config.jobs {
            job.name = job.name.trim().to_string();
            job.source = job.source.trim().to_string();
            job.target = job.target.trim().to_string();
        }

        config
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
            self.status = "Set the drive path or letter first, then browse a USB source folder.".to_string();
            return;
        };

        let picker = FileDialog::new().set_directory(&root);
        if let Some(folder) = picker.pick_folder() {
            match folder.strip_prefix(&root) {
                Ok(relative) => match rel_path_string(relative) {
                    Ok(value) => self.config.jobs[index].source = value,
                    Err(error) => self.status = format!("Source browse failed: {error}"),
                },
                Err(_) => {
                    self.status = format!(
                        "The selected folder must stay inside the configured drive root {}",
                        root.display()
                    );
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
}

impl eframe::App for WizardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_system_theme(ctx);

        let mut browse_missing_target = None;
        let mut create_missing_target = None;
        let mut dismiss_missing_target = false;

        egui::TopBottomPanel::top("wizard_header")
            .frame(egui::Frame::default().inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("ShadowSync Setup");
                    ui.separator();
                    ui.label(self.status.clone());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                ui.add_space(8.0);
                self.render_stepper(ui);
            });

        egui::TopBottomPanel::bottom("wizard_footer")
            .frame(egui::Frame::default().inner_margin(egui::Margin::same(14)))
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

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(banner) = self.banner_text() {
                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.colored_label(egui::Color32::from_rgb(196, 95, 73), banner);
                    });
                ui.add_space(12.0);
            }

            self.render_overview_hero(ui);
            ui.add_space(14.0);

            egui::ScrollArea::vertical().show(ui, |ui| match self.current_step {
                WizardStep::Welcome => self.render_welcome_step(ui),
                WizardStep::Drive => self.render_drive_step(ui),
                WizardStep::SyncBehavior => self.render_behavior_step(ui),
                WizardStep::Jobs => self.render_jobs_step(ui),
                WizardStep::Review => self.render_review_step(ui, ctx),
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
        visuals.window_corner_radius = egui::CornerRadius::same(14);
        visuals.menu_corner_radius = egui::CornerRadius::same(12);
        visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.active.corner_radius = egui::CornerRadius::same(10);
        visuals.widgets.open.corner_radius = egui::CornerRadius::same(10);
        visuals.panel_fill = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(20, 23, 28)
        } else {
            egui::Color32::from_rgb(245, 248, 252)
        };
        visuals.extreme_bg_color = if matches!(theme, egui::Theme::Dark) {
            egui::Color32::from_rgb(12, 15, 19)
        } else {
            egui::Color32::from_rgb(255, 255, 255)
        };

        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.indent = 16.0;
        ctx.set_style(style);

        self.theme_applied = Some(theme);
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
            WizardStep::Drive => "Tell ShadowSync which USB drive or mount path to watch.",
            WizardStep::SyncBehavior => "Choose how often it watches, syncs, and clears cache data.",
            WizardStep::Jobs => "Point each USB source folder at the local folder you actually use.",
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
        if let Some(letter) = self.config.drive.letter.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            parts.push(format!("letter {}", letter.trim_end_matches(':')));
        }
        if let Some(path) = self.config.drive.path.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            parts.push(format!("mount {}", path));
        }
        if parts.is_empty() {
            "No drive configured yet".to_string()
        } else {
            parts.join(" | ")
        }
    }

    fn render_stepper(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (index, step) in Self::step_sequence().iter().copied().enumerate() {
                let selected = step == self.current_step;
                let label = format!("{}. {}", index + 1, Self::step_title(step));
                let response = ui.selectable_label(selected, label);
                if response.clicked() {
                    self.current_step = step;
                }
            }
        });
    }

    fn render_overview_hero(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.heading(Self::step_title(self.current_step));
                ui.label(Self::step_description(self.current_step));
                ui.add_space(6.0);
                ui.small(format!(
                    "Drive: {}    |    Shadow cache: {}",
                    self.drive_summary(),
                    self.effective_shadow_cache_root().display()
                ));
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

    fn render_welcome_step(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.heading("Get started");
                ui.label("ShadowSync normally pulls from the USB into the shadow cache and then into the local folder you work from.");
                ui.add_space(8.0);
                ui.label("Typical flow:");
                ui.monospace("USB drive  ->  shadow cache  ->  local folder");
                ui.add_space(10.0);
                ui.label("If you enable push-back later, the same cache is reused in the opposite direction.");
            });

        ui.add_space(12.0);

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.strong("What this setup covers");
                ui.label("1. Pick the USB drive or mount path.");
                ui.label("2. Confirm how often ShadowSync should react.");
                ui.label("3. Point a USB folder at a local target folder.");
                ui.label("4. Review the effective paths before saving.");
            });
    }

    fn render_drive_step(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.strong("USB drive");
                ui.label("On Windows you can use a drive letter. On macOS and Linux use the mounted path.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Windows letter");
                    let letter = self.config.drive.letter.get_or_insert_default();
                    ui.text_edit_singleline(letter);
                    ui.label("Mount path");
                    let path = self.config.drive.path.get_or_insert_default();
                    ui.text_edit_singleline(path);
                    if ui.button("Browse").clicked() {
                        self.browse_mount_path();
                    }
                });
                ui.checkbox(&mut self.config.drive.eject_after_sync, "Eject after sync");
            });
    }

    fn render_behavior_step(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
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

        ui.add_space(12.0);

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.strong("Shadow cache");
                ui.checkbox(&mut self.config.cache.shadow_copy, "Enable shadow cache");
                ui.checkbox(
                    &mut self.config.cache.clear_shadow_on_eject,
                    "Clear shadow cache on eject",
                );
                ui.horizontal(|ui| {
                    ui.label("Custom cache root");
                    let cache_root = self.config.cache.root.get_or_insert_default();
                    ui.text_edit_singleline(cache_root);
                    if ui.button("Browse").clicked() {
                        self.browse_cache_root();
                    }
                });
                ui.small(format!(
                    "Current shadow cache: {}",
                    self.effective_shadow_cache_root().display()
                ));
                ui.small("Leave custom cache root blank to use the default app cache folder.");
            });

        ui.add_space(12.0);

        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.strong("Comparison");
                ui.checkbox(
                    &mut self.config.compare.hash_on_metadata_change,
                    "Hash files when metadata changes",
                );
            });
    }

    fn render_jobs_step(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Folder jobs");
                    if ui.button("Add job").clicked() {
                        self.config.jobs.push(JobConfig::default());
                    }
                });
                ui.label("Each job maps a folder on the USB drive to a real local folder on this machine.");
            });

        ui.add_space(12.0);

        let mut remove_index = None;
        let mut browse_source_index = None;
        let mut browse_target_index = None;
        for index in 0..self.config.jobs.len() {
            let job = &mut self.config.jobs[index];
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(18))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(format!("Job {}", index + 1));
                        if ui.button("Remove").clicked() {
                            remove_index = Some(index);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut job.name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("USB source (relative)");
                        ui.text_edit_singleline(&mut job.source);
                        if ui.button("Browse").clicked() {
                            browse_source_index = Some(index);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Local target");
                        ui.text_edit_singleline(&mut job.target);
                        if ui.button("Browse").clicked() {
                            browse_target_index = Some(index);
                        }
                    });
                    ui.horizontal(|ui| {
                        let target_path = PathBuf::from(job.target.trim());
                        ui.small("Target folder:");
                        if !target_path.as_os_str().is_empty() && target_path.is_absolute() {
                            Self::render_path_badge(ui, &target_path);
                        } else {
                            ui.colored_label(egui::Color32::from_rgb(196, 95, 73), "Needs valid path");
                        }
                    });
                    ui.checkbox(&mut job.mirror_deletes, "Mirror deletes");
                });
            ui.add_space(8.0);
        }

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
        egui::Frame::group(ui.style())
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.strong("Review");
                ui.label("Save this configuration once the paths below look correct.");
                ui.add_space(8.0);
                ui.label(format!("Drive: {}", self.drive_summary()));
                ui.label(format!(
                    "Shadow cache: {}",
                    self.effective_shadow_cache_root().display()
                ));
                ui.label(format!("Jobs: {}", self.config.jobs.len()));
            });

        ui.add_space(12.0);

        for (index, job) in self.config.jobs.iter().enumerate() {
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(18))
                .show(ui, |ui| {
                    ui.strong(format!("Job {}: {}", index + 1, job.name.trim()));
                    ui.label(format!("USB source: {}", job.source.trim()));
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

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                match self.validate_and_save() {
                    Ok(()) => {
                        self.status = format!("Saved {}", self.paths.config_file.display());
                    }
                    Err(error) => self.status = format!("Save failed: {error}"),
                }
            }
            if ui.button("Save and Close").clicked() {
                match self.validate_and_save() {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(error) => self.status = format!("Save failed: {error}"),
                }
            }
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
}
