use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDCANCEL, IDRETRY, MB_DEFBUTTON2, MB_ICONWARNING, MB_RETRYCANCEL, MB_SYSTEMMODAL, MB_TOPMOST,
    MessageBoxW, SetProcessDPIAware,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub enum AlreadyRunningChoice {
    Restart,
    Cancel,
}

pub fn drive_root(letter: char) -> PathBuf {
    PathBuf::from(format!("{}:\\", letter.to_ascii_uppercase()))
}

pub fn drive_present(letter: char) -> bool {
    drive_root(letter).exists()
}

pub fn wait_for_drive_state(letter: char, expected_present: bool, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if drive_present(letter) == expected_present {
            return true;
        }
        thread::sleep(Duration::from_millis(150));
    }
    drive_present(letter) == expected_present
}

pub fn eject_drive(letter: char) -> Result<()> {
    let drive_token = format!("{}:", letter.to_ascii_uppercase());
    let powershell = format!(
        "$shell = New-Object -ComObject Shell.Application; \
         $folder = $shell.Namespace(17); \
         if ($null -eq $folder) {{ throw 'Shell namespace unavailable.' }}; \
         $item = $folder.ParseName('{drive_token}'); \
         if ($null -eq $item) {{ throw 'Drive not found.' }}; \
         $item.InvokeVerb('Eject')"
    );

    let powershell_attempt = run_hidden_command(
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
        && wait_for_drive_state(letter, false, Duration::from_secs(10))
    {
        return Ok(());
    }

    let mountvol_attempt = run_hidden_command("mountvol.exe", &[&drive_token, "/p"]);
    match mountvol_attempt {
        Ok(()) if wait_for_drive_state(letter, false, Duration::from_secs(10)) => Ok(()),
        Ok(()) => bail!(
            "the drive was dismounted but still appears present; Windows may still be holding the device"
        ),
        Err(error) => Err(error.context(
            "failed to eject the drive using the Shell API and the mountvol fallback",
        )),
    }
}

pub fn open_in_explorer(path: &Path) -> Result<()> {
    let mut command = Command::new("explorer.exe");
    command.creation_flags(CREATE_NO_WINDOW);

    if path.is_dir() {
        command.arg(path);
    } else {
        command.arg(format!("/select,{}", path.display()));
    }

    command
        .spawn()
        .with_context(|| format!("failed to open {}", path.display()))?;
    Ok(())
}

pub fn open_text_file(path: &Path) -> Result<()> {
    Command::new("notepad.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .arg(path)
        .spawn()
        .with_context(|| format!("failed to open {}", path.display()))?;
    Ok(())
}

pub fn configure_process_dpi() {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            SetProcessDPIAware();
        }
    }
}

pub fn show_already_running_prompt() -> AlreadyRunningChoice {
    let body = "USB Mirror Sync is already running.\n\nPress Retry to restart the running copy, or Cancel to leave it alone.";
    let title = "USB Mirror Sync";
    let response = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            to_wide_null(body).as_ptr(),
            to_wide_null(title).as_ptr(),
            MB_ICONWARNING | MB_RETRYCANCEL | MB_DEFBUTTON2 | MB_TOPMOST | MB_SYSTEMMODAL,
        )
    };

    match response {
        IDRETRY => AlreadyRunningChoice::Restart,
        IDCANCEL => AlreadyRunningChoice::Cancel,
        _ => AlreadyRunningChoice::Cancel,
    }
}

pub fn terminate_matching_processes(current_exe: &Path, current_pid: u32) -> Result<()> {
    let current_exe = escape_for_single_quotes(&current_exe.display().to_string());
    let command = format!(
        "$target = '{current_exe}'; \
         $self = {current_pid}; \
         Get-CimInstance Win32_Process | \
         Where-Object {{ $_.ProcessId -ne $self -and $_.ExecutablePath -eq $target }} | \
         ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}"
    );

    run_hidden_command(
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

pub fn sleep_short(duration: Duration) {
    thread::sleep(duration);
}

pub fn to_wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn run_hidden_command(program: &str, args: &[&str]) -> Result<()> {
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

fn escape_for_single_quotes(value: &str) -> String {
    value.replace('\'', "''")
}
