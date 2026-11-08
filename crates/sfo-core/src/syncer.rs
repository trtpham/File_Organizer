//! Removable-drive detection, incremental folder sync, ZIP snapshots.

use crate::archiver;
use crate::error::Result;
use crate::fsutil;
use crate::models::{SyncMode, SyncProfile};
use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A detected removable volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drive {
    pub path: String,
    pub label: String,
}

/// Removable drives: Windows via the volume API, elsewhere the folders under
/// `/media`, `/Volumes`, `/run/media` (Python v3.0 behaviour).
pub fn removable_drives() -> Vec<Drive> {
    #[cfg(windows)]
    {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks
            .iter()
            .filter(|d| d.is_removable())
            .map(|d| {
                let label = d.name().to_string_lossy().into_owned();
                Drive {
                    path: d.mount_point().display().to_string(),
                    label: if label.is_empty() {
                        "USB Drive".into()
                    } else {
                        label
                    },
                }
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        ["/media", "/Volumes", "/run/media"]
            .iter()
            .filter_map(|r| fs::read_dir(r).ok())
            .flat_map(|it| it.flatten())
            .filter(|e| e.path().is_dir())
            .map(|e| Drive {
                path: paths::resolve(&e.path()).display().to_string(),
                label: e.file_name().to_string_lossy().into_owned(),
            })
            .collect()
    }
}

/// Result of [`sync`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncResult {
    pub success: bool,
    pub copied_count: usize,
    pub skipped_count: usize,
    pub bytes_transferred: u64,
    pub elapsed_seconds: f64,
    pub archive_path: Option<PathBuf>,
    pub errors: Vec<String>,
    pub message: String,
}

/// Runs a profile. `Mirror` behaves like `Incremental` (no deletions), as in
/// Python v3.0.
pub fn sync(profile: &SyncProfile, dry_run: bool) -> SyncResult {
    let source = paths::resolve(&profile.source_dir);
    let dest = paths::resolve(&profile.dest_dir);
    if !source.is_dir() {
        let msg = format!("Source directory does not exist: {}", source.display());
        return SyncResult {
            errors: vec![msg.clone()],
            message: msg,
            ..Default::default()
        };
    }
    let start = Instant::now();
    match profile.mode {
        SyncMode::Snapshot => snapshot(&source, &dest, &profile.name, dry_run, start),
        SyncMode::Incremental | SyncMode::Mirror => incremental(&source, &dest, dry_run, start),
    }
}

fn incremental(source: &Path, dest: &Path, dry_run: bool, start: Instant) -> SyncResult {
    let mut r = SyncResult::default();
    for src in fsutil::walk_files(source) {
        let Ok(rel) = src.strip_prefix(source) else {
            continue;
        };
        let dst = dest.join(rel);
        let name = fsutil::name_of(&src);
        let outcome = (|| -> std::io::Result<bool> {
            let sm = fs::metadata(&src)?;
            let copy = match fs::metadata(&dst) {
                Err(_) => true,
                Ok(dm) => {
                    let newer = match (sm.modified(), dm.modified()) {
                        (Ok(s), Ok(d)) => s
                            .duration_since(d)
                            .is_ok_and(|diff| diff.as_secs_f64() > 1.0),
                        _ => false,
                    };
                    sm.len() != dm.len() || newer
                }
            };
            if copy && !dry_run {
                let parent = dst.parent().unwrap_or(dest);
                fs::create_dir_all(parent)?;
                // Stage then rename so a pulled USB stick never leaves a half file.
                let tmp = parent.join(format!(".{name}.sfo_tmp"));
                fsutil::copy_preserving_mtime(&src, &tmp)?;
                fs::rename(&tmp, &dst)?;
            }
            if copy {
                r.bytes_transferred += sm.len();
            }
            Ok(copy)
        })();
        match outcome {
            Ok(true) => r.copied_count += 1,
            Ok(false) => r.skipped_count += 1,
            Err(e) => r.errors.push(format!("Failed {name}: {e}")),
        }
    }
    r.elapsed_seconds = start.elapsed().as_secs_f64();
    r.success = r.errors.is_empty();
    r.message = format!(
        "Synced {} files ({:.1} MB) in {:.2}s.",
        r.copied_count,
        r.bytes_transferred as f64 / (1024.0 * 1024.0),
        r.elapsed_seconds
    );
    r
}

fn snapshot(
    source: &Path,
    dest: &Path,
    profile_name: &str,
    dry_run: bool,
    start: Instant,
) -> SyncResult {
    let safe: String = profile_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let safe = if safe.is_empty() {
        "Backup".to_string()
    } else {
        safe
    };
    let archive_name = format!(
        "{safe}_{}.zip",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let archive_path = dest.join(&archive_name);
    let files = fsutil::walk_files(source);
    let total_bytes: u64 = files
        .iter()
        .filter_map(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    let mut r = SyncResult {
        copied_count: files.len(),
        bytes_transferred: total_bytes,
        ..Default::default()
    };
    if !dry_run {
        let write = || -> Result<()> {
            fs::create_dir_all(dest)?;
            let mut zip = zip::ZipWriter::new(fs::File::create(&archive_path)?);
            for f in &files {
                let rel = f.strip_prefix(source).unwrap_or(f);
                let name = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                archiver::add_file(&mut zip, f, &name)?;
            }
            zip.finish().map_err(archiver::zip_err)?;
            Ok(())
        };
        if let Err(e) = write() {
            r.errors.push(e.to_string());
        }
    }
    r.elapsed_seconds = start.elapsed().as_secs_f64();
    r.success = r.errors.is_empty();
    r.archive_path = Some(archive_path);
    r.message = format!(
        "Created snapshot archive '{archive_name}' ({:.1} MB) in {:.2}s.",
        total_bytes as f64 / (1024.0 * 1024.0),
        r.elapsed_seconds
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(src: &Path, dst: &Path, mode: SyncMode) -> SyncProfile {
        SyncProfile {
            name: "Game Saves!".into(),
            source_dir: src.into(),
            dest_dir: dst.into(),
            mode,
            enabled: true,
            trigger_on_usb: false,
            usb_volume_name: None,
        }
    }

    #[test]
    fn incremental_copies_only_changes() {
        let d = tempfile::tempdir().unwrap();
        let (src, dst) = (d.path().join("src"), d.path().join("dst"));
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("a.txt"), "aaa").unwrap();
        fs::write(src.join("nested/b.txt"), "bb").unwrap();
        let p = profile(&src, &dst, SyncMode::Incremental);

        let dry = sync(&p, true);
        assert_eq!(dry.copied_count, 2);
        assert!(!dst.exists());

        let first = sync(&p, false);
        assert!(first.success, "{first:?}");
        assert_eq!((first.copied_count, first.bytes_transferred), (2, 5));
        assert_eq!(fs::read_to_string(dst.join("nested/b.txt")).unwrap(), "bb");

        let second = sync(&p, false);
        assert_eq!((second.copied_count, second.skipped_count), (0, 2));

        fs::write(src.join("a.txt"), "changed").unwrap();
        assert_eq!(sync(&p, false).copied_count, 1);
        assert!(
            fs::read_dir(&dst)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().ends_with(".sfo_tmp"))
        );
        assert!(
            !sync(
                &profile(&d.path().join("nope"), &dst, SyncMode::Incremental),
                false
            )
            .success
        );
    }

    #[test]
    fn snapshot_zip() {
        let d = tempfile::tempdir().unwrap();
        let (src, dst) = (d.path().join("src"), d.path().join("dst"));
        fs::create_dir_all(src.join("n")).unwrap();
        fs::write(src.join("n/x.txt"), "x").unwrap();
        let r = sync(&profile(&src, &dst, SyncMode::Snapshot), false);
        assert!(r.success, "{r:?}");
        let zip_path = r.archive_path.unwrap();
        assert!(fsutil::name_of(&zip_path).starts_with("GameSaves_"));
        let mut z = zip::ZipArchive::new(fs::File::open(zip_path).unwrap()).unwrap();
        assert!(z.by_name("n/x.txt").is_ok());
    }

    #[test]
    fn drive_listing_does_not_panic() {
        let _ = removable_drives();
    }
}
