//! Executes planned records: collisions, dry runs, moves, deletes.

use crate::config::Config;
use crate::hasher;
use crate::models::{ActionRecord, ActionType, CollisionStrategy, RecordStatus, SessionLog};
use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};

/// Progress callback: `(index_1_based, total, record)`.
pub type ExecProgress<'a> = &'a mut dyn FnMut(usize, usize, &ActionRecord);

/// New session id: `sfo_YYYYMMDD_HHMMSS_<6 hex>`.
pub fn new_session_id() -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!(
        "sfo_{}_{}",
        chrono::Local::now().format("%Y%m%d_%H%M%S"),
        &hex[..6]
    )
}

/// Applies records to the filesystem and builds the [`SessionLog`].
#[derive(Debug, Clone)]
pub struct TransactionExecutor<'c> {
    config: &'c Config,
}

impl<'c> TransactionExecutor<'c> {
    pub fn new(config: &'c Config) -> Self {
        Self { config }
    }

    pub fn execute_session(
        &self,
        records: Vec<ActionRecord>,
        source_dir: &Path,
        destination_dir: &Path,
        strategy: &str,
        dry_run: bool,
        progress: Option<ExecProgress<'_>>,
    ) -> SessionLog {
        self.execute_session_ctl(
            records,
            source_dir,
            destination_dir,
            strategy,
            dry_run,
            progress,
            None,
        )
    }

    /// [`Self::execute_session`] with progress and cancellation. Once `ctl`
    /// is cancelled, the remaining records are marked `SKIPPED` ("Cancelled")
    /// without touching their files; everything already done stays recorded
    /// in the returned session, so it can still be undone.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_session_ctl(
        &self,
        records: Vec<ActionRecord>,
        source_dir: &Path,
        destination_dir: &Path,
        strategy: &str,
        dry_run: bool,
        mut progress: Option<ExecProgress<'_>>,
        ctl: Option<&crate::progress::Control>,
    ) -> SessionLog {
        let mut log = SessionLog::new(new_session_id(), paths::now_naive());
        log.source_dir = source_dir.display().to_string();
        log.destination_dir = destination_dir.display().to_string();
        log.strategy = strategy.to_string();
        log.dry_run = dry_run;
        log.total_files = records.len() as u64;

        let policy = self.config.collision_strategy();
        let total = records.len();
        crate::progress::stage(
            ctl,
            if dry_run {
                "Simulating"
            } else {
                "Moving files"
            },
            total as u64,
        );
        for (i, mut rec) in records.into_iter().enumerate() {
            if rec.action_type == ActionType::Skip {
                log.skipped_count += 1;
            } else if crate::progress::cancelled(ctl) {
                rec.status = RecordStatus::Skipped;
                rec.message = "Cancelled before this file was processed".into();
                log.skipped_count += 1;
            } else {
                process_record(&mut rec, policy, dry_run);
                match rec.status {
                    RecordStatus::Success if rec.action_type.is_reversible() => {
                        log.moved_count += 1;
                        log.total_size_bytes += rec.file_size;
                        if rec.action_type == ActionType::Deduplicate {
                            log.duplicate_count += 1;
                            log.space_saved_bytes += rec.file_size;
                        }
                    }
                    RecordStatus::Success if rec.action_type == ActionType::Delete => {
                        log.duplicate_count += 1;
                        log.space_saved_bytes += rec.file_size;
                    }
                    RecordStatus::Skipped => log.skipped_count += 1,
                    _ => {}
                }
            }
            crate::progress::tick(ctl, 1);
            if let Some(cb) = progress.as_mut() {
                cb(i + 1, total, &rec);
            }
            log.records.push(rec);
        }

        if !dry_run && self.config.clean_empty_folders() {
            clean_empty_directories(source_dir, true);
        }
        log.end_time = Some(paths::now_naive());
        log
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn process_record(rec: &mut ActionRecord, policy: CollisionStrategy, dry_run: bool) {
    if !rec.source_path.exists() {
        rec.status = RecordStatus::Failed;
        rec.message = format!(
            "Source file no longer exists: {}",
            rec.source_path.display()
        );
        return;
    }

    if rec.action_type == ActionType::Delete {
        let name = file_name(&rec.source_path);
        if dry_run {
            rec.status = RecordStatus::Success;
            rec.message = format!("[DRY-RUN] Would delete {name}");
        } else {
            match fs::remove_file(&rec.source_path) {
                Ok(()) => {
                    rec.status = RecordStatus::Success;
                    rec.message = format!("Deleted duplicate: {name}");
                }
                Err(e) => {
                    rec.status = RecordStatus::Failed;
                    rec.message = format!("Delete failed: {e}");
                }
            }
        }
        return;
    }

    let Some(mut target) = rec.destination_path.clone() else {
        rec.status = RecordStatus::Failed;
        rec.message = "Destination path is missing".into();
        return;
    };

    if target.exists()
        && !paths::path_eq(&paths::resolve(&target), &paths::resolve(&rec.source_path))
    {
        match resolve_collision(&rec.source_path, &target, policy) {
            Some(t) => {
                target = t;
                rec.destination_path = Some(target.clone());
            }
            None => {
                rec.status = RecordStatus::Skipped;
                rec.message = format!("Skipped due to collision policy at: {}", file_name(&target));
                return;
            }
        }
    }

    if dry_run {
        rec.status = RecordStatus::Success;
        rec.message = format!("[DRY-RUN] Planned action to {}", target.display());
        return;
    }
    let result = target
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| paths::move_file(&rec.source_path, &target));
    match result {
        Ok(()) => {
            rec.status = RecordStatus::Success;
            rec.message = format!("Moved to {}", target.display());
        }
        Err(e) => {
            rec.status = RecordStatus::Failed;
            rec.message = format!("Move failed: {e}");
        }
    }
}

/// `Some(new_target)` to proceed, `None` to skip.
fn resolve_collision(source: &Path, dest: &Path, policy: CollisionStrategy) -> Option<PathBuf> {
    match policy {
        CollisionStrategy::Skip => None,
        CollisionStrategy::Overwrite => Some(dest.to_path_buf()),
        CollisionStrategy::CompareHash => {
            let (a, b) = (hasher::sha256_file(source), hasher::sha256_file(dest));
            if a.is_some() && a == b {
                None
            } else {
                Some(unique_name(dest))
            }
        }
        CollisionStrategy::AutoRename => Some(unique_name(dest)),
    }
}

/// First free `stem (N).ext` next to `target`.
pub fn unique_name(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or(Path::new(""));
    let stem = paths::stem(target);
    let suffix = paths::suffix_lower(&file_name(target));
    // Keep the original suffix casing, like pathlib.
    let suffix = target
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .filter(|_| !suffix.is_empty())
        .unwrap_or_default();
    (1u64..)
        .map(|n| parent.join(format!("{stem} ({n}){suffix}")))
        .find(|c| !c.exists())
        .expect("unbounded counter always finds a free name")
}

/// Removes empty directories bottom-up; returns how many were removed.
pub fn clean_empty_directories(root: &Path, exclude_root: bool) -> usize {
    fn visit(dir: &Path, is_root: bool, exclude_root: bool) -> usize {
        let mut removed = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    removed += visit(&e.path(), false, exclude_root);
                }
            }
        }
        if is_root && exclude_root {
            return removed;
        }
        let empty = fs::read_dir(dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if empty && fs::remove_dir(dir).is_ok() {
            removed += 1;
        }
        removed
    }
    visit(&paths::resolve(root), true, exclude_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OrganizeStrategy;
    use crate::organizer::{Organizer, PlanOptions};
    use crate::scanner::FileScanner;
    use std::collections::BTreeSet;

    fn names(dir: &Path) -> BTreeSet<String> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn workspace() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (n, c) in [
            ("photo1.jpg", "J"),
            ("report.pdf", "P"),
            ("script.py", "S"),
            ("desktop.ini", "I"),
        ] {
            fs::write(d.path().join(n), c).unwrap();
        }
        d
    }

    fn plan(cfg: &Config, dir: &Path) -> Vec<ActionRecord> {
        let files = FileScanner::new(cfg).scan_dir(dir).unwrap();
        Organizer::new(cfg).plan(
            &files,
            dir,
            OrganizeStrategy::Extension,
            PlanOptions::default(),
        )
    }

    #[test]
    fn session_id_format() {
        let id = new_session_id();
        assert!(
            id.starts_with("sfo_") && id.len() == "sfo_20261108_101452_e020d0".len(),
            "{id}"
        );
    }

    #[test]
    fn dry_run_leaves_files_untouched() {
        let d = workspace();
        let cfg = Config::defaults();
        let before = names(d.path());
        let log = TransactionExecutor::new(&cfg).execute_session(
            plan(&cfg, d.path()),
            d.path(),
            d.path(),
            "extension",
            true,
            None,
        );
        assert_eq!(names(d.path()), before);
        assert!(log.dry_run);
        assert!(
            log.records
                .iter()
                .all(|r| r.message.starts_with("[DRY-RUN]"))
        );
    }

    #[test]
    fn live_execution_moves_files() {
        let d = workspace();
        let cfg = Config::defaults();
        let log = TransactionExecutor::new(&cfg).execute_session(
            plan(&cfg, d.path()),
            d.path(),
            d.path(),
            "extension",
            false,
            None,
        );
        assert_eq!(log.moved_count, 3);
        assert!(d.path().join("Images/photo1.jpg").exists());
        assert!(d.path().join("Documents/report.pdf").exists());
        assert!(log.end_time.is_some());
    }

    #[test]
    fn collision_policies() {
        let d = tempfile::tempdir().unwrap();
        let cfg = Config::defaults();
        fs::create_dir(d.path().join("Images")).unwrap();
        fs::write(d.path().join("Images/pic.png"), "DESTINATION_FILE").unwrap();
        fs::write(d.path().join("pic.png"), "SOURCE_FILE").unwrap();
        TransactionExecutor::new(&cfg).execute_session(
            plan(&cfg, d.path()),
            d.path(),
            d.path(),
            "extension",
            false,
            None,
        );
        assert_eq!(
            fs::read_to_string(d.path().join("Images/pic.png")).unwrap(),
            "DESTINATION_FILE"
        );
        assert_eq!(
            fs::read_to_string(d.path().join("Images/pic (1).png")).unwrap(),
            "SOURCE_FILE"
        );

        // compare_hash: identical content is skipped.
        let mut cfg = Config::defaults();
        cfg.set("collision_strategy", serde_json::json!("compare_hash"));
        fs::write(d.path().join("pic.png"), "DESTINATION_FILE").unwrap();
        let log = TransactionExecutor::new(&cfg).execute_session(
            plan(&cfg, d.path()),
            d.path(),
            d.path(),
            "extension",
            false,
            None,
        );
        assert_eq!(log.skipped_count, 1);
        assert!(d.path().join("pic.png").exists());
    }

    #[test]
    fn missing_source_fails_and_cleanup_runs() {
        let d = workspace();
        let mut cfg = Config::defaults();
        cfg.set("clean_empty_folders", serde_json::json!(true));
        let mut recs = plan(&cfg, d.path());
        fs::remove_file(d.path().join("script.py")).unwrap();
        fs::create_dir_all(d.path().join("empty/nested")).unwrap();
        recs.push(ActionRecord::new(
            ActionType::Delete,
            d.path().join("gone.txt"),
            None,
            0,
        ));
        let log = TransactionExecutor::new(&cfg).execute_session(
            recs,
            d.path(),
            d.path(),
            "extension",
            false,
            None,
        );
        assert_eq!(
            log.records
                .iter()
                .filter(|r| r.status == RecordStatus::Failed)
                .count(),
            2
        );
        assert!(!d.path().join("empty").exists());
    }
}
