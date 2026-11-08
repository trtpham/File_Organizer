//! Session journal (SQLite + JSON files) and transactional rollback.
//!
//! Schema and file layout are identical to Python v3.0, so history written by
//! either build can be listed and rolled back by the other.

use crate::error::Result;
use crate::models::{RecordStatus, SessionLog};
use crate::paths;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS sessions (
        session_id TEXT PRIMARY KEY,
        start_time TEXT,
        end_time TEXT,
        source_dir TEXT,
        destination_dir TEXT,
        strategy TEXT,
        dry_run INTEGER,
        total_files INTEGER,
        moved_count INTEGER,
        skipped_count INTEGER,
        duplicate_count INTEGER,
        space_saved_bytes INTEGER,
        status TEXT DEFAULT 'COMPLETED',
        data_json TEXT
    )";

/// Row returned by [`UndoManager::list_sessions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_id: String,
    pub start_time: String,
    pub source_dir: String,
    pub destination_dir: String,
    pub strategy: String,
    pub dry_run: bool,
    pub moved_count: i64,
    pub duplicate_count: i64,
    pub status: String,
}

/// Outcome of [`UndoManager::rollback_session`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RollbackResult {
    pub success: bool,
    pub session_id: Option<String>,
    pub restored_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
    pub message: String,
}

impl RollbackResult {
    fn refused(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            ..Default::default()
        }
    }
}

/// Records sessions and reverses them.
#[derive(Debug, Clone)]
pub struct UndoManager {
    data_dir: PathBuf,
    db_path: PathBuf,
}

impl UndoManager {
    /// Opens (creating if needed) `<data_dir>/history.db`; `None` means
    /// `~/.smart_file_organizer`.
    pub fn new(data_dir: Option<&Path>) -> Result<Self> {
        let data_dir = data_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(crate::default_data_dir);
        fs::create_dir_all(&data_dir)?;
        let mgr = Self {
            db_path: data_dir.join("history.db"),
            data_dir,
        };
        let conn = mgr.connect()?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute(SCHEMA, [])?;
        Ok(mgr)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(conn)
    }

    fn journals_dir(&self) -> PathBuf {
        self.data_dir.join("journals")
    }

    /// Writes the session to SQLite and `journals/<id>.json`.
    pub fn save_session(&self, s: &SessionLog) -> Result<()> {
        let json = serde_json::to_string(s)?;
        self.connect()?.execute(
            "INSERT OR REPLACE INTO sessions (
                session_id, start_time, end_time, source_dir, destination_dir,
                strategy, dry_run, total_files, moved_count, skipped_count,
                duplicate_count, space_saved_bytes, status, data_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                s.session_id,
                paths::py_isoformat(&s.start_time),
                s.end_time
                    .as_ref()
                    .map(paths::py_isoformat)
                    .unwrap_or_default(),
                s.source_dir,
                s.destination_dir,
                s.strategy,
                i64::from(s.dry_run),
                s.total_files as i64,
                s.moved_count as i64,
                s.skipped_count as i64,
                s.duplicate_count as i64,
                s.space_saved_bytes as i64,
                if s.dry_run { "DRY_RUN" } else { "COMPLETED" },
                json,
            ],
        )?;
        fs::create_dir_all(self.journals_dir())?;
        fs::write(
            self.journals_dir().join(format!("{}.json", s.session_id)),
            serde_json::to_string_pretty(s)?,
        )?;
        Ok(())
    }

    /// Newest-first summaries.
    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT session_id, start_time, source_dir, destination_dir,
                    strategy, dry_run, moved_count, duplicate_count, status
             FROM sessions ORDER BY start_time DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| {
            let text = |i: usize| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default);
            let int = |i: usize| r.get::<_, Option<i64>>(i).map(|v| v.unwrap_or(0));
            Ok(SessionSummary {
                session_id: text(0)?,
                start_time: text(1)?,
                source_dir: text(2)?,
                destination_dir: text(3)?,
                strategy: text(4)?,
                dry_run: int(5)? != 0,
                moved_count: int(6)?,
                duplicate_count: int(7)?,
                status: text(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Full session from SQLite, falling back to the JSON journal.
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionLog>> {
        let row: Option<Option<String>> = self
            .connect()?
            .query_row(
                "SELECT data_json FROM sessions WHERE session_id = ?1",
                [session_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(Some(json)) = row.filter(|j| j.as_deref().is_some_and(|s| !s.is_empty())) {
            return Ok(Some(serde_json::from_str(&json)?));
        }
        let journal = self.journals_dir().join(format!("{session_id}.json"));
        if journal.exists() {
            return Ok(Some(serde_json::from_str(&fs::read_to_string(journal)?)?));
        }
        Ok(None)
    }

    /// Most recent live session that has not been rolled back.
    pub fn get_latest_session(&self) -> Result<Option<SessionLog>> {
        let id: Option<String> = self
            .connect()?
            .query_row(
                "SELECT session_id FROM sessions
                 WHERE dry_run = 0 AND status = 'COMPLETED'
                 ORDER BY start_time DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        match id {
            Some(id) => self.get_session(&id),
            None => Ok(None),
        }
    }

    /// Reverses successful MOVE / DEDUPLICATE / RENAME records in LIFO order,
    /// prunes folders emptied by the rollback, marks the session `ROLLED_BACK`.
    pub fn rollback_session(&self, session_id: Option<&str>) -> Result<RollbackResult> {
        let session = match session_id {
            None => match self.get_latest_session()? {
                Some(s) => s,
                None => {
                    return Ok(RollbackResult::refused(
                        "No undoable completed session found.",
                    ));
                }
            },
            Some(id) => match self.get_session(id)? {
                Some(s) => s,
                None => {
                    return Ok(RollbackResult::refused(format!(
                        "Session '{id}' not found."
                    )));
                }
            },
        };
        if session.dry_run {
            return Ok(RollbackResult::refused(
                "Cannot undo a Dry-Run simulation session.",
            ));
        }

        let mut res = RollbackResult {
            session_id: Some(session.session_id.clone()),
            ..Default::default()
        };
        let mut touched_dirs = BTreeSet::new();

        for rec in session.records.iter().rev() {
            if rec.status != RecordStatus::Success || !rec.action_type.is_reversible() {
                continue;
            }
            let Some(current) = rec.destination_path.as_deref() else {
                continue;
            };
            let original = rec.source_path.as_path();
            if !current.exists() {
                res.failed_count += 1;
                res.errors.push(format!(
                    "File missing at organized path: {}",
                    current.display()
                ));
                continue;
            }
            let restored = original
                .parent()
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| paths::move_file(current, original));
            match restored {
                Ok(()) => {
                    res.restored_count += 1;
                    if let Some(parent) = current.parent() {
                        touched_dirs.insert(parent.to_path_buf());
                    }
                }
                Err(e) => {
                    res.failed_count += 1;
                    let name = current
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    res.errors.push(format!("Failed to restore {name}: {e}"));
                }
            }
        }

        for dir in touched_dirs {
            prune_empty_parents(&dir, &session);
        }

        self.connect()?.execute(
            "UPDATE sessions SET status = 'ROLLED_BACK' WHERE session_id = ?1",
            [&session.session_id],
        )?;
        res.success = true;
        res.message = format!("Restored {} files successfully.", res.restored_count);
        Ok(res)
    }

    /// Deletes sessions (and their journals) started more than `days` ago.
    pub fn prune_old_sessions(&self, days: i64) -> Result<usize> {
        let cutoff = paths::py_isoformat(&paths::days_ago(days));
        let conn = self.connect()?;
        let ids: Vec<String> = conn
            .prepare("SELECT session_id FROM sessions WHERE start_time < ?1")?
            .query_map([&cutoff], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if !ids.is_empty() {
            conn.execute("DELETE FROM sessions WHERE start_time < ?1", [&cutoff])?;
        }
        for id in &ids {
            let _ = fs::remove_file(self.journals_dir().join(format!("{id}.json")));
        }
        Ok(ids.len())
    }
}

/// Removes `start` and then each parent while they are empty.
fn prune_empty_parents(start: &Path, session: &SessionLog) {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if should_stop_pruning(dir, session) {
            break;
        }
        let empty = fs::read_dir(dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if !empty || fs::remove_dir(dir).is_err() {
            break;
        }
        cur = dir.parent();
    }
}

/// Decides whether rollback cleanup must stop before removing `dir`.
///
/// Python v3.0 never stops: it keeps deleting empty ancestors, so undoing a
/// session could remove the session's own folder and empty folders above it
/// (e.g. an empty `Downloads`). The Rust build (PO decision, 2026-11-08) only
/// prunes folders strictly *inside* the session's source or destination
/// folder; those folders themselves always survive. Sessions that recorded
/// neither folder keep the Python behaviour.
fn should_stop_pruning(dir: &Path, session: &SessionLog) -> bool {
    let bounds: Vec<std::path::PathBuf> = [&session.destination_dir, &session.source_dir]
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| paths::resolve(Path::new(s)))
        .collect();
    if bounds.is_empty() {
        return false;
    }
    let dir = paths::resolve(dir);
    let strictly_inside =
        |b: &std::path::PathBuf| paths::is_within(&dir, b) && !paths::path_eq(&dir, b);
    !bounds.iter().any(strictly_inside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::executor::TransactionExecutor;
    use crate::models::{ActionRecord, ActionType, OrganizeStrategy};
    use crate::organizer::{Organizer, PlanOptions};
    use crate::scanner::FileScanner;
    use std::collections::BTreeMap;

    fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_file())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn undo_restores_all_files() {
        let ws = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        for (n, c) in [
            ("photo1.jpg", "J"),
            ("report.pdf", "P"),
            ("tài liệu.docx", "U"),
            ("script.py", "S"),
        ] {
            fs::write(ws.path().join(n), c).unwrap();
        }
        let original = snapshot(ws.path());
        let cfg = Config::defaults();
        let files = FileScanner::new(&cfg).scan_dir(ws.path()).unwrap();
        let recs = Organizer::new(&cfg).plan(
            &files,
            ws.path(),
            OrganizeStrategy::Extension,
            PlanOptions::default(),
        );
        let session = TransactionExecutor::new(&cfg).execute_session(
            recs,
            ws.path(),
            ws.path(),
            "extension",
            false,
            None,
        );

        let undo = UndoManager::new(Some(data.path())).unwrap();
        undo.save_session(&session).unwrap();
        assert!(ws.path().join("Images/photo1.jpg").exists());
        assert_eq!(undo.list_sessions(10).unwrap()[0].status, "COMPLETED");

        let res = undo.rollback_session(Some(&session.session_id)).unwrap();
        assert!(res.success, "{res:?}");
        assert_eq!(res.restored_count, 4);
        assert_eq!(snapshot(ws.path()), original);
        assert!(!ws.path().join("Images").exists());
        assert!(!ws.path().join("Documents").exists());
        assert_eq!(undo.list_sessions(10).unwrap()[0].status, "ROLLED_BACK");
        assert_eq!(
            undo.rollback_session(None).unwrap().message,
            "No undoable completed session found."
        );
    }

    #[test]
    fn rename_rollback_dry_run_refusal_and_prune() {
        let ws = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let src = ws.path().join("document_v1.txt");
        fs::write(&src, "Hello World").unwrap();
        let dst = ws.path().join("renamed_document_v1.txt");
        let cfg = Config::defaults();
        let rec = ActionRecord::new(ActionType::Rename, src.clone(), Some(dst.clone()), 11);
        let session = TransactionExecutor::new(&cfg).execute_session(
            vec![rec],
            ws.path(),
            ws.path(),
            "rename",
            false,
            None,
        );
        let undo = UndoManager::new(Some(data.path())).unwrap();
        undo.save_session(&session).unwrap();
        assert!(dst.exists() && !src.exists());

        assert!(undo.rollback_session(None).unwrap().success);
        assert!(src.exists() && !dst.exists());

        let dry = TransactionExecutor::new(&cfg).execute_session(
            vec![],
            ws.path(),
            ws.path(),
            "extension",
            true,
            None,
        );
        undo.save_session(&dry).unwrap();
        assert_eq!(
            undo.rollback_session(Some(&dry.session_id))
                .unwrap()
                .message,
            "Cannot undo a Dry-Run simulation session."
        );
        assert!(!undo.rollback_session(Some("nope")).unwrap().success);

        assert!(undo.prune_old_sessions(0).unwrap() >= 1);
        assert!(undo.get_session(&session.session_id).unwrap().is_none());
    }

    #[test]
    fn prune_never_removes_session_folders() {
        // Organize `parent/inbox` into `parent/sorted`; after undo, `sorted`
        // is empty but must survive, and so must `parent`.
        let root = tempfile::tempdir().unwrap();
        let inbox = root.path().join("parent/inbox");
        let sorted = root.path().join("parent/sorted");
        fs::create_dir_all(&inbox).unwrap();
        fs::create_dir_all(&sorted).unwrap();
        fs::write(inbox.join("a.png"), "A").unwrap();
        let cfg = Config::defaults();
        let files = FileScanner::new(&cfg).scan_dir(&inbox).unwrap();
        let recs = Organizer::new(&cfg).plan(
            &files,
            &sorted,
            OrganizeStrategy::Extension,
            PlanOptions::default(),
        );
        let session = TransactionExecutor::new(&cfg).execute_session(
            recs,
            &inbox,
            &sorted,
            "extension",
            false,
            None,
        );
        fs::remove_dir(&inbox).unwrap(); // inbox emptied by the move
        let data = tempfile::tempdir().unwrap();
        let undo = UndoManager::new(Some(data.path())).unwrap();
        undo.save_session(&session).unwrap();

        assert!(undo.rollback_session(None).unwrap().success);
        assert!(inbox.join("a.png").exists());
        assert!(!sorted.join("Images").exists());
        assert!(sorted.is_dir(), "destination folder must survive");
        assert!(root.path().join("parent").is_dir());
    }

    #[test]
    fn journal_fallback() {
        let data = tempfile::tempdir().unwrap();
        let undo = UndoManager::new(Some(data.path())).unwrap();
        let s = SessionLog::new("sfo_x".into(), paths::now_naive());
        undo.save_session(&s).unwrap();
        undo.connect()
            .unwrap()
            .execute("DELETE FROM sessions", [])
            .unwrap();
        assert_eq!(
            undo.get_session("sfo_x").unwrap().unwrap().session_id,
            "sfo_x"
        );
    }
}
