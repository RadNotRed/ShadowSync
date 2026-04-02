use std::env;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex};

use crate::windows_util;

const INSTANCE_MUTEX_NAME: &str = "Local\\UsbMirrorSync.SingleInstance";

pub fn ensure_single_instance() -> Result<Option<SingleInstanceGuard>> {
    if let Some(guard) = SingleInstanceGuard::try_acquire()? {
        return Ok(Some(guard));
    }

    match windows_util::show_already_running_prompt() {
        windows_util::AlreadyRunningChoice::Restart => {
            restart_running_copy()?;
            windows_util::sleep_short(Duration::from_millis(300));
            SingleInstanceGuard::wait_and_acquire(Duration::from_secs(6))
                .map(Some)
                .with_context(|| {
                    "the existing copy was asked to restart, but the single-instance lock did not clear"
                })
        }
        windows_util::AlreadyRunningChoice::Cancel => Ok(None),
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

pub struct SingleInstanceGuard {
    _handle: OwnedHandle,
}

impl SingleInstanceGuard {
    fn try_acquire() -> Result<Option<Self>> {
        let name = windows_util::to_wide_null(INSTANCE_MUTEX_NAME);
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if handle.is_null() {
            bail!("failed to create the single-instance mutex");
        }

        let last_error = unsafe { GetLastError() };
        if last_error == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS {
            unsafe {
                CloseHandle(handle);
            }
            Ok(None)
        } else {
            Ok(Some(Self {
                _handle: OwnedHandle(handle),
            }))
        }
    }

    fn wait_and_acquire(timeout: Duration) -> Result<Self> {
        let start = std::time::Instant::now();
        loop {
            if let Some(guard) = Self::try_acquire()? {
                return Ok(guard);
            }
            if start.elapsed() >= timeout {
                break;
            }
            windows_util::sleep_short(Duration::from_millis(250));
        }
        bail!("another instance is still active")
    }
}

fn restart_running_copy() -> Result<()> {
    let current_exe = env::current_exe().context("failed to resolve the current executable path")?;
    let current_pid = std::process::id();
    windows_util::terminate_matching_processes(&current_exe, current_pid)
}
