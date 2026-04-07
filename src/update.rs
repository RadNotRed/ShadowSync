use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::config::AppPaths;

const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/RadNotRed/ShadowSync/releases/latest";
pub const RELEASES_PAGE_URL: &str = "https://github.com/RadNotRed/ShadowSync/releases";
const UPDATE_CACHE_FILE: &str = "update-state.json";
const AUTOMATIC_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub release_url: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckOutcome {
    pub manual: bool,
    pub state: UpdateCheckState,
}

#[derive(Debug, Clone)]
pub enum UpdateCheckState {
    Available(UpdateInfo),
    UpToDate,
    Error(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CachedUpdateState {
    #[serde(default)]
    last_checked_at: u64,
    #[serde(default)]
    latest_version: Option<String>,
    #[serde(default)]
    release_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

pub fn load_cached_available_update(paths: &AppPaths, current_version: &str) -> Option<UpdateInfo> {
    let state = read_cached_state(paths).ok()?;
    let version = state.latest_version?;
    let release_url = state.release_url?;

    is_newer_version(&version, current_version).then_some(UpdateInfo {
        version,
        release_url,
    })
}

pub fn should_check_automatically(paths: &AppPaths) -> bool {
    let Ok(state) = read_cached_state(paths) else {
        return true;
    };

    let last_checked = UNIX_EPOCH + Duration::from_secs(state.last_checked_at);
    SystemTime::now()
        .duration_since(last_checked)
        .map(|elapsed| elapsed >= AUTOMATIC_CHECK_INTERVAL)
        .unwrap_or(true)
}

pub fn check_for_updates(
    paths: &AppPaths,
    current_version: &str,
    manual: bool,
) -> UpdateCheckOutcome {
    match fetch_latest_release(current_version) {
        Ok(Some(info)) => {
            let _ = write_cached_state(
                paths,
                CachedUpdateState {
                    last_checked_at: current_unix_timestamp(),
                    latest_version: Some(info.version.clone()),
                    release_url: Some(info.release_url.clone()),
                },
            );
            UpdateCheckOutcome {
                manual,
                state: UpdateCheckState::Available(info),
            }
        }
        Ok(None) => {
            let _ = write_cached_state(
                paths,
                CachedUpdateState {
                    last_checked_at: current_unix_timestamp(),
                    latest_version: None,
                    release_url: None,
                },
            );
            UpdateCheckOutcome {
                manual,
                state: UpdateCheckState::UpToDate,
            }
        }
        Err(error) => UpdateCheckOutcome {
            manual,
            state: UpdateCheckState::Error(error.to_string()),
        },
    }
}

fn fetch_latest_release(current_version: &str) -> Result<Option<UpdateInfo>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build update client")?;
    let release = client
        .get(GITHUB_LATEST_RELEASE_API)
        .header("User-Agent", format!("ShadowSync/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("failed to contact GitHub Releases")?
        .error_for_status()
        .context("GitHub Releases responded with an error")?
        .json::<GitHubRelease>()
        .context("failed to parse the GitHub release response")?;

    let version = normalize_tag(&release.tag_name);
    if !is_newer_version(&version, current_version) {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        version,
        release_url: release.html_url,
    }))
}

fn normalize_tag(tag_name: &str) -> String {
    tag_name.trim().trim_start_matches('v').to_string()
}

fn is_newer_version(candidate: &str, current: &str) -> bool {
    match (Version::parse(candidate), Version::parse(current)) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => candidate != current,
    }
}

fn read_cached_state(paths: &AppPaths) -> Result<CachedUpdateState> {
    let path = cache_file_path(paths);
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_cached_state(paths: &AppPaths, state: CachedUpdateState) -> Result<()> {
    let path = cache_file_path(paths);
    let serialized =
        serde_json::to_string(&state).context("failed to serialize cached update state")?;
    fs::write(&path, serialized).with_context(|| format!("failed to write {}", path.display()))
}

fn cache_file_path(paths: &AppPaths) -> PathBuf {
    paths.app_dir.join(UPDATE_CACHE_FILE)
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_semver_is_detected() {
        assert!(is_newer_version("0.2.0", "0.1.9"));
        assert!(!is_newer_version("0.1.1", "0.1.1"));
    }

    #[test]
    fn leading_v_is_trimmed_from_tags() {
        assert_eq!(normalize_tag("v1.2.3"), "1.2.3");
        assert_eq!(normalize_tag("1.2.3"), "1.2.3");
    }
}
