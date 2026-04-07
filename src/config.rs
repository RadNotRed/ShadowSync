use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail, ensure};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};

const APP_QUALIFIER: &str = "com";
const APP_ORGANIZATION: &str = "rad";
const APP_NAME: &str = "ShadowSync";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub app_dir: PathBuf,
    pub config_file: PathBuf,
    pub manifest_file: PathBuf,
    pub log_file: PathBuf,
    pub shadow_root: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .context("failed to resolve the local app-data directory")?;
        let app_dir = dirs.data_local_dir().to_path_buf();
        Ok(Self {
            config_file: app_dir.join("config.json"),
            manifest_file: app_dir.join("manifest.json"),
            log_file: app_dir.join("sync.log"),
            shadow_root: app_dir.join("shadow"),
            app_dir,
        })
    }

    pub fn ensure_layout(&self) -> Result<()> {
        self.ensure_app_dir()?;
        ensure_seed_file(&self.shadow_root, None)?;
        ensure_seed_file(&self.config_file, Some(default_config_template().as_bytes()))?;
        ensure_seed_file(&self.manifest_file, Some(b"{}"))?;
        ensure_seed_file(&self.log_file, Some(b""))?;

        Ok(())
    }

    pub fn ensure_wizard_layout(&self) -> Result<()> {
        self.ensure_app_dir()?;
        ensure_seed_file(&self.config_file, Some(default_config_template().as_bytes()))?;

        Ok(())
    }

    fn ensure_app_dir(&self) -> Result<()> {
        ensure_seed_file(&self.app_dir, None)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub drive: DriveConfig,
    #[serde(default)]
    pub app: AppBehavior,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub compare: CompareConfig,
    pub jobs: Vec<JobConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DriveConfig {
    #[serde(default)]
    pub letter: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default = "default_true")]
    pub eject_after_sync: bool,
}

impl Default for DriveConfig {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self {
                letter: Some("E".to_string()),
                path: None,
                eject_after_sync: true,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self {
                letter: None,
                path: Some(default_mount_path().to_string()),
                eject_after_sync: true,
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppBehavior {
    #[serde(default = "default_true")]
    pub sync_on_insert: bool,
    #[serde(default = "default_true")]
    pub sync_while_mounted: bool,
    #[serde(default)]
    pub auto_sync_to_usb: bool,
    #[serde(default = "default_poll_seconds")]
    pub poll_interval_seconds: u64,
}

impl Default for AppBehavior {
    fn default() -> Self {
        Self {
            sync_on_insert: true,
            sync_while_mounted: true,
            auto_sync_to_usb: false,
            poll_interval_seconds: default_poll_seconds(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default = "default_true")]
    pub shadow_copy: bool,
    #[serde(default = "default_true")]
    pub clear_shadow_on_eject: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            root: None,
            shadow_copy: true,
            clear_shadow_on_eject: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompareConfig {
    #[serde(default = "default_true")]
    pub hash_on_metadata_change: bool,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            hash_on_metadata_change: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobConfig {
    pub name: String,
    pub source: String,
    pub target: String,
    #[serde(default = "default_true")]
    pub mirror_deletes: bool,
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            name: "Documents".to_string(),
            source: default_job_source().to_string(),
            target: default_job_target(),
            mirror_deletes: true,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            drive: DriveConfig::default(),
            app: AppBehavior::default(),
            cache: CacheConfig::default(),
            compare: CompareConfig::default(),
            jobs: vec![JobConfig::default()],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub drive_label: String,
    pub drive_root: PathBuf,
    pub eject_after_sync: bool,
    pub app: AppBehavior,
    pub cache: ResolvedCacheConfig,
    pub compare: CompareConfig,
    pub jobs: Vec<ResolvedJob>,
}

#[derive(Debug, Clone)]
pub struct ResolvedCacheConfig {
    pub shadow_root: PathBuf,
    pub shadow_copy: bool,
    pub clear_shadow_on_eject: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedJob {
    pub name: String,
    pub usb_source_relative: PathBuf,
    pub local_target: PathBuf,
    pub mirror_deletes: bool,
    pub shadow_dir: PathBuf,
}

impl ResolvedJob {
    pub fn usb_source_root(&self, drive_root: &Path) -> PathBuf {
        drive_root.join(&self.usb_source_relative)
    }
}

pub fn load_config(paths: &AppPaths) -> Result<ResolvedConfig> {
    let raw = fs::read_to_string(&paths.config_file)
        .with_context(|| format!("failed to read {}", paths.config_file.display()))?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let parsed: AppConfig = serde_json::from_str(raw)
        .with_context(|| format!("failed to parse {}", paths.config_file.display()))?;
    validate_config(parsed, paths)
}

pub fn config_modified(paths: &AppPaths) -> Option<SystemTime> {
    fs::metadata(&paths.config_file)
        .and_then(|meta| meta.modified())
        .ok()
}

pub fn append_log(paths: &AppPaths, line: impl AsRef<str>) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let entry = format!("[{timestamp}] {}\r\n", line.as_ref());
    let _ = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&paths.log_file)
        .and_then(|mut file| std::io::Write::write_all(&mut file, entry.as_bytes()));
}

fn ensure_seed_file(path: &Path, contents: Option<&[u8]>) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    match contents {
        Some(contents) => fs::write(path, contents)
            .with_context(|| format!("failed to create {}", path.display())),
        None => fs::create_dir_all(path)
            .with_context(|| format!("failed to create {}", path.display())),
    }
}

fn validate_config(config: AppConfig, paths: &AppPaths) -> Result<ResolvedConfig> {
    ensure!(
        !config.jobs.is_empty(),
        "config.json must contain at least one sync job"
    );

    let (drive_label, drive_root) = resolve_drive_location(&config.drive)?;

    let poll_interval_seconds = config.app.poll_interval_seconds.clamp(1, 60);
    let app = AppBehavior {
        sync_on_insert: config.app.sync_on_insert,
        sync_while_mounted: config.app.sync_while_mounted,
        auto_sync_to_usb: config.app.auto_sync_to_usb,
        poll_interval_seconds,
    };

    let shadow_root = config
        .cache
        .root
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.shadow_root.clone());
    let shadow_root = normalize_cache_root(shadow_root, &paths.app_dir)?;

    let cache = ResolvedCacheConfig {
        shadow_root,
        shadow_copy: config.cache.shadow_copy,
        clear_shadow_on_eject: config.cache.clear_shadow_on_eject,
    };

    let mut names = HashSet::new();
    let mut jobs = Vec::with_capacity(config.jobs.len());
    for job in config.jobs {
        ensure!(
            names.insert(job.name.clone()),
            "job names must be unique; duplicate found: {}",
            job.name
        );
        ensure!(
            !job.name.trim().is_empty(),
            "job names must not be empty"
        );

        let usb_source_relative = normalize_relative_target(&job.source)
            .with_context(|| format!("job '{}' source must be a relative path on the USB drive", job.name))?;
        let local_target = PathBuf::from(job.target.trim());
        ensure!(
            local_target.is_absolute(),
            "job '{}' target must be an absolute local path",
            job.name
        );
        let shadow_dir = cache.shadow_root.join(sanitize_name(&job.name));

        jobs.push(ResolvedJob {
            name: job.name,
            usb_source_relative,
            local_target,
            mirror_deletes: job.mirror_deletes,
            shadow_dir,
        });
    }

    Ok(ResolvedConfig {
        drive_label,
        drive_root,
        eject_after_sync: config.drive.eject_after_sync,
        app,
        cache,
        compare: config.compare,
        jobs,
    })
}

fn normalize_cache_root(root: PathBuf, app_dir: &Path) -> Result<PathBuf> {
    if root.as_os_str().is_empty() {
        bail!("cache.root must not be empty");
    }

    if root.is_absolute() {
        return Ok(root);
    }

    Ok(app_dir.join(root))
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn normalize_drive_letter(value: &str) -> Result<char> {
    let trimmed = value.trim().trim_end_matches('\\');
    let candidate = trimmed.trim_end_matches(':');
    ensure!(
        candidate.len() == 1,
        "drive.letter must look like 'E' or 'E:'"
    );
    let letter = candidate
        .chars()
        .next()
        .ok_or_else(|| anyhow!("drive.letter must not be empty"))?;
    ensure!(
        letter.is_ascii_alphabetic(),
        "drive.letter must be an ASCII letter"
    );
    Ok(letter.to_ascii_uppercase())
}

fn resolve_drive_location(drive: &DriveConfig) -> Result<(String, PathBuf)> {
    if let Some(path) = drive.path.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        let root = PathBuf::from(path);
        ensure!(root.is_absolute(), "drive.path must be an absolute mount path");
        return Ok((root.display().to_string(), root));
    }

    #[cfg(target_os = "windows")]
    {
        let letter = drive
            .letter
            .as_deref()
            .ok_or_else(|| anyhow!("drive.letter must be set on Windows"))?;
        let drive_letter = normalize_drive_letter(letter)?;
        return Ok((
            format!("{drive_letter}:"),
            PathBuf::from(format!("{drive_letter}:\\")),
        ));
    }

    #[cfg(not(target_os = "windows"))]
    {
        bail!("drive.path must be set to the mounted USB path on this operating system");
    }
}

fn normalize_relative_target(value: &str) -> Result<PathBuf> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "target path must not be empty");
    ensure!(
        !trimmed.starts_with('/') && !trimmed.starts_with('\\'),
        "target path must be relative to the USB drive root"
    );
    ensure!(
        !looks_like_windows_absolute(trimmed),
        "target path must be relative to the USB drive root"
    );

    let parts = trimmed
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    ensure!(
        !parts.is_empty(),
        "target path must not be empty"
    );

    let mut normalized = PathBuf::new();
    for part in parts {
        match part {
            "." => {}
            ".." => bail!("target path must stay inside the USB drive root"),
            _ => normalized.push(part),
        }
    }

    ensure!(
        !normalized.as_os_str().is_empty(),
        "target path must not collapse to an empty value"
    );
    Ok(normalized)
}

fn looks_like_windows_absolute(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn rel_path_string(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            _ => bail!("path contains a non-normal component: {}", path.display()),
        }
    }
    Ok(parts.join("/"))
}

pub fn slash_path_to_native(relative: &str) -> PathBuf {
    relative.split('/').fold(PathBuf::new(), |mut buffer, part| {
        buffer.push(part);
        buffer
    })
}

pub fn sanitize_name(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            sanitized.push(character);
        } else {
            sanitized.push('_');
        }
    }

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "job".to_string()
    } else {
        trimmed.to_string()
    }
}

fn default_true() -> bool {
    true
}

fn default_poll_seconds() -> u64 {
    2
}

pub fn default_config_template() -> String {
    serde_json::to_string_pretty(&AppConfig::default()).unwrap_or_else(|_| "{}".to_string())
}

impl fmt::Display for ResolvedConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "drive {} with {} job(s)",
            self.drive_label,
            self.jobs.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_path_rejects_escape_segments() {
        assert!(normalize_relative_target(r"..\escape").is_err());
        assert!(normalize_relative_target("../escape").is_err());
        assert!(normalize_relative_target("/absolute").is_err());
        assert!(normalize_relative_target(r"C:\absolute").is_err());
    }

    #[test]
    fn target_path_normalizes_cleanly() {
        let path = normalize_relative_target(r"Backups\Docs\2026").unwrap();
        assert_eq!(path, PathBuf::from("Backups").join("Docs").join("2026"));
    }

    #[test]
    fn drive_letter_accepts_common_forms() {
        assert_eq!(normalize_drive_letter("e").unwrap(), 'E');
        assert_eq!(normalize_drive_letter("e:").unwrap(), 'E');
        assert_eq!(normalize_drive_letter("e:\\").unwrap(), 'E');
    }

    #[test]
    fn resolve_drive_location_accepts_mount_path() {
        let drive = DriveConfig {
            letter: None,
            path: Some(default_mount_path().to_string()),
            eject_after_sync: false,
        };
        let (label, root) = resolve_drive_location(&drive).unwrap();
        assert_eq!(label, default_mount_path());
        assert_eq!(root, PathBuf::from(default_mount_path()));
    }

    #[test]
    fn default_job_target_uses_real_path_text() {
        let target = default_job_target();
        assert!(target.ends_with("Important"));
        assert!(!target.contains("YOUR_NAME"));
    }
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
fn default_mount_path() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "/Volumes/USB"
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        "/media/user/USB"
    }
    #[cfg(target_os = "windows")]
    {
        r"C:\"
    }
}

fn default_job_source() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Backups\\Documents"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "Backups/Documents"
    }
}

fn default_job_target() -> String {
    default_documents_dir().join("Important").display().to_string()
}

fn default_documents_dir() -> PathBuf {
    if let Some(user_dirs) = UserDirs::new() {
        if let Some(documents) = user_dirs.document_dir() {
            return documents.to_path_buf();
        }
        return user_dirs.home_dir().join("Documents");
    }

    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\Users\Public\Documents")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Users/Shared/Documents")
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        PathBuf::from("/tmp")
    }
}
