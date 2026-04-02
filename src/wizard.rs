use std::env;
use std::fs;
use std::path::PathBuf;
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
            .with_title("USB Mirror Sync Setup"),
        ..Default::default()
    };

    eframe::run_native(
        "USB Mirror Sync Setup",
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
    close_after_save: bool,
}

impl WizardApp {
    fn load(paths: AppPaths, context: WizardLaunchContext) -> Self {
        let config = fs::read_to_string(&paths.config_file)
            .ok()
            .and_then(|raw| serde_json::from_str::<AppConfig>(raw.strip_prefix('\u{feff}').unwrap_or(&raw)).ok())
            .unwrap_or_default();

        Self {
            paths,
            context,
            config,
            status: "Edit settings, then save the config.".to_string(),
            close_after_save: false,
        }
    }

    fn validate_and_save(&mut self) -> Result<()> {
        let serialized = serde_json::to_string_pretty(&self.config)
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
        egui::TopBottomPanel::top("actions").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    match self.validate_and_save() {
                        Ok(()) => {
                            self.status = format!("Saved {}", self.paths.config_file.display());
                            if self.close_after_save {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        Err(error) => self.status = format!("Save failed: {error}"),
                    }
                }
                if ui.button("Save and Close").clicked() {
                    self.close_after_save = true;
                    match self.validate_and_save() {
                        Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        Err(error) => {
                            self.close_after_save = false;
                            self.status = format!("Save failed: {error}");
                        }
                    }
                }
                if ui.button("Close").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(banner) = self.banner_text() {
                ui.group(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(196, 95, 73), banner);
                });
                ui.add_space(10.0);
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Drive");
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

                ui.separator();
                ui.heading("Behavior");
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

                ui.separator();
                ui.heading("Cache");
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

                ui.separator();
                ui.heading("Comparison");
                ui.checkbox(
                    &mut self.config.compare.hash_on_metadata_change,
                    "Hash files when metadata changes",
                );

                ui.separator();
                ui.horizontal(|ui| {
                    ui.heading("Jobs");
                    if ui.button("Add job").clicked() {
                        self.config.jobs.push(JobConfig::default());
                    }
                });

                let mut remove_index = None;
                let mut browse_source_index = None;
                let mut browse_target_index = None;
                for index in 0..self.config.jobs.len() {
                    let job = &mut self.config.jobs[index];
                    ui.group(|ui| {
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
                        ui.checkbox(&mut job.mirror_deletes, "Mirror deletes");
                    });
                    ui.add_space(8.0);
                }

                if let Some(index) = remove_index {
                    self.config.jobs.remove(index);
                }
                if let Some(index) = browse_source_index {
                    self.browse_job_source(index);
                }
                if let Some(index) = browse_target_index {
                    self.browse_job_target(index);
                }
            });
        });
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
}
