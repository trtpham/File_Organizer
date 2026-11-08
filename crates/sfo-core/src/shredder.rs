//! Multi-pass secure file shredder (1/2/3/7-pass), guarded by
//! [`crate::guardrails`] and optionally recorded in the audit chain.
//!
//! Overwriting cannot guarantee erasure on SSDs (wear levelling), copy-on-write
//! filesystems or cloud-synced folders; it is a best-effort control.

use crate::fsutil;
use crate::guardrails::{self, AuditLogChain};
use crate::paths;
use serde_json::{Map, json};
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const CHUNK: usize = 1024 * 1024;

/// Progress callback: `(current, total, file_name)`.
pub type ShredProgress<'a> = &'a mut dyn FnMut(usize, usize, &str);

/// Wiping standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShredMethod {
    Quick1Pass,
    Nist80088,
    #[default]
    Dod3Pass,
    Gutmann7Pass,
}

impl ShredMethod {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "quick_1pass" => Self::Quick1Pass,
            "nist_800_88" => Self::Nist80088,
            "dod_3pass" => Self::Dod3Pass,
            "gutmann_7pass" => Self::Gutmann7Pass,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quick1Pass => "quick_1pass",
            Self::Nist80088 => "nist_800_88",
            Self::Dod3Pass => "dod_3pass",
            Self::Gutmann7Pass => "gutmann_7pass",
        }
    }

    /// `None` = random bytes.
    pub fn patterns(self) -> &'static [Option<u8>] {
        match self {
            Self::Quick1Pass => &[None],
            Self::Nist80088 => &[Some(0x00), None],
            Self::Dod3Pass => &[Some(0x00), Some(0xFF), None],
            Self::Gutmann7Pass => &[
                None,
                Some(0x55),
                Some(0xAA),
                Some(0x00),
                Some(0xFF),
                Some(0x92),
                None,
            ],
        }
    }
}

/// Refusals raised before any byte is touched.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShredError {
    #[error("File not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("Action blocked by Enterprise Guardrails: {0}")]
    Protected(String),
    #[error("Path is not a regular file: {}", .0.display())]
    NotAFile(PathBuf),
    #[error("Path is not a directory: {}", .0.display())]
    NotADirectory(PathBuf),
}

/// Outcome for one file.
#[derive(Debug, Clone, PartialEq)]
pub struct ShredResult {
    pub file_path: PathBuf,
    pub original_size: u64,
    pub passes_completed: usize,
    pub success: bool,
    pub error_message: String,
    pub duration_seconds: f64,
}

/// Outcome for a directory.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShredBatchResult {
    pub total_files: usize,
    pub total_bytes: u64,
    pub shredded_files: Vec<PathBuf>,
    pub failed_files: Vec<(PathBuf, String)>,
    pub duration_seconds: f64,
}

impl ShredBatchResult {
    pub fn success_rate(&self) -> f64 {
        if self.total_files == 0 {
            100.0
        } else {
            self.shredded_files.len() as f64 / self.total_files as f64 * 100.0
        }
    }
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

fn overwrite(
    path: &Path,
    size: u64,
    method: ShredMethod,
    progress: &mut Option<ShredProgress<'_>>,
    done: &mut usize,
) -> std::io::Result<()> {
    let mut f = fs::OpenOptions::new().read(true).write(true).open(path)?;
    let passes = method.patterns();
    let name = fsutil::name_of(path);
    for (i, pat) in passes.iter().enumerate() {
        f.seek(SeekFrom::Start(0))?;
        let mut left = size;
        let fixed = pat.map(|b| vec![b; CHUNK]);
        while left > 0 {
            let n = left.min(CHUNK as u64) as usize;
            match &fixed {
                Some(buf) => f.write_all(&buf[..n])?,
                None => f.write_all(&fsutil::random_bytes(n))?,
            }
            left -= n as u64;
        }
        f.flush()?;
        let _ = f.sync_all();
        *done += 1;
        if let Some(cb) = progress.as_mut() {
            cb(i + 1, passes.len(), &name);
        }
    }
    f.set_len(0)?;
    let _ = f.sync_all();
    Ok(())
}

/// Overwrites, truncates, renames to a random name, then deletes the file.
pub fn shred_file(
    file_path: &Path,
    method: ShredMethod,
    audit: Option<&AuditLogChain>,
    mut progress: Option<ShredProgress<'_>>,
) -> Result<ShredResult, ShredError> {
    let start = Instant::now();
    let path = paths::resolve(file_path);
    if !path.exists() {
        return Err(ShredError::NotFound(path));
    }
    if let Some(reason) = guardrails::protected_reason(&path) {
        return Err(ShredError::Protected(reason));
    }
    if !path.is_file() {
        return Err(ShredError::NotAFile(path));
    }
    fsutil::make_writable(&path);
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut done = 0;

    let run = (|| -> std::io::Result<()> {
        if size > 0 {
            overwrite(&path, size, method, &mut progress, &mut done)?;
        }
        let obfuscated = path.with_file_name(format!(
            ".sfo_shredded_{}.tmp",
            hex::encode(fsutil::random_bytes(12))
        ));
        fs::rename(&path, &obfuscated)?;
        fs::remove_file(&obfuscated)
    })();

    let duration = round4(start.elapsed().as_secs_f64());
    let result = ShredResult {
        file_path: path.clone(),
        original_size: size,
        passes_completed: done,
        success: run.is_ok(),
        error_message: run.err().map(|e| e.to_string()).unwrap_or_default(),
        duration_seconds: duration,
    };
    if result.success
        && let Some(chain) = audit
    {
        let mut d = Map::new();
        d.insert("path".into(), json!(path.display().to_string()));
        d.insert("original_size".into(), json!(size));
        d.insert("method".into(), json!(method.as_str()));
        d.insert("passes".into(), json!(done));
        d.insert("duration_sec".into(), json!(duration));
        let _ = chain.log_event("SECURE_SHRED", d, None);
    }
    Ok(result)
}

/// Shreds every file under `dir_path`, then removes emptied folders
/// (including the root when it ends up empty, like Python v3.0).
pub fn shred_directory(
    dir_path: &Path,
    method: ShredMethod,
    remove_empty_dirs: bool,
    audit: Option<&AuditLogChain>,
    mut progress: Option<ShredProgress<'_>>,
) -> Result<ShredBatchResult, ShredError> {
    let start = Instant::now();
    let root = paths::resolve(dir_path);
    if !root.exists() {
        return Err(ShredError::NotFound(root));
    }
    if !root.is_dir() {
        return Err(ShredError::NotADirectory(root));
    }
    if let Some(reason) = guardrails::protected_reason(&root) {
        return Err(ShredError::Protected(reason));
    }
    let files = fsutil::walk_files(&root);
    let mut batch = ShredBatchResult {
        total_files: files.len(),
        total_bytes: files
            .iter()
            .filter_map(|f| fs::metadata(f).ok())
            .map(|m| m.len())
            .sum(),
        ..Default::default()
    };
    for (i, f) in files.iter().enumerate() {
        if let Some(cb) = progress.as_mut() {
            cb(i + 1, files.len(), &fsutil::name_of(f));
        }
        match shred_file(f, method, audit, None) {
            Ok(r) if r.success => batch.shredded_files.push(f.clone()),
            Ok(r) => batch.failed_files.push((f.clone(), r.error_message)),
            Err(e) => batch.failed_files.push((f.clone(), e.to_string())),
        }
    }
    if remove_empty_dirs {
        crate::executor::clean_empty_directories(&root, false);
    }
    batch.duration_seconds = round4(start.elapsed().as_secs_f64());
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shreds_file_with_progress_and_audit() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("secret.txt");
        fs::write(&p, vec![b'S'; 3 * CHUNK + 17]).unwrap();
        let mut ro = fs::metadata(&p).unwrap().permissions();
        ro.set_readonly(true);
        fs::set_permissions(&p, ro).unwrap();
        let chain = AuditLogChain::new(Some(&d.path().join("audit.jsonl"))).unwrap();
        let mut passes = Vec::new();
        let mut cb = |i: usize, n: usize, _: &str| passes.push((i, n));
        let r = shred_file(&p, ShredMethod::Dod3Pass, Some(&chain), Some(&mut cb)).unwrap();
        assert!(r.success, "{r:?}");
        assert_eq!(r.passes_completed, 3);
        assert_eq!(passes, [(1, 3), (2, 3), (3, 3)]);
        assert!(!p.exists());
        assert_eq!(
            fs::read_dir(d.path()).unwrap().count(),
            1,
            "only the audit log remains"
        );
        let v = chain.verify_integrity();
        assert!(v.valid && v.verified == 1);
    }

    #[test]
    fn refusals_and_directory() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(
            shred_file(&d.path().join("nope"), ShredMethod::Quick1Pass, None, None),
            Err(ShredError::NotFound(_))
        ));
        assert!(matches!(
            shred_file(d.path(), ShredMethod::Quick1Pass, None, None),
            Err(ShredError::NotAFile(_))
        ));
        let prot = d.path().join("AppData");
        fs::create_dir(&prot).unwrap();
        assert!(matches!(
            shred_directory(&prot, ShredMethod::Quick1Pass, true, None, None),
            Err(ShredError::Protected(_))
        ));

        let dir = d.path().join("wipe");
        fs::create_dir_all(dir.join("n")).unwrap();
        fs::write(dir.join("a"), "1").unwrap();
        fs::write(dir.join("n/b"), "").unwrap();
        let b = shred_directory(&dir, ShredMethod::Gutmann7Pass, true, None, None).unwrap();
        assert_eq!((b.total_files, b.shredded_files.len()), (2, 2));
        assert_eq!(b.success_rate(), 100.0);
        assert!(!dir.exists());
        assert_eq!(
            ShredMethod::parse("nist_800_88").unwrap().patterns().len(),
            2
        );
    }
}
