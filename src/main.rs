#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod platform;
mod single_instance;
mod sync_engine;
mod update;
mod watcher;
mod wizard;
fn main() -> anyhow::Result<()> {
    let paths = config::AppPaths::discover()?;

    if wizard::maybe_run_from_args(&paths)? {
        return Ok(());
    }

    paths.ensure_layout()?;

    platform::configure_process();

    let _instance_guard = match single_instance::ensure_single_instance()? {
        Some(guard) => guard,
        None => return Ok(()),
    };

    app::run()
}
