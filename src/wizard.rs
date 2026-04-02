use std::fs;
use std::os::windows::process::CommandExt;
use std::process::Command;

use anyhow::{Context, Result};

use crate::config::AppPaths;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SCRIPT_FILE_NAME: &str = "setup_wizard.ps1";

pub fn open_setup_wizard(paths: &AppPaths) -> Result<()> {
    let script_path = paths.app_dir.join(SCRIPT_FILE_NAME);
    fs::write(&script_path, include_str!("../assets/setup_wizard.ps1"))
        .with_context(|| format!("failed to write {}", script_path.display()))?;

    Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script_path)
        .arg("-ConfigPath")
        .arg(&paths.config_file)
        .spawn()
        .with_context(|| format!("failed to launch {}", script_path.display()))?;

    Ok(())
}
