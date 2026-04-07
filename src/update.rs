use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::config::AppPaths;

const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/RadNotRed/ShadowSync/releases?per_page=10";
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
    Skipped(UpdateInfo),
    UpToDate,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum CachedUpdateStatus {
    Available(UpdateInfo),
    Skipped(UpdateInfo),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CachedUpdateState {
    #[serde(default)]
    last_checked_at: u64,
    #[serde(default)]
    latest_version: Option<String>,
    #[serde(default)]
    release_url: Option<String>,
    #[serde(default)]
    skipped_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
}

pub fn load_cached_update_status(
    paths: &AppPaths,
    current_version: &str,
) -> Option<CachedUpdateStatus> {
    let state = read_cached_state(paths).ok()?;
    let version = state.latest_version?;
    let release_url = state.release_url?;
    let is_skipped = state.skipped_version.as_deref() == Some(version.as_str());

    let info = is_newer_version(&version, current_version).then_some(UpdateInfo {
        version,
        release_url,
    })?;

    Some(if is_skipped {
        CachedUpdateStatus::Skipped(info)
    } else {
        CachedUpdateStatus::Available(info)
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
    let cached_state = read_cached_state(paths).unwrap_or_default();
    let skipped_version = cached_state.skipped_version.clone();

    match fetch_latest_release(current_version) {
        Ok(Some(info)) => {
            let is_skipped = skipped_version.as_deref() == Some(info.version.as_str());
            let _ = write_cached_state(
                paths,
                CachedUpdateState {
                    last_checked_at: current_unix_timestamp(),
                    latest_version: Some(info.version.clone()),
                    release_url: Some(info.release_url.clone()),
                    skipped_version: if is_skipped { skipped_version } else { None },
                },
            );
            UpdateCheckOutcome {
                manual,
                state: if is_skipped {
                    UpdateCheckState::Skipped(info)
                } else {
                    UpdateCheckState::Available(info)
                },
            }
        }
        Ok(None) => {
            let _ = write_cached_state(
                paths,
                CachedUpdateState {
                    last_checked_at: current_unix_timestamp(),
                    latest_version: None,
                    release_url: None,
                    skipped_version,
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

pub fn skip_version(paths: &AppPaths, info: &UpdateInfo) -> Result<()> {
    let mut state = read_cached_state(paths).unwrap_or_default();
    state.last_checked_at = current_unix_timestamp();
    state.latest_version = Some(info.version.clone());
    state.release_url = Some(info.release_url.clone());
    state.skipped_version = Some(info.version.clone());
    write_cached_state(paths, state)
}

fn fetch_latest_release(current_version: &str) -> Result<Option<UpdateInfo>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build update client")?;
    let releases = client
        .get(GITHUB_RELEASES_API)
        .header("User-Agent", format!("ShadowSync/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("failed to contact GitHub Releases")?
        .error_for_status()
        .context("GitHub Releases responded with an error")?
        .json::<Vec<GitHubRelease>>()
        .context("failed to parse the GitHub release response")?;

    let current = Version::parse(current_version)
        .with_context(|| format!("failed to parse current version {current_version}"))?;
    let Some(release) = select_update_candidate(releases, &current) else {
        return Ok(None);
    };

    let version = normalize_tag(&release.tag_name);
    Ok(Some(UpdateInfo {
        version,
        release_url: release.html_url,
    }))
}

fn normalize_tag(tag_name: &str) -> String {
    tag_name.trim().trim_start_matches('v').to_string()
}

fn select_update_candidate(
    releases: Vec<GitHubRelease>,
    current_version: &Version,
) -> Option<GitHubRelease> {
    releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = Version::parse(&normalize_tag(&release.tag_name)).ok()?;
            Some((version, release))
        })
        .filter(|(candidate, _)| should_offer_release(candidate, current_version))
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, release)| release)
}

fn should_offer_release(candidate: &Version, current: &Version) -> bool {
    if current.pre.is_empty() && !candidate.pre.is_empty() {
        return false;
    }
    candidate > current
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

    #[test]
    fn stable_build_ignores_newer_prerelease_tags() {
        let releases = vec![
            GitHubRelease {
                tag_name: "0.1.2-pre".to_string(),
                html_url: "https://example.com/pre".to_string(),
                draft: false,
            },
            GitHubRelease {
                tag_name: "0.1.1".to_string(),
                html_url: "https://example.com/stable".to_string(),
                draft: false,
            },
        ];

        let selected = select_update_candidate(releases, &Version::parse("0.1.1").unwrap());
        assert!(selected.is_none());
    }

    #[test]
    fn highest_valid_release_is_selected() {
        let releases = vec![
            GitHubRelease {
                tag_name: "0.1.2".to_string(),
                html_url: "https://example.com/one".to_string(),
                draft: false,
            },
            GitHubRelease {
                tag_name: "0.1.4".to_string(),
                html_url: "https://example.com/two".to_string(),
                draft: false,
            },
            GitHubRelease {
                tag_name: "0.1.3".to_string(),
                html_url: "https://example.com/three".to_string(),
                draft: false,
            },
        ];

        let selected = select_update_candidate(releases, &Version::parse("0.1.1").unwrap())
            .expect("a newer release should be selected");
        assert_eq!(selected.tag_name, "0.1.4");
    }
}
