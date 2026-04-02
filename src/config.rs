use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail, ensure};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const APP_QUALIFIER: &str = "com";
const APP_ORGANIZATION: &str = "rad";
const APP_NAME: &str = "UsbMirrorSync";

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
        fs::create_dir_all(&self.app_dir)
            .with_context(|| format!("failed to create {}", self.app_dir.display()))?;
        fs::create_dir_all(&self.shadow_root)
            .with_context(|| format!("failed to create {}", self.shadow_root.display()))?;

        if !self.config_file.exists() {
            fs::write(&self.config_file, default_config_template())
                .with_context(|| format!("failed to create {}", self.config_file.display()))?;
        }

        if !self.manifest_file.exists() {
            fs::write(&self.manifest_file, "{}")
                .with_context(|| format!("failed to create {}", self.manifest_file.display()))?;
        }

        if !self.log_file.exists() {
            fs::write(&self.log_file, b"")
                .with_context(|| format!("failed to create {}", self.log_file.display()))?;
        }

        Ok(())
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
    pub letter: String,
    #[serde(default = "default_true")]
    pub eject_after_sync: bool,
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

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub drive_letter: char,
    pub drive_root: PathBuf,
    pub eject_after_sync: bool,
    pub app: AppBehavior,
    pub cache: ResolvedCacheConfig,
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

fn validate_config(config: AppConfig, paths: &AppPaths) -> Result<ResolvedConfig> {
    ensure!(
        !config.jobs.is_empty(),
        "config.json must contain at least one sync job"
    );

    let drive_letter = normalize_drive_letter(&config.drive.letter)?;
    let drive_root = PathBuf::from(format!("{drive_letter}:\\"));

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
        drive_letter,
        drive_root,
        eject_after_sync: config.drive.eject_after_sync,
        app,
        cache,
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

fn normalize_relative_target(value: &str) -> Result<PathBuf> {
    let candidate = Path::new(value.trim());
    ensure!(
        !candidate.as_os_str().is_empty(),
        "target path must not be empty"
    );
    ensure!(
        !candidate.is_absolute(),
        "target path must be relative to the USB drive root"
    );

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("target path must stay inside the USB drive root")
            }
        }
    }

    ensure!(
        !normalized.as_os_str().is_empty(),
        "target path must not collapse to an empty value"
    );
    Ok(normalized)
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

pub fn default_config_template() -> &'static str {
    r#"{
  "drive": {
    "letter": "E",
    "eject_after_sync": true
  },
  "app": {
    "sync_on_insert": true,
    "sync_while_mounted": true,
    "auto_sync_to_usb": false,
    "poll_interval_seconds": 2
  },
  "cache": {
    "shadow_copy": true,
    "clear_shadow_on_eject": false
  },
  "compare": {
    "hash_on_metadata_change": true
  },
  "jobs": [
    {
      "name": "Documents",
      "source": "Backups\\Documents",
      "target": "C:\\Users\\YOUR_NAME\\Documents\\Important",
      "mirror_deletes": true
    }
  ]
}
"#
}

impl fmt::Display for ResolvedConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "drive {}: with {} job(s)",
            self.drive_letter,
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
        assert!(normalize_relative_target(r"C:\absolute").is_err());
    }

    #[test]
    fn target_path_normalizes_cleanly() {
        let path = normalize_relative_target(r"Backups\Docs\2026").unwrap();
        assert_eq!(path, PathBuf::from(r"Backups\Docs\2026"));
    }

    #[test]
    fn drive_letter_accepts_common_forms() {
        assert_eq!(normalize_drive_letter("e").unwrap(), 'E');
        assert_eq!(normalize_drive_letter("e:").unwrap(), 'E');
        assert_eq!(normalize_drive_letter("e:\\").unwrap(), 'E');
    }
}
