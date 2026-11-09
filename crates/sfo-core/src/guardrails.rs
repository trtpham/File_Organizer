//! Enterprise safety guardrails and the tamper-evident audit log.
//!
//! The audit chain format is shared with Python v3.0: each JSONL line hashes
//! `prev|timestamp|operator|action|json.dumps(details, sort_keys=True)`.

use crate::error::Result;
use crate::paths;
use crate::pyjson;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Folder names that must never be a bulk-operation target.
pub const PROTECTED_ROOT_NAMES: [&str; 7] = [
    "AppData",
    "Application Data",
    "Windows",
    "System32",
    "SysWOW64",
    "Recovery",
    "Boot",
];

/// Default blast-radius limits.
pub const DEFAULT_MAX_FILES: u64 = 500;
pub const DEFAULT_MAX_BYTES: u64 = 10 * 1024 * 1024 * 1024;

fn windows_protected_paths() -> Vec<PathBuf> {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    [
        env("SystemRoot", "C:/Windows"),
        env("ProgramFiles", "C:/Program Files"),
        env("ProgramFiles(x86)", "C:/Program Files (x86)"),
        "C:/Windows".into(),
        "C:/System Volume Information".into(),
        "C:/$Recycle.Bin".into(),
    ]
    .iter()
    .map(|p| paths::resolve(Path::new(p)))
    .collect()
}

/// Returns `Some(reason)` when `path` is a drive root, inside a Windows
/// system directory, or named like an OS component.
pub fn protected_reason(path: &Path) -> Option<String> {
    let resolved = paths::resolve(path);

    if resolved.parent().is_none() {
        return Some("Cannot target drive root directory directly.".into());
    }

    if cfg!(windows) {
        for sys in windows_protected_paths() {
            if paths::is_within(&resolved, &sys) {
                return Some(format!(
                    "Target is located within protected system directory: {}",
                    sys.display()
                ));
            }
        }
    }

    let name = resolved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if PROTECTED_ROOT_NAMES.contains(&name.as_str()) {
        return Some(format!(
            "Directory name '{name}' is a protected operating system component."
        ));
    }
    None
}

/// `true` when [`protected_reason`] finds a reason.
pub fn is_protected_path(path: &Path) -> bool {
    protected_reason(path).is_some()
}

/// Returns `Some(warning)` when an operation exceeds the safety caps.
pub fn check_blast_radius(
    affected_count: u64,
    affected_bytes: u64,
    max_files: u64,
    max_bytes: u64,
) -> Option<String> {
    if affected_count > max_files {
        return Some(format!(
            "Operation impacts {affected_count} files (Safety threshold: {max_files}). Explicit confirmation required."
        ));
    }
    if affected_bytes > max_bytes {
        let gib = 1024.0 * 1024.0 * 1024.0;
        return Some(format!(
            "Operation impacts {:.1} GB of data (Safety threshold: {:.1} GB). Explicit confirmation required.",
            affected_bytes as f64 / gib,
            max_bytes as f64 / gib
        ));
    }
    None
}

/// [`check_blast_radius`] over planned records: counts every record that
/// would touch a file (not `SKIP`) and the bytes involved.
pub fn check_records(
    records: &[crate::models::ActionRecord],
    max_files: u64,
    max_bytes: u64,
) -> Option<String> {
    let active = records
        .iter()
        .filter(|r| r.action_type != crate::models::ActionType::Skip);
    let (count, bytes) = active.fold((0u64, 0u64), |(c, b), r| (c + 1, b + r.file_size));
    check_blast_radius(count, bytes, max_files, max_bytes)
}

/// All-zero hash that starts every chain.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Result of [`AuditLogChain::verify_integrity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainVerification {
    pub valid: bool,
    pub verified: usize,
    pub message: String,
}

/// Append-only SHA-256 hash-chained JSONL audit log.
#[derive(Debug, Clone)]
pub struct AuditLogChain {
    log_path: PathBuf,
}

impl AuditLogChain {
    /// `~/.smart_file_organizer/audit_chain.jsonl`.
    pub fn default_path() -> PathBuf {
        crate::default_data_dir().join("audit_chain.jsonl")
    }

    pub fn new(log_path: Option<&Path>) -> Result<Self> {
        let log_path = paths::resolve(
            &log_path
                .map(Path::to_path_buf)
                .unwrap_or_else(Self::default_path),
        );
        if let Some(dir) = log_path.parent() {
            fs::create_dir_all(dir)?;
        }
        Ok(Self { log_path })
    }

    pub fn path(&self) -> &Path {
        &self.log_path
    }

    /// `current_hash` of the last non-blank line, or [`GENESIS_HASH`].
    pub fn latest_hash(&self) -> String {
        let Ok(file) = fs::File::open(&self.log_path) else {
            return GENESIS_HASH.into();
        };
        let last = BufReader::new(file)
            .lines()
            .map_while(std::result::Result::ok)
            .filter(|l| !l.trim().is_empty())
            .last();
        last.and_then(|l| serde_json::from_str::<Value>(l.trim()).ok())
            .and_then(|v| {
                v.get("current_hash")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| GENESIS_HASH.into())
    }

    /// Appends an entry; `user` defaults to `USERNAME` / `USER` / `"system"`.
    pub fn log_event(
        &self,
        action: &str,
        details: Map<String, Value>,
        user: Option<&str>,
    ) -> Result<Value> {
        let prev = self.latest_hash();
        let timestamp = paths::py_isoformat_utc(&chrono::Utc::now());
        let operator = user.map(str::to_string).unwrap_or_else(default_operator);
        let details = Value::Object(details);
        let current = chain_hash(&prev, &timestamp, &operator, action, &details);

        let entry = json!({
            "timestamp": timestamp,
            "operator": operator,
            "action": action,
            "details": details,
            "previous_hash": prev,
            "current_hash": current,
        });
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(f, "{}", pyjson::dumps(&entry, false))?;
        Ok(entry)
    }

    /// Re-validates every link of the chain from genesis.
    pub fn verify_integrity(&self) -> ChainVerification {
        let file = match fs::File::open(&self.log_path) {
            Ok(f) if f.metadata().map(|m| m.len() > 0).unwrap_or(false) => f,
            _ => {
                return ChainVerification {
                    valid: true,
                    verified: 0,
                    message: "Audit log is empty.".into(),
                };
            }
        };
        let mut expected_prev = GENESIS_HASH.to_string();
        let mut count = 0;
        let fail = |count, message: String| ChainVerification {
            valid: false,
            verified: count,
            message,
        };

        for (idx, line) in BufReader::new(file).lines().enumerate() {
            let line_num = idx + 1;
            let line = match line {
                Ok(l) => l,
                Err(e) => return fail(count, format!("Verification failed due to error: {e}")),
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let record: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => return fail(count, format!("Verification failed due to error: {e}")),
            };
            let s = |k: &str| {
                record
                    .get(k)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            let prev = s("previous_hash");
            if prev != expected_prev {
                return fail(
                    count,
                    format!("Hash chain broken at line {line_num}: previous hash mismatch."),
                );
            }
            let details = record.get("details").cloned().unwrap_or(Value::Null);
            let computed = chain_hash(
                &prev,
                &s("timestamp"),
                &s("operator"),
                &s("action"),
                &details,
            );
            let current = s("current_hash");
            if computed != current {
                return fail(
                    count,
                    format!("Hash chain broken at line {line_num}: entry has been tampered with."),
                );
            }
            expected_prev = current;
            count += 1;
        }
        ChainVerification {
            valid: true,
            verified: count,
            message: format!(
                "Audit chain verified clean! All {count} records mathematically validated."
            ),
        }
    }
}

fn default_operator() -> String {
    ["USERNAME", "USER"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| "system".into())
}

/// SHA-256 over the Python v3.0 payload layout.
pub fn chain_hash(
    prev: &str,
    timestamp: &str,
    operator: &str,
    action: &str,
    details: &Value,
) -> String {
    let payload = format!(
        "{prev}|{timestamp}|{operator}|{action}|{}",
        pyjson::dumps(details, true)
    );
    hex::encode(Sha256::digest(payload.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_paths() {
        let root = if cfg!(windows) {
            Path::new("C:/")
        } else {
            Path::new("/")
        };
        assert!(is_protected_path(root));
        if cfg!(windows) {
            assert!(is_protected_path(Path::new("C:/Windows/System32")));
            assert!(is_protected_path(Path::new("c:/windows/whatever_missing")));
        }
        let safe = dirs::home_dir()
            .unwrap()
            .join("Documents")
            .join("MyProject");
        assert!(!is_protected_path(&safe));
        assert!(is_protected_path(&std::env::temp_dir().join("AppData")));
    }

    #[test]
    fn blast_radius() {
        assert!(
            check_blast_radius(100, 1024 * 1024, DEFAULT_MAX_FILES, DEFAULT_MAX_BYTES).is_none()
        );
        assert!(
            check_blast_radius(600, 1024, DEFAULT_MAX_FILES, DEFAULT_MAX_BYTES)
                .unwrap()
                .contains("600 files")
        );
        assert!(
            check_blast_radius(
                5,
                12 * 1024 * 1024 * 1024,
                DEFAULT_MAX_FILES,
                DEFAULT_MAX_BYTES
            )
            .unwrap()
            .contains("12.0 GB")
        );
    }

    #[test]
    fn audit_chain_detects_tampering() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("test_audit.jsonl");
        let chain = AuditLogChain::new(Some(&path)).unwrap();
        assert_eq!(chain.latest_hash(), GENESIS_HASH);

        let mut details = Map::new();
        details.insert("moved_files".into(), json!(10));
        let e1 = chain
            .log_event("ORGANIZE", details, Some("test_admin"))
            .unwrap();
        assert_eq!(e1["previous_hash"], GENESIS_HASH);
        assert_eq!(e1["current_hash"].as_str().unwrap().len(), 64);

        let mut details = Map::new();
        details.insert("freed_bytes".into(), json!(500000));
        let e2 = chain
            .log_event("CLEAN_JUNK", details, Some("test_admin"))
            .unwrap();
        assert_eq!(e2["previous_hash"], e1["current_hash"]);

        let v = chain.verify_integrity();
        assert!(v.valid && v.verified == 2, "{v:?}");

        let text = fs::read_to_string(&path).unwrap().replace("10", "9999");
        fs::write(&path, text).unwrap();
        let v = chain.verify_integrity();
        assert!(!v.valid);
        assert!(v.message.contains("tampered") || v.message.contains("mismatch"));
    }
}
