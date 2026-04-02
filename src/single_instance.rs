use std::fs::{self, File, OpenOptions};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use fs2::FileExt;

use crate::config::AppPaths;
use crate::platform;

const INSTANCE_LOCK_FILE: &str = "instance.lock";

pub fn ensure_single_instance() -> Result<Option<SingleInstanceGuard>> {
    let paths = AppPaths::discover()?;
    fs::create_dir_all(&paths.app_dir)
        .with_context(|| format!("failed to create {}", paths.app_dir.display()))?;

    if let Some(guard) = SingleInstanceGuard::try_acquire(&paths)? {
        return Ok(Some(guard));
    }

    match platform::show_already_running_prompt() {
        platform::AlreadyRunningChoice::Retry => SingleInstanceGuard::wait_and_acquire(&paths, Duration::from_secs(8))
            .map(Some)
            .with_context(|| "another instance is still active"),
        platform::AlreadyRunningChoice::Cancel => Ok(None),
    }
}

pub struct SingleInstanceGuard {
    file: File,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl SingleInstanceGuard {
    fn try_acquire(paths: &AppPaths) -> Result<Option<Self>> {
        let lock_path = paths.app_dir.join(INSTANCE_LOCK_FILE);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("failed to open {}", lock_path.display()))?;

        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(Self { file })),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error).with_context(|| {
                format!("failed to lock {}", lock_path.display())
            }),
        }
    }

    fn wait_and_acquire(paths: &AppPaths, timeout: Duration) -> Result<Self> {
        let start = std::time::Instant::now();
        loop {
            if let Some(guard) = Self::try_acquire(paths)? {
                return Ok(guard);
            }
            if start.elapsed() >= timeout {
                break;
            }
            platform::sleep_short(Duration::from_millis(250));
        }
        bail!("another instance is still active")
    }
}
