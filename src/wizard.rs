use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::config::{AppPaths, default_config_template};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SCRIPT_FILE_NAME: &str = "setup_wizard.ps1";

#[derive(Debug, Default, Clone)]
pub struct WizardLaunchContext {
    pub error_message: Option<String>,
    pub recovery_backup_path: Option<PathBuf>,
    pub recovered_default: bool,
}

pub fn open_setup_wizard(paths: &AppPaths) -> Result<()> {
    open_setup_wizard_with_context(paths, &WizardLaunchContext::default())
}

pub fn open_setup_wizard_with_context(
    paths: &AppPaths,
    context: &WizardLaunchContext,
) -> Result<()> {
    let script_path = paths.app_dir.join(SCRIPT_FILE_NAME);
    fs::write(&script_path, include_str!("../assets/setup_wizard.ps1"))
        .with_context(|| format!("failed to write {}", script_path.display()))?;

    let mut command = Command::new("powershell.exe");
    command.creation_flags(CREATE_NO_WINDOW);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]);
    command.arg(&script_path);
    command.arg("-ConfigPath");
    command.arg(&paths.config_file);

    if let Some(error_message) = context.error_message.as_ref() {
        command.arg("-ErrorMessage");
        command.arg(error_message);
    }
    if let Some(recovery_backup_path) = context.recovery_backup_path.as_ref() {
        command.arg("-RecoveryBackupPath");
        command.arg(recovery_backup_path);
    }
    if context.recovered_default {
        command.arg("-RecoveredDefault");
    }

    command
        .spawn()
        .with_context(|| format!("failed to launch {}", script_path.display()))?;

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
