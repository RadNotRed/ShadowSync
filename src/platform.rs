use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

pub enum AlreadyRunningChoice {
    Retry,
    Cancel,
}

pub enum UpdatePromptChoice {
    OpenRelease,
    RemindLater,
    SkipVersion,
}

pub fn configure_process() {
    #[cfg(target_os = "windows")]
    configure_process_windows_dpi();
}

pub fn drive_present(root: &Path) -> bool {
    root.exists()
}

pub fn eject_drive(root: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        return eject_drive_windows(root);
    }
    #[cfg(target_os = "macos")]
    {
        return run_status("diskutil", &["eject", &root.display().to_string()]);
    }
    #[cfg(target_os = "linux")]
    {
        if run_status("gio", &["mount", "-u", &root.display().to_string()]).is_ok() {
            return Ok(());
        }
        return run_status("umount", &[&root.display().to_string()]);
    }
    #[allow(unreachable_code)]
    Err(anyhow!(
        "safe eject is not implemented for this operating system"
    ))
}

pub fn open_path(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        return start_process_windows(&path.display().to_string());
    }
    #[cfg(target_os = "macos")]
    {
        return run_status("open", &[&path.display().to_string()]);
    }
    #[cfg(target_os = "linux")]
    {
        return run_status("xdg-open", &[&path.display().to_string()]);
    }
    #[allow(unreachable_code)]
    Err(anyhow!("opening paths is not implemented for this operating system"))
}

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        return start_process_windows(url);
    }
    #[cfg(target_os = "macos")]
    {
        return run_status("open", &[url]);
    }
    #[cfg(target_os = "linux")]
    {
        return run_status("xdg-open", &[url]);
    }
    #[allow(unreachable_code)]
    Err(anyhow!("opening URLs is not implemented for this operating system"))
}

pub fn open_in_file_manager(path: &Path) -> Result<()> {
    if path.is_dir() {
        open_path(path)
    } else {
        let parent = path.parent().unwrap_or(path);
        open_path(parent)
    }
}

pub fn show_already_running_prompt() -> AlreadyRunningChoice {
    let result = MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("ShadowSync")
        .set_description(
            "ShadowSync is already running.\n\nClose the existing copy and press OK to retry, or Cancel to leave it alone.",
        )
        .set_buttons(MessageButtons::OkCancel)
        .show();

    match result {
        MessageDialogResult::Ok | MessageDialogResult::Yes => AlreadyRunningChoice::Retry,
        _ => AlreadyRunningChoice::Cancel,
    }
}

pub fn prompt_for_update(current_version: &str, latest_version: &str) -> UpdatePromptChoice {
    let description = format!(
        "ShadowSync {latest_version} is available.\n\nYou are running {current_version}.\n\nOpen the GitHub Releases page now, remind later, or skip this version?"
    );
    match MessageDialog::new()
        .set_level(MessageLevel::Info)
        .set_title("ShadowSync Update Available")
        .set_description(description)
        .set_buttons(MessageButtons::YesNoCancelCustom(
            "Open release".to_string(),
            "Later".to_string(),
            "Skip this version".to_string(),
        ))
        .show()
    {
        MessageDialogResult::Yes => UpdatePromptChoice::OpenRelease,
        MessageDialogResult::No => UpdatePromptChoice::RemindLater,
        MessageDialogResult::Cancel => UpdatePromptChoice::SkipVersion,
        MessageDialogResult::Custom(choice) if choice == "Open release" => {
            UpdatePromptChoice::OpenRelease
        }
        MessageDialogResult::Custom(choice) if choice == "Later" => {
            UpdatePromptChoice::RemindLater
        }
        MessageDialogResult::Custom(choice) if choice == "Skip this version" => {
            UpdatePromptChoice::SkipVersion
        }
        _ => UpdatePromptChoice::RemindLater,
    }
}

pub fn show_wizard_loading_indicator(signal_path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let signal_path = powershell_single_quoted(&signal_path.display().to_string());
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             Add-Type -AssemblyName System.Drawing; \
             $signal = '{signal_path}'; \
             $form = New-Object System.Windows.Forms.Form; \
             $form.Text = 'ShadowSync'; \
             $form.StartPosition = 'CenterScreen'; \
             $form.Size = New-Object System.Drawing.Size(340, 176); \
             $form.TopMost = $true; \
             $form.FormBorderStyle = 'FixedDialog'; \
             $form.ControlBox = $false; \
             $form.MinimizeBox = $false; \
             $form.MaximizeBox = $false; \
             $form.BackColor = [System.Drawing.Color]::FromArgb(250, 250, 252); \
             $form.Font = New-Object System.Drawing.Font('Segoe UI', 9); \
             $spinnerIndex = 0; \
             $spinner = New-Object System.Windows.Forms.Panel; \
             $spinner.Size = New-Object System.Drawing.Size(72, 72); \
             $spinner.Location = New-Object System.Drawing.Point(26, 44); \
             $spinner.BackColor = $form.BackColor; \
             $spinner.Add_Paint({{ \
                 param($sender, $eventArgs) \
                 $graphics = $eventArgs.Graphics; \
                 $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias; \
                 $centerX = $sender.Width / 2.0; \
                 $centerY = $sender.Height / 2.0; \
                 $radius = 22.0; \
                 for ($i = 0; $i -lt 12; $i++) {{ \
                     $step = ($i - $script:spinnerIndex + 12) % 12; \
                     $alpha = [Math]::Max(48, 255 - ($step * 18)); \
                     $angle = (([Math]::PI * 2.0) / 12.0) * $i; \
                     $x = $centerX + ([Math]::Cos($angle) * $radius) - 4.0; \
                     $y = $centerY + ([Math]::Sin($angle) * $radius) - 4.0; \
                     $brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb([int]$alpha, 31, 122, 194)); \
                     $graphics.FillEllipse($brush, [float]$x, [float]$y, 8.0, 8.0); \
                     $brush.Dispose(); \
                 }} \
             }}); \
             $title = New-Object System.Windows.Forms.Label; \
             $title.Text = 'Starting Setup Wizard'; \
             $title.AutoSize = $true; \
             $title.Location = New-Object System.Drawing.Point(118, 48); \
             $title.Font = New-Object System.Drawing.Font('Segoe UI', 12, [System.Drawing.FontStyle]::Bold); \
             $label = New-Object System.Windows.Forms.Label; \
             $label.Text = 'Loading ShadowSync settings and recovery details.'; \
             $label.AutoSize = $true; \
             $label.MaximumSize = New-Object System.Drawing.Size(180, 0); \
             $label.Location = New-Object System.Drawing.Point(118, 78); \
             $label.ForeColor = [System.Drawing.Color]::FromArgb(78, 84, 94); \
             $hint = New-Object System.Windows.Forms.Label; \
             $hint.Text = 'This should only take a moment.'; \
             $hint.AutoSize = $true; \
             $hint.Location = New-Object System.Drawing.Point(118, 118); \
             $hint.ForeColor = [System.Drawing.Color]::FromArgb(110, 116, 126); \
             $divider = New-Object System.Windows.Forms.Label; \
             $divider.BorderStyle = 'Fixed3D'; \
             $divider.AutoSize = $false; \
             $divider.Size = New-Object System.Drawing.Size(288, 2); \
             $divider.Location = New-Object System.Drawing.Point(26, 24); \
             $divider.ForeColor = [System.Drawing.Color]::FromArgb(224, 227, 232); \
             $form.Controls.Add($divider); \
             $form.Controls.Add($spinner); \
             $form.Controls.Add($title); \
             $form.Controls.Add($label); \
             $form.Controls.Add($hint); \
             $animate = New-Object System.Windows.Forms.Timer; \
             $animate.Interval = 80; \
             $animate.Add_Tick({{ $script:spinnerIndex = ($script:spinnerIndex + 1) % 12; $spinner.Invalidate(); }}); \
             $animate.Start(); \
             $timer = New-Object System.Windows.Forms.Timer; \
             $timer.Interval = 150; \
             $timer.Add_Tick({{ if (-not (Test-Path -LiteralPath $signal)) {{ $form.Close() }} }}); \
             $timer.Start(); \
             $timeout = New-Object System.Windows.Forms.Timer; \
             $timeout.Interval = 20000; \
             $timeout.Add_Tick({{ $form.Close() }}); \
             $timeout.Start(); \
             [void]$form.ShowDialog()"
        );
        return run_hidden_detached(
            "powershell.exe",
            &[
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ],
        );
    }
    #[allow(unreachable_code)]
    Ok(())
}

pub fn sleep_short(duration: Duration) {
    thread::sleep(duration);
}

pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("failed to resolve the current executable path")
}

#[cfg(target_os = "windows")]
fn configure_process_windows_dpi() {
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            SetProcessDPIAware();
        }
    }
}

#[cfg(target_os = "windows")]
fn eject_drive_windows(root: &Path) -> Result<()> {
    let drive_token = windows_drive_token(root)?;
    let powershell = format!(
        "$shell = New-Object -ComObject Shell.Application; \
         $folder = $shell.Namespace(17); \
         if ($null -eq $folder) {{ throw 'Shell namespace unavailable.' }}; \
         $item = $folder.ParseName('{drive_token}'); \
         if ($null -eq $item) {{ throw 'Drive not found.' }}; \
         $item.InvokeVerb('Eject')"
    );

    let powershell_attempt = run_hidden(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &powershell,
        ],
    );

    if powershell_attempt.is_ok()
        && wait_for_drive_state(root, false, Duration::from_secs(10))
    {
        return Ok(());
    }

    let mountvol_attempt = run_hidden("mountvol.exe", &[&drive_token, "/p"]);
    match mountvol_attempt {
        Ok(()) if wait_for_drive_state(root, false, Duration::from_secs(10)) => Ok(()),
        Ok(()) => anyhow::bail!(
            "the drive was dismounted but still appears present; Windows may still be holding the device"
        ),
        Err(error) => Err(error.context(
            "failed to eject the drive using the Shell API and the mountvol fallback",
        )),
    }
}

#[cfg(target_os = "windows")]
fn windows_drive_token(root: &Path) -> Result<String> {
    let text = root.display().to_string();
    let mut chars = text.chars();
    let letter = chars
        .next()
        .ok_or_else(|| anyhow!("drive root must not be empty"))?;
    let colon = chars
        .next()
        .ok_or_else(|| anyhow!("drive root must look like 'E:\\'"))?;
    if !letter.is_ascii_alphabetic() || colon != ':' {
        anyhow::bail!("drive root must look like 'E:\\'");
    }
    Ok(format!("{}:", letter.to_ascii_uppercase()))
}

#[cfg(target_os = "windows")]
fn wait_for_drive_state(root: &Path, expected_present: bool, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if drive_present(root) == expected_present {
            return true;
        }
        sleep_short(Duration::from_millis(150));
    }
    drive_present(root) == expected_present
}

#[cfg(target_os = "windows")]
fn run_hidden(program: &str, args: &[&str]) -> Result<()> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = Command::new(program)
        .creation_flags(CREATE_NO_WINDOW)
        .args(args)
        .status()
        .with_context(|| format!("failed to start {program}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{program} exited with status {status}"))
    }
}

#[cfg(target_os = "windows")]
fn run_hidden_detached(program: &str, args: &[&str]) -> Result<()> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new(program)
        .creation_flags(CREATE_NO_WINDOW)
        .args(args)
        .spawn()
        .with_context(|| format!("failed to start {program}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn start_process_windows(target: &str) -> Result<()> {
    let command = format!(
        "Start-Process -FilePath '{}'",
        powershell_single_quoted(target)
    );
    run_hidden(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &command,
        ],
    )
}

#[cfg(target_os = "windows")]
fn powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(not(target_os = "windows"))]
fn run_status(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to start {program}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{program} exited with status {status}"))
    }
}
