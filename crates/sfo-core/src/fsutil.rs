//! Filesystem helpers shared by the P3 modules (Python `os.walk` style).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Every regular file under `root` (recursive, symlinked dirs not followed),
/// in `os.walk` order: a directory's files before its subdirectories.
pub fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_into(root, &mut out);
    out
}

fn walk_into(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let p = e.path();
        if ft.is_dir() {
            subdirs.push(p);
        } else if ft.is_symlink() {
            if !p.is_dir() {
                out.push(p);
            }
        } else {
            out.push(p);
        }
    }
    for d in subdirs {
        walk_into(&d, out);
    }
}

/// Immediate children that are files (no recursion).
pub fn list_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

/// Total bytes of all files below `dir`.
pub fn dir_size(dir: &Path) -> u64 {
    walk_files(dir)
        .iter()
        .filter_map(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

/// Sets a file's modification time.
pub fn set_mtime(path: &Path, mtime: SystemTime) -> io::Result<()> {
    fs::File::options()
        .write(true)
        .open(path)?
        .set_modified(mtime)
}

/// `shutil.copy2`: copy bytes and permissions, then keep the source mtime.
pub fn copy_preserving_mtime(src: &Path, dst: &Path) -> io::Result<u64> {
    let n = fs::copy(src, dst)?;
    if let Ok(m) = fs::metadata(src).and_then(|m| m.modified()) {
        let _ = set_mtime(dst, m);
    }
    Ok(n)
}

/// Clears the read-only flag (owner write bit on Unix).
pub fn make_writable(path: &Path) {
    let Ok(meta) = fs::metadata(path) else { return };
    let mut perms = meta.permissions();
    if !perms.readonly() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(perms.mode() | 0o200);
    }
    #[cfg(not(unix))]
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    let _ = fs::set_permissions(path, perms);
}

/// Cryptographically random bytes from the OS.
pub fn random_bytes(n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).expect("OS random number generator unavailable");
    buf
}

/// File name as a `String` (`""` when absent).
pub fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_order_and_sizes() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("a/b")).unwrap();
        fs::write(d.path().join("top.txt"), "12345").unwrap();
        fs::write(d.path().join("a/mid.txt"), "12").unwrap();
        fs::write(d.path().join("a/b/deep.txt"), "1").unwrap();
        let names: Vec<String> = walk_files(d.path()).iter().map(|p| name_of(p)).collect();
        assert_eq!(names, ["top.txt", "mid.txt", "deep.txt"]);
        assert_eq!(dir_size(d.path()), 8);
        assert_eq!(list_files(d.path()).len(), 1);
    }

    #[test]
    fn copy_keeps_mtime_and_writable() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        fs::write(&a, "x").unwrap();
        let old = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        set_mtime(&a, old).unwrap();
        copy_preserving_mtime(&a, &b).unwrap();
        assert_eq!(fs::metadata(&b).unwrap().modified().unwrap(), old);

        let mut p = fs::metadata(&a).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&a, p).unwrap();
        make_writable(&a);
        assert!(!fs::metadata(&a).unwrap().permissions().readonly());
        assert_eq!(random_bytes(16).len(), 16);
    }
}
