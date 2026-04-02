#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod single_instance;
mod sync_engine;
mod watcher;
mod wizard;
mod windows_util;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    windows_util::configure_process_dpi();

    let _instance_guard = match single_instance::ensure_single_instance()? {
        Some(guard) => guard,
        None => return Ok(()),
    };

    app::run()
}

#[cfg(not(target_os = "windows"))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("usb_mirror_sync currently targets Windows only.");
}
