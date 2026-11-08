//! Path and timestamp helpers that reproduce Python v3.0 semantics.
//!
//! Stored journals contain paths produced by `pathlib.Path.resolve()` and
//! timestamps produced by `datetime.isoformat()`. These helpers keep the Rust
//! build byte-compatible with both.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// Equivalent of Python's `Path.resolve(strict=False)`.
///
/// Makes the path absolute, resolves symlinks for the longest existing
/// prefix, and never returns Windows `\\?\` verbatim paths.
pub fn resolve(path: &Path) -> PathBuf {
    let abs = normalize(&std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    if let Ok(real) = dunce::canonicalize(&abs) {
        return real;
    }
    // Walk up until an ancestor exists, canonicalize it, re-append the rest.
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = abs.as_path();
    while let Some(parent) = cur.parent() {
        if let Some(name) = cur.file_name() {
            tail.push(name.to_os_string());
        }
        if let Ok(real) = dunce::canonicalize(parent) {
            let mut out = real;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return normalize(&out);
        }
        cur = parent;
    }
    normalize(&abs)
}

/// Lexically removes `.` and `..` components.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Joins a `/`-separated relative folder (e.g. `"2026/11"`) onto `base`
/// segment by segment, so Windows paths use `\` throughout like `pathlib`.
pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    for seg in rel.split(seps).filter(|s| !s.is_empty()) {
        out.push(seg);
    }
    out
}

/// Python `Path.suffix.lower()`: last `.ext` of the file name, or `""`.
pub fn suffix_lower(name: &str) -> String {
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => format!(".{}", ext.to_lowercase()),
        _ => String::new(),
    }
}

/// Python `Path.stem`.
pub fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Case-insensitive on Windows (like `PureWindowsPath`), exact elsewhere.
pub fn path_eq(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// `base == path or base in path.parents`, with Windows case folding.
pub fn is_within(path: &Path, base: &Path) -> bool {
    if cfg!(windows) {
        let p = PathBuf::from(path.to_string_lossy().to_lowercase());
        let b = PathBuf::from(base.to_string_lossy().to_lowercase());
        p.starts_with(b)
    } else {
        path.starts_with(base)
    }
}

/// Moves a file, replacing any existing destination (Python `os.replace`,
/// falling back to copy + delete across devices like `shutil.move`).
pub fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            if !src.is_file() {
                return Err(rename_err);
            }
            fs::copy(src, dst)?;
            if let Ok(mtime) = fs::metadata(src).and_then(|m| m.modified()) {
                let _ = fs::File::options()
                    .write(true)
                    .open(dst)
                    .and_then(|f| f.set_modified(mtime));
            }
            fs::remove_file(src)
        }
    }
}

/// `datetime.fromtimestamp()` equivalent: local naive time.
pub fn local_naive(t: SystemTime) -> NaiveDateTime {
    DateTime::<Local>::from(t).naive_local()
}

/// `datetime.now()` equivalent.
pub fn now_naive() -> NaiveDateTime {
    Local::now().naive_local()
}

/// Python `datetime.isoformat()` for naive datetimes: microseconds are
/// printed only when non-zero.
pub fn py_isoformat(dt: &NaiveDateTime) -> String {
    let micros = dt.and_utc().timestamp_subsec_micros();
    if micros == 0 {
        dt.format("%Y-%m-%dT%H:%M:%S").to_string()
    } else {
        format!("{}.{:06}", dt.format("%Y-%m-%dT%H:%M:%S"), micros)
    }
}

/// Python `datetime.now(timezone.utc).isoformat()`.
pub fn py_isoformat_utc(dt: &DateTime<Utc>) -> String {
    format!("{}+00:00", py_isoformat(&dt.naive_utc()))
}

/// Python `datetime.fromisoformat()` for the formats SFO writes. Offsets are
/// converted to local time and dropped (the Python build only stores naive
/// local times in sessions).
pub fn py_fromisoformat(s: &str) -> Option<NaiveDateTime> {
    const NAIVE: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M",
    ];
    for fmt in NAIVE {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt);
        }
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local).naive_local());
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
}

/// Local naive time `days` ago, for history pruning.
pub fn days_ago(days: i64) -> NaiveDateTime {
    let now = Local::now();
    let then = now - chrono::Duration::days(days);
    Local
        .from_local_datetime(&then.naive_local())
        .single()
        .map(|d| d.naive_local())
        .unwrap_or_else(|| then.naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isoformat_matches_python() {
        let dt =
            NaiveDateTime::parse_from_str("2026-11-08 10:14:52.489526", "%Y-%m-%d %H:%M:%S%.f")
                .unwrap();
        assert_eq!(py_isoformat(&dt), "2026-11-08T10:14:52.489526");
        let whole =
            NaiveDateTime::parse_from_str("2026-11-08 10:14:52", "%Y-%m-%d %H:%M:%S").unwrap();
        assert_eq!(py_isoformat(&whole), "2026-11-08T10:14:52");
    }

    #[test]
    fn fromisoformat_round_trips() {
        for s in ["2026-11-08T10:14:52.489526", "2026-11-08T10:14:52"] {
            assert_eq!(py_isoformat(&py_fromisoformat(s).unwrap()), s);
        }
        assert!(py_fromisoformat("2026-11-08T03:14:52.615156+00:00").is_some());
        assert!(py_fromisoformat("garbage").is_none());
    }

    #[test]
    fn suffix_matches_pathlib() {
        assert_eq!(suffix_lower("Photo.JPG"), ".jpg");
        assert_eq!(suffix_lower("archive.tar.gz"), ".gz");
        assert_eq!(suffix_lower(".bashrc"), "");
        assert_eq!(suffix_lower("noext"), "");
        assert_eq!(suffix_lower("trailing."), "");
    }

    #[test]
    fn join_rel_splits_segments() {
        let p = join_rel(Path::new("root"), "2026/11");
        assert_eq!(p, Path::new("root").join("2026").join("11"));
    }

    #[test]
    fn resolve_handles_missing_tail() {
        let dir = std::env::temp_dir();
        let r = resolve(&dir.join("sfo_missing_x").join("..").join("sfo_missing_y"));
        assert!(r.ends_with("sfo_missing_y"));
        assert!(!r.to_string_lossy().starts_with(r"\\?\"));
    }
}
