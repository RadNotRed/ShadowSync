use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use filetime::{FileTime, set_file_mtime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::config::{
    AppPaths, ResolvedConfig, ResolvedJob, append_log, rel_path_string, slash_path_to_native,
};
use crate::platform;

const MANIFEST_VERSION: u32 = 1;
const DRIVE_MARKER_DIRECTORY: &str = ".usb-mirror-sync";
const DRIVE_MARKER_FILE: &str = "state.json";

#[derive(Debug, Clone)]
pub struct SyncReport {
    pub copied_files: usize,
    pub deleted_files: usize,
    pub skipped_files: usize,
    pub bytes_written: u64,
    pub full_resync: bool,
    pub drive_ejected: bool,
}

#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub current_job: String,
    pub job_index: usize,
    pub job_count: usize,
    pub operations_done: usize,
    pub operations_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current_path: Option<String>,
    pub phase: SyncPhase,
}

#[derive(Debug, Clone, Copy)]
pub enum SyncPhase {
    Planning,
    Copying,
    Deleting,
    Finalizing,
}

impl SyncReport {
    pub fn summary(&self) -> String {
        format!(
            "copied {}, deleted {}, skipped {}, wrote {:.2} MB{}{}",
            self.copied_files,
            self.deleted_files,
            self.skipped_files,
            self.bytes_written as f64 / (1024.0 * 1024.0),
            if self.full_resync { ", full-resync" } else { "" },
            if self.drive_ejected { ", ejected" } else { "" }
        )
    }

    pub fn has_activity(&self) -> bool {
        self.copied_files > 0
            || self.deleted_files > 0
            || self.bytes_written > 0
            || self.full_resync
            || self.drive_ejected
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Manifest {
    #[serde(default = "manifest_version")]
    version: u32,
    #[serde(default)]
    drive_label: Option<String>,
    #[serde(default)]
    last_sync_token: Option<String>,
    #[serde(default)]
    jobs: BTreeMap<String, JobManifest>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct JobManifest {
    #[serde(default)]
    files: BTreeMap<String, FileRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileRecord {
    size: u64,
    modified_millis: i64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DriveMarker {
    version: u32,
    sync_token: String,
    updated_millis: i64,
}

#[derive(Debug, Clone)]
struct SourceEntry {
    relative: String,
    absolute: PathBuf,
    size: u64,
    modified_millis: i64,
}

#[derive(Debug, Default)]
struct JobPlan {
    entries: Vec<SourceEntry>,
    copies: Vec<CopyAction>,
    deletions: Vec<String>,
    next_files: BTreeMap<String, FileRecord>,
    skipped: usize,
}

impl JobPlan {
    fn total_operations(&self) -> usize {
        self.copies.len() + self.deletions.len()
    }

    fn total_copy_bytes(&self) -> u64 {
        self.copies.iter().map(|copy| copy.source.size).sum()
    }
}

#[derive(Debug, Default)]
struct LocalPlan {
    copies: Vec<SourceEntry>,
    deletions: Vec<String>,
    skipped: usize,
}

impl LocalPlan {
    fn total_operations(&self) -> usize {
        self.copies.len() + self.deletions.len()
    }

    fn total_copy_bytes(&self) -> u64 {
        self.copies.iter().map(|copy| copy.size).sum()
    }
}

#[derive(Debug, Clone)]
struct CopyAction {
    source: SourceEntry,
    known_hash: Option<String>,
}

#[allow(dead_code)]
pub fn run_sync(config: &ResolvedConfig, paths: &AppPaths) -> Result<SyncReport> {
    run_sync_with_progress(config, paths, |_| {})
}

#[allow(dead_code)]
pub fn run_sync_to_usb(config: &ResolvedConfig, paths: &AppPaths) -> Result<SyncReport> {
    run_sync_to_usb_with_progress(config, paths, |_| {})
}

pub fn run_sync_with_progress<F>(
    config: &ResolvedConfig,
    paths: &AppPaths,
    mut progress: F,
) -> Result<SyncReport>
where
    F: FnMut(SyncProgress),
{
    if !platform::drive_present(&config.drive_root) {
        bail!("drive {} is not mounted", config.drive_label);
    }

    fs::create_dir_all(&config.cache.shadow_root).with_context(|| {
        format!(
            "failed to create shadow root {}",
            config.cache.shadow_root.display()
        )
    })?;

    let mut manifest = load_manifest(paths)?;
    let marker = load_drive_marker(&config.drive_root)?;
    let marker_matches = manifest.last_sync_token.is_some()
        && marker
            .as_ref()
            .map(|marker| Some(marker.sync_token.as_str()) == manifest.last_sync_token.as_deref())
            .unwrap_or(false);
    let force_copy_all = !marker_matches;

    let mut planned_jobs = Vec::with_capacity(config.jobs.len());
    let mut total_operations = 0usize;
    let mut total_copy_bytes = 0u64;
    for job in &config.jobs {
        let previous_job = manifest.jobs.remove(&job.name).unwrap_or_default();
        let shadow_plan = plan_job(job, &previous_job, config, force_copy_all)?;
        let local_plan = plan_local_sync(job, &shadow_plan.entries)?;
        total_operations += shadow_plan.total_operations() + local_plan.total_operations();
        total_copy_bytes += shadow_plan.total_copy_bytes() + local_plan.total_copy_bytes();
        planned_jobs.push(PlannedJob {
            job,
            shadow_plan,
            local_plan,
        });
    }

    progress(SyncProgress {
        current_job: planned_jobs
            .first()
            .map(|planned| planned.job.name.clone())
            .unwrap_or_else(|| "Sync".to_string()),
        job_index: 0,
        job_count: config.jobs.len(),
        operations_done: 0,
        operations_total: total_operations,
        bytes_done: 0,
        bytes_total: total_copy_bytes,
        current_path: None,
        phase: SyncPhase::Planning,
    });

    let mut copied_files = 0usize;
    let mut deleted_files = 0usize;
    let mut skipped_files = 0usize;
    let mut bytes_written = 0u64;

    let mut next_manifest = Manifest {
        version: MANIFEST_VERSION,
        drive_label: Some(config.drive_label.clone()),
        last_sync_token: None,
        jobs: BTreeMap::new(),
    };

    let mut progress_state = ProgressState {
        operations_done: 0,
        operations_total: total_operations,
        bytes_done: 0,
        bytes_total: total_copy_bytes,
    };

    for (job_index, planned) in planned_jobs.into_iter().enumerate() {
        let shadow_outcome = execute_shadow_plan(
            planned.job,
            &planned.shadow_plan,
            config,
            job_index,
            &mut progress_state,
            &mut progress,
        )?;
        let local_outcome = execute_local_plan(
            planned.job,
            &planned.local_plan,
            config,
            job_index,
            &mut progress_state,
            &mut progress,
        )?;

        copied_files += shadow_outcome.copied_files + local_outcome.copied_files;
        deleted_files += shadow_outcome.deleted_files + local_outcome.deleted_files;
        skipped_files += planned.shadow_plan.skipped + planned.local_plan.skipped;
        bytes_written += shadow_outcome.bytes_written + local_outcome.bytes_written;

        next_manifest.jobs.insert(
            planned.job.name.clone(),
            JobManifest {
                files: shadow_outcome.next_files,
            },
        );
    }

    let sync_token = new_sync_token();
    next_manifest.last_sync_token = Some(sync_token.clone());
    save_manifest(paths, &next_manifest)?;
    progress(SyncProgress {
        current_job: "Finalizing".to_string(),
        job_index: config.jobs.len(),
        job_count: config.jobs.len(),
        operations_done: progress_state.operations_done,
        operations_total: progress_state.operations_total,
        bytes_done: progress_state.bytes_done,
        bytes_total: progress_state.bytes_total,
        current_path: None,
        phase: SyncPhase::Finalizing,
    });
    save_drive_marker(
        &config.drive_root,
        &DriveMarker {
            version: MANIFEST_VERSION,
            sync_token,
            updated_millis: unix_millis(SystemTime::now())?,
        },
    )?;

    let mut drive_ejected = false;
    if config.eject_after_sync {
        platform::eject_drive(&config.drive_root).with_context(|| {
            format!("failed to eject drive {}", config.drive_label)
        })?;
        drive_ejected = true;
        if config.cache.shadow_copy && config.cache.clear_shadow_on_eject {
            clear_shadow_cache(config)?;
        }
    }

    let report = SyncReport {
        copied_files,
        deleted_files,
        skipped_files,
        bytes_written,
        full_resync: force_copy_all,
        drive_ejected,
    };
    if report.has_activity() {
        append_log(paths, format!("Sync completed: {}", report.summary()));
    }
    Ok(report)
}

pub fn run_sync_to_usb_with_progress<F>(
    config: &ResolvedConfig,
    paths: &AppPaths,
    mut progress: F,
) -> Result<SyncReport>
where
    F: FnMut(SyncProgress),
{
    if !platform::drive_present(&config.drive_root) {
        bail!("drive {} is not mounted", config.drive_label);
    }

    fs::create_dir_all(&config.cache.shadow_root).with_context(|| {
        format!(
            "failed to create shadow root {}",
            config.cache.shadow_root.display()
        )
    })?;

    let mut manifest = load_manifest(paths)?;
    let marker = load_drive_marker(&config.drive_root)?;
    let marker_matches = manifest.last_sync_token.is_some()
        && marker
            .as_ref()
            .map(|marker| Some(marker.sync_token.as_str()) == manifest.last_sync_token.as_deref())
            .unwrap_or(false);
    let force_copy_all = !marker_matches;

    let mut planned_jobs = Vec::with_capacity(config.jobs.len());
    let mut total_operations = 0usize;
    let mut total_copy_bytes = 0u64;
    for job in &config.jobs {
        let previous_job = manifest.jobs.remove(&job.name).unwrap_or_default();
        let shadow_plan = plan_local_to_shadow(job, &previous_job, force_copy_all)?;
        let usb_plan = plan_usb_sync(job, &config.drive_root, &shadow_plan.entries)?;
        total_operations += shadow_plan.total_operations() + usb_plan.total_operations();
        total_copy_bytes += shadow_plan.total_copy_bytes() + usb_plan.total_copy_bytes();
        planned_jobs.push(PlannedPushJob {
            job,
            shadow_plan,
            usb_plan,
        });
    }

    progress(SyncProgress {
        current_job: planned_jobs
            .first()
            .map(|planned| planned.job.name.clone())
            .unwrap_or_else(|| "Sync".to_string()),
        job_index: 0,
        job_count: config.jobs.len(),
        operations_done: 0,
        operations_total: total_operations,
        bytes_done: 0,
        bytes_total: total_copy_bytes,
        current_path: None,
        phase: SyncPhase::Planning,
    });

    let mut copied_files = 0usize;
    let mut deleted_files = 0usize;
    let mut skipped_files = 0usize;
    let mut bytes_written = 0u64;

    let mut next_manifest = Manifest {
        version: MANIFEST_VERSION,
        drive_label: Some(config.drive_label.clone()),
        last_sync_token: None,
        jobs: BTreeMap::new(),
    };

    let mut progress_state = ProgressState {
        operations_done: 0,
        operations_total: total_operations,
        bytes_done: 0,
        bytes_total: total_copy_bytes,
    };

    for (job_index, planned) in planned_jobs.into_iter().enumerate() {
        let shadow_outcome = execute_shadow_plan(
            planned.job,
            &planned.shadow_plan,
            config,
            job_index,
            &mut progress_state,
            &mut progress,
        )?;
        let usb_outcome = execute_usb_plan(
            planned.job,
            &planned.usb_plan,
            config,
            job_index,
            &mut progress_state,
            &mut progress,
        )?;

        copied_files += shadow_outcome.copied_files + usb_outcome.copied_files;
        deleted_files += shadow_outcome.deleted_files + usb_outcome.deleted_files;
        skipped_files += planned.shadow_plan.skipped + planned.usb_plan.skipped;
        bytes_written += shadow_outcome.bytes_written + usb_outcome.bytes_written;

        next_manifest.jobs.insert(
            planned.job.name.clone(),
            JobManifest {
                files: shadow_outcome.next_files,
            },
        );
    }

    let sync_token = new_sync_token();
    next_manifest.last_sync_token = Some(sync_token.clone());
    save_manifest(paths, &next_manifest)?;
    progress(SyncProgress {
        current_job: "Finalizing".to_string(),
        job_index: config.jobs.len(),
        job_count: config.jobs.len(),
        operations_done: progress_state.operations_done,
        operations_total: progress_state.operations_total,
        bytes_done: progress_state.bytes_done,
        bytes_total: progress_state.bytes_total,
        current_path: None,
        phase: SyncPhase::Finalizing,
    });
    save_drive_marker(
        &config.drive_root,
        &DriveMarker {
            version: MANIFEST_VERSION,
            sync_token,
            updated_millis: unix_millis(SystemTime::now())?,
        },
    )?;

    let mut drive_ejected = false;
    if config.eject_after_sync {
        platform::eject_drive(&config.drive_root).with_context(|| {
            format!("failed to eject drive {}", config.drive_label)
        })?;
        drive_ejected = true;
        if config.cache.shadow_copy && config.cache.clear_shadow_on_eject {
            clear_shadow_cache(config)?;
        }
    }

    let report = SyncReport {
        copied_files,
        deleted_files,
        skipped_files,
        bytes_written,
        full_resync: force_copy_all,
        drive_ejected,
    };
    if report.has_activity() {
        append_log(paths, format!("Sync completed: {}", report.summary()));
    }
    Ok(report)
}

struct PlannedJob<'a> {
    job: &'a ResolvedJob,
    shadow_plan: JobPlan,
    local_plan: LocalPlan,
}

struct PlannedPushJob<'a> {
    job: &'a ResolvedJob,
    shadow_plan: JobPlan,
    usb_plan: LocalPlan,
}

#[derive(Debug, Default)]
struct ProgressState {
    operations_done: usize,
    operations_total: usize,
    bytes_done: u64,
    bytes_total: u64,
}

pub fn clear_shadow_cache(config: &ResolvedConfig) -> Result<()> {
    if !config.cache.shadow_copy {
        return Ok(());
    }

    for job in &config.jobs {
        if job.shadow_dir.exists() {
            fs::remove_dir_all(&job.shadow_dir)
                .with_context(|| format!("failed to remove {}", job.shadow_dir.display()))?;
        }
    }
    Ok(())
}

fn plan_job(
    job: &ResolvedJob,
    previous: &JobManifest,
    config: &ResolvedConfig,
    force_copy_all: bool,
) -> Result<JobPlan> {
    let entries = scan_source_entries(job, &config.drive_root)?;
    plan_shadow_sync(
        entries,
        &job.shadow_dir,
        previous,
        job.mirror_deletes,
        force_copy_all,
    )
}

fn plan_local_to_shadow(
    job: &ResolvedJob,
    previous: &JobManifest,
    force_copy_all: bool,
) -> Result<JobPlan> {
    let entries = scan_path_entries(&job.local_target)?;
    plan_shadow_sync(
        entries,
        &job.shadow_dir,
        previous,
        job.mirror_deletes,
        force_copy_all,
    )
}

fn plan_shadow_sync(
    entries: Vec<SourceEntry>,
    shadow_dir: &Path,
    previous: &JobManifest,
    mirror_deletes: bool,
    force_copy_all: bool,
) -> Result<JobPlan> {
    let current_paths: HashSet<String> = entries.iter().map(|entry| entry.relative.clone()).collect();
    let mut plan = JobPlan {
        entries,
        ..JobPlan::default()
    };

    for entry in &plan.entries {
        let previous_record = previous.files.get(&entry.relative);
        let shadow_path = shadow_dir.join(slash_path_to_native(&entry.relative));

        if !force_copy_all
            && let Some(record) = previous_record
            && record.size == entry.size
            && record.modified_millis == entry.modified_millis
            && metadata_matches_path(&shadow_path, entry.size, entry.modified_millis)
        {
            plan.next_files.insert(entry.relative.clone(), record.clone());
            plan.skipped += 1;
            continue;
        }

        let known_hash = previous_record.and_then(|record| {
            if record.size == entry.size && record.modified_millis == entry.modified_millis {
                Some(record.sha256.clone())
            } else {
                None
            }
        });

        plan.copies.push(CopyAction {
            source: entry.clone(),
            known_hash,
        });
    }

    if mirror_deletes {
        for existing in previous.files.keys() {
            if !current_paths.contains(existing) {
                plan.deletions.push(existing.clone());
            }
        }
    }

    Ok(plan)
}

fn plan_local_sync(job: &ResolvedJob, desired_entries: &[SourceEntry]) -> Result<LocalPlan> {
    plan_destination_sync(&job.local_target, job.mirror_deletes, desired_entries)
}

fn plan_usb_sync(
    job: &ResolvedJob,
    drive_root: &Path,
    desired_entries: &[SourceEntry],
) -> Result<LocalPlan> {
    let usb_target_root = job.usb_source_root(drive_root);
    plan_destination_sync(&usb_target_root, job.mirror_deletes, desired_entries)
}

fn plan_destination_sync(
    destination_root: &Path,
    mirror_deletes: bool,
    desired_entries: &[SourceEntry],
) -> Result<LocalPlan> {
    let desired_paths: HashSet<String> = desired_entries
        .iter()
        .map(|entry| entry.relative.clone())
        .collect();
    let mut plan = LocalPlan::default();

    for entry in desired_entries {
        let destination_path = destination_root.join(slash_path_to_native(&entry.relative));
        if metadata_matches_path(&destination_path, entry.size, entry.modified_millis) {
            plan.skipped += 1;
        } else {
            plan.copies.push(entry.clone());
        }
    }

    if mirror_deletes && destination_root.exists() {
        for entry in scan_path_entries(destination_root)? {
            if !desired_paths.contains(&entry.relative) {
                plan.deletions.push(entry.relative);
            }
        }
    }

    Ok(plan)
}

struct ExecutionOutcome {
    copied_files: usize,
    deleted_files: usize,
    bytes_written: u64,
    next_files: BTreeMap<String, FileRecord>,
}

fn execute_shadow_plan(
    job: &ResolvedJob,
    plan: &JobPlan,
    config: &ResolvedConfig,
    job_index: usize,
    progress_state: &mut ProgressState,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<ExecutionOutcome> {
    let mut next_files = plan.next_files.clone();
    let mut copied_files = 0usize;
    let mut deleted_files = 0usize;
    let mut bytes_written = 0u64;
    let mut created_directories = DirectoryCache::default();

    for action in &plan.copies {
        let relative_native = slash_path_to_native(&action.source.relative);
        let shadow_destination = job.shadow_dir.join(&relative_native);

        let hash = copy_with_optional_hash(
            &action.source.absolute,
            &shadow_destination,
            action.source.modified_millis,
            action.known_hash.as_deref(),
            &mut created_directories,
        )?;

        next_files.insert(
            action.source.relative.clone(),
            FileRecord {
                size: action.source.size,
                modified_millis: action.source.modified_millis,
                sha256: hash,
            },
        );
        copied_files += 1;
        bytes_written += action.source.size;
        progress_state.operations_done += 1;
        progress_state.bytes_done += action.source.size;
        progress(SyncProgress {
            current_job: job.name.clone(),
            job_index: job_index + 1,
            job_count: config.jobs.len(),
            operations_done: progress_state.operations_done,
            operations_total: progress_state.operations_total,
            bytes_done: progress_state.bytes_done,
            bytes_total: progress_state.bytes_total,
            current_path: Some(action.source.relative.clone()),
            phase: SyncPhase::Copying,
        });
    }

    for relative in &plan.deletions {
        let relative_native = slash_path_to_native(relative);
        let shadow_target = job.shadow_dir.join(&relative_native);
        remove_file_if_exists(&shadow_target)?;
        prune_empty_ancestors(shadow_target.parent(), &job.shadow_dir)?;

        deleted_files += 1;
        progress_state.operations_done += 1;
        progress(SyncProgress {
            current_job: job.name.clone(),
            job_index: job_index + 1,
            job_count: config.jobs.len(),
            operations_done: progress_state.operations_done,
            operations_total: progress_state.operations_total,
            bytes_done: progress_state.bytes_done,
            bytes_total: progress_state.bytes_total,
            current_path: Some(relative.clone()),
            phase: SyncPhase::Deleting,
        });
    }

    Ok(ExecutionOutcome {
        copied_files,
        deleted_files,
        bytes_written,
        next_files,
    })
}

fn execute_local_plan(
    job: &ResolvedJob,
    plan: &LocalPlan,
    config: &ResolvedConfig,
    job_index: usize,
    progress_state: &mut ProgressState,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<ExecutionOutcome> {
    execute_destination_plan(
        &job.name,
        &job.shadow_dir,
        &job.local_target,
        plan,
        config,
        job_index,
        progress_state,
        progress,
    )
}

fn execute_usb_plan(
    job: &ResolvedJob,
    plan: &LocalPlan,
    config: &ResolvedConfig,
    job_index: usize,
    progress_state: &mut ProgressState,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<ExecutionOutcome> {
    let usb_target_root = job.usb_source_root(&config.drive_root);
    execute_destination_plan(
        &job.name,
        &job.shadow_dir,
        &usb_target_root,
        plan,
        config,
        job_index,
        progress_state,
        progress,
    )
}

fn execute_destination_plan(
    job_name: &str,
    shadow_dir: &Path,
    destination_root: &Path,
    plan: &LocalPlan,
    config: &ResolvedConfig,
    job_index: usize,
    progress_state: &mut ProgressState,
    progress: &mut impl FnMut(SyncProgress),
) -> Result<ExecutionOutcome> {
    let mut copied_files = 0usize;
    let mut deleted_files = 0usize;
    let mut bytes_written = 0u64;
    let mut created_directories = DirectoryCache::default();

    for entry in &plan.copies {
        let relative_native = slash_path_to_native(&entry.relative);
        let shadow_source = shadow_dir.join(&relative_native);
        let destination = destination_root.join(&relative_native);

        copy_without_hash(
            &shadow_source,
            &destination,
            entry.modified_millis,
            &mut created_directories,
        )?;
        copied_files += 1;
        bytes_written += entry.size;
        progress_state.operations_done += 1;
        progress_state.bytes_done += entry.size;
        progress(SyncProgress {
            current_job: job_name.to_string(),
            job_index: job_index + 1,
            job_count: config.jobs.len(),
            operations_done: progress_state.operations_done,
            operations_total: progress_state.operations_total,
            bytes_done: progress_state.bytes_done,
            bytes_total: progress_state.bytes_total,
            current_path: Some(entry.relative.clone()),
            phase: SyncPhase::Copying,
        });
    }

    for relative in &plan.deletions {
        let relative_native = slash_path_to_native(relative);
        let destination = destination_root.join(&relative_native);
        remove_file_if_exists(&destination)?;
        prune_empty_ancestors(destination.parent(), destination_root)?;
        deleted_files += 1;
        progress_state.operations_done += 1;
        progress(SyncProgress {
            current_job: job_name.to_string(),
            job_index: job_index + 1,
            job_count: config.jobs.len(),
            operations_done: progress_state.operations_done,
            operations_total: progress_state.operations_total,
            bytes_done: progress_state.bytes_done,
            bytes_total: progress_state.bytes_total,
            current_path: Some(relative.clone()),
            phase: SyncPhase::Deleting,
        });
    }

    Ok(ExecutionOutcome {
        copied_files,
        deleted_files,
        bytes_written,
        next_files: BTreeMap::new(),
    })
}

fn scan_source_entries(job: &ResolvedJob, drive_root: &Path) -> Result<Vec<SourceEntry>> {
    let usb_source_root = job.usb_source_root(drive_root);
    scan_path_entries(&usb_source_root)
}

fn scan_path_entries(root: &Path) -> Result<Vec<SourceEntry>> {
    let mut entries = Vec::new();
    if !root.exists() {
        return Ok(entries);
    }

    for entry in WalkDir::new(root).follow_links(false).sort_by_file_name() {
        let entry = entry.with_context(|| format!("failed while scanning {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let absolute = entry.path().to_path_buf();
        let relative_path = absolute
            .strip_prefix(root)
            .with_context(|| format!("failed to trim {}", absolute.display()))?;
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata for {}", absolute.display()))?;

        entries.push(SourceEntry {
            relative: rel_path_string(relative_path)?,
            absolute,
            size: metadata.len(),
            modified_millis: unix_millis(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH))?,
        });
    }

    Ok(entries)
}

fn metadata_matches_path(path: &Path, size: u64, modified_millis: i64) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() != size {
        return false;
    }
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(path_millis) = unix_millis(modified) else {
        return false;
    };
    path_millis == modified_millis
}

#[derive(Debug, Default)]
struct DirectoryCache {
    created: HashSet<PathBuf>,
}

impl DirectoryCache {
    fn ensure_parent_directory(&mut self, path: &Path) -> Result<()> {
        let Some(parent) = path.parent() else {
            bail!("path has no parent: {}", path.display());
        };

        let parent = parent.to_path_buf();
        if self.created.contains(&parent) {
            return Ok(());
        }

        fs::create_dir_all(&parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        self.created.insert(parent);
        Ok(())
    }
}

fn copy_with_optional_hash(
    source: &Path,
    destination: &Path,
    modified_millis: i64,
    known_hash: Option<&str>,
    directories: &mut DirectoryCache,
) -> Result<String> {
    directories.ensure_parent_directory(destination)?;

    if let Some(hash) = known_hash {
        fs::copy(source, destination).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        preserve_modified_time_from_millis(destination, modified_millis)?;
        return Ok(hash.to_owned());
    }

    let mut reader = BufReader::new(
        fs::File::open(source).with_context(|| format!("failed to open {}", source.display()))?,
    );
    let writer = fs::File::create(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    let mut writer = BufWriter::new(writer);

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 256];
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", source.display()))?;
        if read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..read])
            .with_context(|| format!("failed to write {}", destination.display()))?;
        if known_hash.is_none() {
            hasher.update(&buffer[..read]);
        }
    }

    writer
        .flush()
        .with_context(|| format!("failed to flush {}", destination.display()))?;
    let inner = writer
        .into_inner()
        .with_context(|| format!("failed to finish {}", destination.display()))?;
    inner
        .sync_all()
        .with_context(|| format!("failed to flush {}", destination.display()))?;
    preserve_modified_time_from_millis(destination, modified_millis)?;

    Ok(digest_to_hex(hasher.finalize().as_slice()))
}

fn copy_without_hash(
    source: &Path,
    destination: &Path,
    modified_millis: i64,
    directories: &mut DirectoryCache,
) -> Result<()> {
    directories.ensure_parent_directory(destination)?;
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    preserve_modified_time_from_millis(destination, modified_millis)?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

fn prune_empty_ancestors(mut candidate: Option<&Path>, stop_root: &Path) -> Result<()> {
    while let Some(current) = candidate {
        if current == stop_root || current.as_os_str().is_empty() {
            break;
        }

        match fs::remove_dir(current) {
            Ok(()) => {
                candidate = current.parent();
            }
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                candidate = current.parent();
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to prune {}", current.display()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn hash_file(path: &Path) -> Result<String> {
    let mut file =
        BufReader::new(fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 256];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(digest_to_hex(hasher.finalize().as_slice()))
}

fn preserve_modified_time_from_millis(destination: &Path, modified_millis: i64) -> Result<()> {
    let seconds = modified_millis.div_euclid(1000);
    let nanos = (modified_millis.rem_euclid(1000) as u32) * 1_000_000;
    let file_time = FileTime::from_unix_time(seconds, nanos);
    set_file_mtime(destination, file_time)
        .with_context(|| format!("failed to set modified time for {}", destination.display()))?;
    Ok(())
}

fn load_manifest(paths: &AppPaths) -> Result<Manifest> {
    let raw = fs::read_to_string(&paths.manifest_file)
        .with_context(|| format!("failed to read {}", paths.manifest_file.display()))?;
    if raw.trim().is_empty() || raw.trim() == "{}" {
        return Ok(Manifest::default());
    }
    let manifest = serde_json::from_str::<Manifest>(&raw)
        .with_context(|| format!("failed to parse {}", paths.manifest_file.display()))?;
    Ok(manifest)
}

fn save_manifest(paths: &AppPaths, manifest: &Manifest) -> Result<()> {
    save_json_atomically(&paths.manifest_file, manifest)
}

fn load_drive_marker(drive_root: &Path) -> Result<Option<DriveMarker>> {
    let path = drive_marker_path(drive_root);
    if !path.exists() {
        return Ok(None);
    }

    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let marker = serde_json::from_str::<DriveMarker>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(marker))
}

fn save_drive_marker(drive_root: &Path, marker: &DriveMarker) -> Result<()> {
    let path = drive_marker_path(drive_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    save_json_atomically(&path, marker)
}

fn drive_marker_path(drive_root: &Path) -> PathBuf {
    drive_root.join(DRIVE_MARKER_DIRECTORY).join(DRIVE_MARKER_FILE)
}

fn save_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temp_path = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(value)?;
    fs::write(&temp_path, json).with_context(|| format!("failed to write {}", temp_path.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
    }
    fs::rename(&temp_path, path).with_context(|| format!("failed to update {}", path.display()))?;
    Ok(())
}

fn unix_millis(time: SystemTime) -> Result<i64> {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .with_context(|| "system time was before the Unix epoch")?
        .as_millis();
    i64::try_from(millis).with_context(|| "timestamp overflowed i64")
}

fn new_sync_token() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("sync-{millis}")
}

fn manifest_version() -> u32 {
    MANIFEST_VERSION
}

fn digest_to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mounted_drive_label(path: &Path) -> String {
        path.display().to_string()
    }

    fn make_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn sample_config(root: &Path) -> ResolvedConfig {
        let usb_root = root.join("usb");
        let usb_source = usb_root.join("Docs");
        let target = root.join("target");
        let cache = root.join("shadow");
        fs::create_dir_all(&usb_source).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&cache).unwrap();

        ResolvedConfig {
            drive_label: mounted_drive_label(&usb_root),
            drive_root: usb_root,
            eject_after_sync: false,
            app: crate::config::AppBehavior::default(),
            cache: crate::config::ResolvedCacheConfig {
                shadow_root: cache.clone(),
                shadow_copy: true,
                clear_shadow_on_eject: true,
            },
            jobs: vec![ResolvedJob {
                name: "docs".to_string(),
                usb_source_relative: PathBuf::from("Docs"),
                local_target: target,
                mirror_deletes: true,
                shadow_dir: cache.join("docs"),
            }],
        }
    }

    #[test]
    fn plan_detects_changed_and_deleted_files() {
        let temp = TempDir::new().unwrap();
        let config = sample_config(temp.path());
        let job = &config.jobs[0];
        let usb_source = job.usb_source_root(&config.drive_root);

        make_file(&usb_source.join("keep.txt"), "same");
        make_file(&usb_source.join("changed.txt"), "new");
        let mut directories = DirectoryCache::default();
        let keep_metadata = fs::metadata(usb_source.join("keep.txt")).unwrap();
        copy_without_hash(
            &usb_source.join("keep.txt"),
            &job.shadow_dir.join("keep.txt"),
            unix_millis(keep_metadata.modified().unwrap()).unwrap(),
            &mut directories,
        )
        .unwrap();

        let old_hash = hash_file(&usb_source.join("keep.txt")).unwrap();
        let previous = JobManifest {
            files: BTreeMap::from([
                (
                    "keep.txt".to_string(),
                    FileRecord {
                        size: 4,
                        modified_millis: unix_millis(keep_metadata.modified().unwrap()).unwrap(),
                        sha256: old_hash,
                    },
                ),
                (
                    "gone.txt".to_string(),
                    FileRecord {
                        size: 3,
                        modified_millis: 0,
                        sha256: "abc".to_string(),
                    },
                ),
            ]),
        };

        let plan = plan_job(job, &previous, &config, false).unwrap();
        assert!(!plan.copies.iter().any(|copy| copy.source.relative == "keep.txt"));
        assert!(plan.copies.iter().any(|copy| copy.source.relative == "changed.txt"));
        assert_eq!(plan.deletions, vec!["gone.txt".to_string()]);
        assert!(plan.skipped >= 1);
    }

    #[test]
    fn plan_skips_matching_metadata() {
        let temp = TempDir::new().unwrap();
        let config = sample_config(temp.path());
        let job = &config.jobs[0];
        let usb_source = job.usb_source_root(&config.drive_root);

        let file_path = usb_source.join("same.txt");
        make_file(&file_path, "payload");
        let metadata = fs::metadata(&file_path).unwrap();
        let mut directories = DirectoryCache::default();
        copy_without_hash(
            &file_path,
            &job.shadow_dir.join("same.txt"),
            unix_millis(metadata.modified().unwrap()).unwrap(),
            &mut directories,
        )
        .unwrap();
        let previous = JobManifest {
            files: BTreeMap::from([(
                "same.txt".to_string(),
                FileRecord {
                    size: metadata.len(),
                    modified_millis: unix_millis(metadata.modified().unwrap()).unwrap(),
                    sha256: hash_file(&file_path).unwrap(),
                },
            )]),
        };

        let plan = plan_job(job, &previous, &config, false).unwrap();
        assert_eq!(plan.copies.len(), 0);
        assert_eq!(plan.skipped, 1);
    }

    #[test]
    fn plan_rebuilds_shadow_when_cache_file_is_missing() {
        let temp = TempDir::new().unwrap();
        let config = sample_config(temp.path());
        let job = &config.jobs[0];
        let usb_source = job.usb_source_root(&config.drive_root);

        let file_path = usb_source.join("cached.txt");
        make_file(&file_path, "payload");
        let metadata = fs::metadata(&file_path).unwrap();
        let previous = JobManifest {
            files: BTreeMap::from([(
                "cached.txt".to_string(),
                FileRecord {
                    size: metadata.len(),
                    modified_millis: unix_millis(metadata.modified().unwrap()).unwrap(),
                    sha256: hash_file(&file_path).unwrap(),
                },
            )]),
        };

        let plan = plan_job(job, &previous, &config, false).unwrap();
        assert_eq!(plan.skipped, 0);
        assert_eq!(plan.copies.len(), 1);
        assert_eq!(plan.copies[0].source.relative, "cached.txt");
    }

    #[test]
    fn push_sync_updates_shadow_and_usb_from_local_target() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let paths = AppPaths {
            app_dir: root.join("app"),
            config_file: root.join("app").join("config.json"),
            manifest_file: root.join("app").join("manifest.json"),
            log_file: root.join("app").join("sync.log"),
            shadow_root: root.join("shadow"),
        };
        fs::create_dir_all(&paths.app_dir).unwrap();
        fs::write(&paths.manifest_file, "{}").unwrap();
        fs::write(&paths.log_file, b"").unwrap();

        let config = sample_config(root);
        let job = &config.jobs[0];
        let local_file = job.local_target.join("folder").join("new.txt");
        make_file(&local_file, "hello from local");

        let report = run_sync_to_usb(&config, &paths).unwrap();
        assert!(report.has_activity());

        let usb_file = job
            .usb_source_root(&config.drive_root)
            .join("folder")
            .join("new.txt");
        let shadow_file = job.shadow_dir.join("folder").join("new.txt");
        assert_eq!(fs::read_to_string(usb_file).unwrap(), "hello from local");
        assert_eq!(fs::read_to_string(shadow_file).unwrap(), "hello from local");
    }
}
