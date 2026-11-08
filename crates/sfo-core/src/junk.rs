//! Developer junk sweeper: inactive `node_modules`, venvs, build caches, temp.

use crate::fsutil;
use crate::paths;
use chrono::NaiveDateTime;
use std::fs;
use std::path::{Path, PathBuf};

pub const NODE_MODULES: &str = "Node.js Modules";
pub const VIRTUAL_ENV: &str = "Python Virtualenvs";
pub const PYTHON_CACHE: &str = "Python Bytecode Cache";
pub const BUILD_ARTIFACTS: &str = "Build Artifacts (Target/Dist)";
pub const SYSTEM_TEMP: &str = "System Temp Files";

/// `(dir name, category, description)`.
pub const TARGET_DIRS: [(&str, &str, &str); 13] = [
    (
        "node_modules",
        NODE_MODULES,
        "JavaScript dependency package tree",
    ),
    (".venv", VIRTUAL_ENV, "Python isolated virtual environment"),
    ("venv", VIRTUAL_ENV, "Python isolated virtual environment"),
    ("__pycache__", PYTHON_CACHE, "Compiled Python .pyc files"),
    (".pytest_cache", PYTHON_CACHE, "Pytest test run cache"),
    (".mypy_cache", PYTHON_CACHE, "Mypy type checker cache"),
    (
        "target",
        BUILD_ARTIFACTS,
        "Rust/Java compiled binary build artifacts",
    ),
    (".gradle", BUILD_ARTIFACTS, "Gradle build system cache"),
    (".dart_tool", BUILD_ARTIFACTS, "Dart/Flutter build cache"),
    (".next", BUILD_ARTIFACTS, "Next.js build artifacts"),
    (".nuxt", BUILD_ARTIFACTS, "Nuxt.js build artifacts"),
    (".turbo", BUILD_ARTIFACTS, "Turborepo cache"),
    (
        "DerivedData",
        BUILD_ARTIFACTS,
        "Xcode compiled intermediates",
    ),
];

/// Caches that are always reported regardless of age.
const ALWAYS_JUNK: [&str; 3] = ["__pycache__", ".pytest_cache", ".mypy_cache"];

/// A disposable artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct JunkItem {
    pub name: String,
    pub path: PathBuf,
    pub category: String,
    pub size_bytes: u64,
    pub last_modified: NaiveDateTime,
    pub description: String,
}

/// Result of [`clean_junk`].
#[derive(Debug, Clone, PartialEq)]
pub struct CleanResult {
    pub success: bool,
    pub cleaned_count: usize,
    pub reclaimed_bytes: u64,
    pub dry_run: bool,
    pub errors: Vec<String>,
    pub message: String,
}

fn mtime(path: &Path) -> Option<NaiveDateTime> {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(paths::local_naive)
}

/// Junk directories under `root` older than `inactive_days` (caches always).
/// `.git` is never entered; matched folders are not descended into.
pub fn scan_directory(root: &Path, inactive_days: i64) -> Vec<JunkItem> {
    let cutoff = paths::days_ago(inactive_days);
    let mut out = Vec::new();
    scan_into(&paths::resolve(root), cutoff, &mut out);
    out
}

fn scan_into(dir: &Path, cutoff: NaiveDateTime, out: &mut Vec<JunkItem>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut descend = Vec::new();
    for e in entries.flatten() {
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        let path = e.path();
        if let Some((_, cat, desc)) = TARGET_DIRS.iter().find(|(n, _, _)| *n == name)
            && let Some(mt) = mtime(&path)
            && (mt < cutoff || ALWAYS_JUNK.contains(&name.as_str()))
        {
            out.push(JunkItem {
                size_bytes: fsutil::dir_size(&path),
                name,
                path,
                category: cat.to_string(),
                last_modified: mt,
                description: desc.to_string(),
            });
            continue;
        }
        descend.push(path);
    }
    for d in descend {
        scan_into(&d, cutoff, out);
    }
}

/// Entries in the OS temp directory older than 3 days.
pub fn scan_system_temp() -> Vec<JunkItem> {
    scan_temp_dir(&std::env::temp_dir(), 3)
}

/// [`scan_system_temp`] for an arbitrary directory and age.
pub fn scan_temp_dir(temp: &Path, older_than_days: i64) -> Vec<JunkItem> {
    let cutoff = paths::days_ago(older_than_days);
    let Ok(entries) = fs::read_dir(temp) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let ft = e.file_type().ok()?;
            let meta = e.metadata().ok()?;
            let mt = paths::local_naive(meta.modified().ok()?);
            if mt >= cutoff {
                return None;
            }
            let (size, description) = if ft.is_file() {
                (meta.len(), "Temporary system file")
            } else if ft.is_dir() {
                (fsutil::dir_size(&e.path()), "Temporary system directory")
            } else {
                return None;
            };
            Some(JunkItem {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path(),
                category: SYSTEM_TEMP.into(),
                size_bytes: size,
                last_modified: mt,
                description: description.into(),
            })
        })
        .collect()
}

/// Deletes the items (or only reports with `dry_run`).
pub fn clean_junk(items: &[JunkItem], dry_run: bool) -> CleanResult {
    let mut cleaned = 0;
    let mut bytes = 0;
    let mut errors = Vec::new();
    for item in items {
        if !item.path.exists() {
            continue;
        }
        bytes += item.size_bytes;
        cleaned += 1;
        if dry_run {
            continue;
        }
        let res = if item.path.is_dir() {
            fs::remove_dir_all(&item.path)
        } else {
            fs::remove_file(&item.path)
        };
        if let Err(e) = res {
            errors.push(format!(
                "Failed to remove {}: {e}",
                fsutil::name_of(&item.path)
            ));
        }
    }
    CleanResult {
        success: errors.is_empty(),
        cleaned_count: cleaned,
        reclaimed_bytes: bytes,
        dry_run,
        message: format!(
            "{} {:.1} MB across {cleaned} items.",
            if dry_run {
                "[DRY RUN] Would reclaim"
            } else {
                "Reclaimed"
            },
            bytes as f64 / (1024.0 * 1024.0)
        ),
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn finds_caches_and_skips_git_and_fresh_dirs() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        fs::create_dir_all(r.join("proj/__pycache__")).unwrap();
        fs::write(r.join("proj/__pycache__/a.pyc"), vec![0u8; 100]).unwrap();
        fs::create_dir_all(r.join("proj/node_modules/x/node_modules")).unwrap();
        fs::write(r.join("proj/node_modules/x/index.js"), "x").unwrap();
        fs::create_dir_all(r.join(".git/__pycache__")).unwrap();

        // inactive_days = -1 makes every target folder count as inactive.
        let items = scan_directory(r, -1);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"__pycache__"));
        assert!(
            items
                .iter()
                .all(|i| !i.path.starts_with(paths::resolve(&r.join(".git"))))
        );
        let pyc = items.iter().find(|i| i.name == "__pycache__").unwrap();
        assert_eq!(pyc.size_bytes, 100);
        assert_eq!(pyc.category, PYTHON_CACHE);
        // Matched folders are not descended into: nested node_modules not listed.
        assert_eq!(items.iter().filter(|i| i.name == "node_modules").count(), 1);
        assert!(
            scan_directory(r, 14)
                .iter()
                .all(|i| i.name != "node_modules")
        );

        let dry = clean_junk(&items, true);
        assert!(dry.dry_run && dry.message.starts_with("[DRY RUN] Would reclaim"));
        assert!(r.join("proj/__pycache__").exists());
        let live = clean_junk(&items, false);
        assert!(live.success, "{live:?}");
        assert!(!r.join("proj/__pycache__").exists());
    }

    #[test]
    fn temp_scan_by_age() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("old.tmp");
        fs::write(&old, "abc").unwrap();
        fsutil::set_mtime(&old, SystemTime::now() - Duration::from_secs(10 * 86_400)).unwrap();
        fs::write(d.path().join("new.tmp"), "x").unwrap();
        let items = scan_temp_dir(d.path(), 3);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "old.tmp");
        assert_eq!(items[0].size_bytes, 3);
    }
}
