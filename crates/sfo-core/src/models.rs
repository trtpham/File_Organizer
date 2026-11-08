//! Data models shared across the engine. Field names and order match the
//! Python v3.0 `to_dict()` output so journals stay interchangeable.

use crate::paths::{self, local_naive, now_naive};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Kind of file operation recorded in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ActionType {
    Move,
    Copy,
    Rename,
    Delete,
    Skip,
    Deduplicate,
}

impl ActionType {
    /// Operations that `UndoManager` can reverse.
    pub fn is_reversible(self) -> bool {
        matches!(self, Self::Move | Self::Deduplicate | Self::Rename)
    }
}

/// What to do when the destination already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollisionStrategy {
    #[default]
    AutoRename,
    Skip,
    CompareHash,
    Overwrite,
}

impl CollisionStrategy {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto_rename" => Self::AutoRename,
            "skip" => Self::Skip,
            "compare_hash" => Self::CompareHash,
            "overwrite" => Self::Overwrite,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutoRename => "auto_rename",
            Self::Skip => "skip",
            Self::CompareHash => "compare_hash",
            Self::Overwrite => "overwrite",
        }
    }
}

/// How the organizer chooses a destination folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizeStrategy {
    #[default]
    Extension,
    Date,
    Size,
    Regex,
}

impl OrganizeStrategy {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "extension" => Self::Extension,
            "date" => Self::Date,
            "size" => Self::Size,
            "regex" => Self::Regex,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Extension => "extension",
            Self::Date => "date",
            Self::Size => "size",
            Self::Regex => "regex",
        }
    }
}

/// Outcome of a single record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RecordStatus {
    #[default]
    Pending,
    Success,
    Skipped,
    Failed,
}

/// Serde adapter for Python `isoformat()` naive datetimes.
pub mod py_datetime {
    use super::*;
    use serde::{Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(dt: &NaiveDateTime, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&paths::py_isoformat(dt))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<NaiveDateTime, D::Error> {
        let raw = String::deserialize(d)?;
        paths::py_fromisoformat(&raw)
            .ok_or_else(|| D::Error::custom(format!("bad datetime: {raw}")))
    }

    /// `Option<NaiveDateTime>`; `null` and `""` both mean `None`.
    pub mod option {
        use super::*;

        pub fn serialize<S: Serializer>(
            dt: &Option<NaiveDateTime>,
            s: S,
        ) -> Result<S::Ok, S::Error> {
            match dt {
                Some(dt) => s.serialize_str(&paths::py_isoformat(dt)),
                None => s.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<Option<NaiveDateTime>, D::Error> {
            match Option::<String>::deserialize(d)? {
                None => Ok(None),
                Some(raw) if raw.is_empty() => Ok(None),
                Some(raw) => paths::py_fromisoformat(&raw)
                    .map(Some)
                    .ok_or_else(|| D::Error::custom(format!("bad datetime: {raw}"))),
            }
        }
    }
}

/// A discovered file with cached metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub filename: String,
    pub extension: String,
    pub size_bytes: u64,
    #[serde(with = "py_datetime")]
    pub created_time: NaiveDateTime,
    #[serde(with = "py_datetime")]
    pub modified_time: NaiveDateTime,
    pub partial_hash: Option<String>,
    pub full_hash: Option<String>,
}

impl FileInfo {
    /// Python `FileInfo.from_path`: stats the file (following symlinks).
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let meta = std::fs::metadata(path)?;
        let modified = meta
            .modified()
            .map(local_naive)
            .unwrap_or_else(|_| now_naive());
        let created = created_time(&meta).unwrap_or(modified);
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self {
            path: paths::resolve(path),
            extension: paths::suffix_lower(&filename),
            filename,
            size_bytes: meta.len(),
            created_time: created,
            modified_time: modified,
            partial_hash: None,
            full_hash: None,
        })
    }
}

/// Python `st_ctime`: creation time on Windows, inode change time on Unix.
#[cfg(unix)]
fn created_time(meta: &std::fs::Metadata) -> Option<NaiveDateTime> {
    use std::os::unix::fs::MetadataExt;
    chrono::DateTime::from_timestamp(meta.ctime(), meta.ctime_nsec() as u32)
        .map(|utc| utc.with_timezone(&chrono::Local).naive_local())
}

#[cfg(not(unix))]
fn created_time(meta: &std::fs::Metadata) -> Option<NaiveDateTime> {
    meta.created().ok().map(local_naive)
}

fn default_success() -> RecordStatus {
    RecordStatus::Success
}

/// A single planned or executed file operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub action_type: ActionType,
    pub source_path: PathBuf,
    #[serde(default)]
    pub destination_path: Option<PathBuf>,
    #[serde(default)]
    pub file_size: u64,
    #[serde(default)]
    pub file_hash: Option<String>,
    /// Python `from_dict` defaults a missing status to `SUCCESS`.
    #[serde(default = "default_success")]
    pub status: RecordStatus,
    #[serde(default)]
    pub message: String,
    #[serde(with = "py_datetime", default = "now_naive")]
    pub timestamp: NaiveDateTime,
}

impl ActionRecord {
    /// New `PENDING` record stamped with the current time.
    pub fn new(
        action_type: ActionType,
        source: PathBuf,
        destination: Option<PathBuf>,
        file_size: u64,
    ) -> Self {
        Self {
            action_type,
            source_path: source,
            destination_path: destination,
            file_size,
            file_hash: None,
            status: RecordStatus::Pending,
            message: String::new(),
            timestamp: now_naive(),
        }
    }
}

/// Files sharing identical content.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateGroup {
    pub file_hash: String,
    pub file_size: u64,
    pub files: Vec<FileInfo>,
    pub suggested_keep: Option<FileInfo>,
}

impl DuplicateGroup {
    /// Bytes wasted by duplicates, excluding one kept copy.
    pub fn wasted_space_bytes(&self) -> u64 {
        (self.files.len().saturating_sub(1) as u64) * self.file_size
    }
}

/// Images with similar perceptual hashes.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualDuplicateGroup {
    pub phash: String,
    pub files: Vec<FileInfo>,
    pub similarity_score: f64,
    pub suggested_keep: Option<FileInfo>,
}

impl VisualDuplicateGroup {
    pub fn total_count(&self) -> usize {
        self.files.len()
    }
}

/// Folder / USB synchronization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Mirror,
    #[default]
    Incremental,
    Snapshot,
}

fn default_true() -> bool {
    true
}

/// Saved sync profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncProfile {
    pub name: String,
    pub source_dir: PathBuf,
    pub dest_dir: PathBuf,
    #[serde(default)]
    pub mode: SyncMode,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub trigger_on_usb: bool,
    #[serde(default)]
    pub usb_volume_name: Option<String>,
}

/// Financial metadata extracted from invoices and receipts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReceiptMetadata {
    pub is_receipt: bool,
    pub vendor: Option<String>,
    pub date_str: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub confidence: f64,
    pub summary: String,
}

fn default_strategy() -> String {
    "extension".into()
}

/// Complete record of one organization / dedup / rename session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLog {
    pub session_id: String,
    #[serde(with = "py_datetime")]
    pub start_time: NaiveDateTime,
    #[serde(with = "py_datetime::option", default)]
    pub end_time: Option<NaiveDateTime>,
    #[serde(default)]
    pub source_dir: String,
    #[serde(default)]
    pub destination_dir: String,
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub total_files: u64,
    #[serde(default)]
    pub total_size_bytes: u64,
    #[serde(default)]
    pub moved_count: u64,
    #[serde(default)]
    pub skipped_count: u64,
    #[serde(default)]
    pub duplicate_count: u64,
    #[serde(default)]
    pub space_saved_bytes: u64,
    #[serde(default)]
    pub records: Vec<ActionRecord>,
}

impl SessionLog {
    pub fn new(session_id: String, start_time: NaiveDateTime) -> Self {
        Self {
            session_id,
            start_time,
            end_time: None,
            source_dir: String::new(),
            destination_dir: String::new(),
            strategy: default_strategy(),
            dry_run: false,
            total_files: 0,
            total_size_bytes: 0,
            moved_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            space_saved_bytes: 0,
            records: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_json_matches_python_shape() {
        let mut r = ActionRecord::new(ActionType::Move, "a".into(), Some("b".into()), 3);
        r.timestamp = paths::py_fromisoformat("2026-11-08T10:14:52.488581").unwrap();
        let v = serde_json::to_value(&r).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "action_type",
                "source_path",
                "destination_path",
                "file_size",
                "file_hash",
                "status",
                "message",
                "timestamp"
            ]
        );
        assert_eq!(v["action_type"], "MOVE");
        assert_eq!(v["status"], "PENDING");
        assert_eq!(v["timestamp"], "2026-11-08T10:14:52.488581");
    }

    #[test]
    fn record_defaults_match_python_from_dict() {
        let r: ActionRecord =
            serde_json::from_str(r#"{"action_type":"RENAME","source_path":"x"}"#).unwrap();
        assert_eq!(r.status, RecordStatus::Success);
        assert_eq!(r.file_size, 0);
        assert!(r.destination_path.is_none());
    }

    #[test]
    fn wasted_space() {
        let g = DuplicateGroup {
            file_hash: "h".into(),
            file_size: 10,
            files: vec![],
            suggested_keep: None,
        };
        assert_eq!(g.wasted_space_bytes(), 0);
    }
}
