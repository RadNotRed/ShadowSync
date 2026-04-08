use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    recommended_watcher,
};

use crate::config::ResolvedConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchKind {
    UsbSource,
    LocalTarget,
}

pub struct ChangeWatcher {
    usb_watcher: Option<RecommendedWatcher>,
    local_watcher: Option<RecommendedWatcher>,
}

impl ChangeWatcher {
    pub fn new<F>(
        config: &ResolvedConfig,
        watch_usb: bool,
        watch_local: bool,
        on_event: F,
    ) -> Result<Self>
    where
        F: Fn(WatchKind) + Send + Sync + 'static,
    {
        let callback: Arc<dyn Fn(WatchKind) + Send + Sync> = Arc::new(on_event);

        let usb_watcher = if watch_usb {
            Some(build_watcher(
                usb_watch_roots(config),
                WatchKind::UsbSource,
                callback.clone(),
            )?)
        } else {
            None
        };

        let local_watcher = if watch_local {
            Some(build_watcher(
                local_watch_roots(config),
                WatchKind::LocalTarget,
                callback,
            )?)
        } else {
            None
        };

        Ok(Self {
            usb_watcher,
            local_watcher,
        })
    }

    pub fn is_active(&self) -> bool {
        self.usb_watcher.is_some() || self.local_watcher.is_some()
    }
}

fn build_watcher(
    roots: Vec<PathBuf>,
    kind: WatchKind,
    callback: Arc<dyn Fn(WatchKind) + Send + Sync>,
) -> Result<RecommendedWatcher> {
    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };

        if should_forward_event(&event) {
            callback(kind);
        }
    })
    .context("failed to create filesystem watcher")?;

    watcher
        .configure(NotifyConfig::default().with_poll_interval(Duration::from_secs(2)))
        .context("failed to configure filesystem watcher")?;

    for root in roots {
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", root.display()))?;
    }

    Ok(watcher)
}

fn usb_watch_roots(config: &ResolvedConfig) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for job in &config.jobs {
        let source_root = job.usb_source_root().to_path_buf();
        if let Some(root) = nearest_existing_root(&source_root, Some(&config.drive_root)) {
            push_unique_root(&mut roots, &mut seen, root);
        }
    }
    roots
}

fn local_watch_roots(config: &ResolvedConfig) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();
    for job in &config.jobs {
        if let Some(root) = nearest_existing_root(&job.local_target, None) {
            push_unique_root(&mut roots, &mut seen, root);
        }
    }
    roots
}

fn nearest_existing_root(path: &Path, fallback: Option<&Path>) -> Option<PathBuf> {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }

    fallback.filter(|path| path.exists()).map(Path::to_path_buf)
}

fn push_unique_root(roots: &mut Vec<PathBuf>, seen: &mut HashSet<String>, root: PathBuf) {
    let key = root.to_string_lossy().to_ascii_lowercase();
    if seen.insert(key) {
        roots.push(root);
    }
}

fn should_forward_event(event: &Event) -> bool {
    !matches!(event.kind, EventKind::Access(_))
}
