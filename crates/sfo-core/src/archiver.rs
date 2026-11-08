//! Cold-storage analyzer and ZIP archiver with an embedded manifest.
//!
//! Archive layout is shared with Python v3.0: Deflate level 9 members plus
//! `sfo_archive_manifest.json` mapping members back to original paths.

use crate::error::{Result, SfoError};
use crate::fsutil;
use crate::models::FileInfo;
use crate::paths;
use chrono::{Datelike, Local, NaiveDateTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use zip::write::SimpleFileOptions;

pub const MANIFEST_NAME: &str = "sfo_archive_manifest.json";

/// Files untouched (mtime and atime) for at least `inactive_days`, largest
/// first. Hidden (`.`) and temp (`~`) names are skipped.
pub fn scan_cold_files(
    source_dir: &Path,
    inactive_days: u64,
    min_size_bytes: u64,
    recursive: bool,
) -> Vec<FileInfo> {
    let root = paths::resolve(source_dir);
    if !root.exists() {
        return Vec::new();
    }
    let threshold = Duration::from_secs(inactive_days * 86_400);
    let now = SystemTime::now();
    let files = if recursive {
        fsutil::walk_files(&root)
    } else {
        fsutil::list_files(&root)
    };
    let mut cold: Vec<FileInfo> = files
        .into_iter()
        .filter(|p| {
            let n = fsutil::name_of(p);
            !n.starts_with('.') && !n.starts_with('~')
        })
        .filter_map(|p| {
            let meta = fs::metadata(&p).ok()?;
            let m = meta.modified().ok()?;
            let last = meta.accessed().ok().map_or(m, |a| a.max(m));
            let age = now.duration_since(last).unwrap_or_default();
            (age >= threshold && meta.len() >= min_size_bytes)
                .then(|| FileInfo::from_path(&p).ok())
                .flatten()
        })
        .collect();
    cold.sort_by_key(|f| std::cmp::Reverse(f.size_bytes));
    cold
}

/// One manifest entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub original_path: String,
    pub archive_name: String,
    pub size: u64,
    pub mtime: String,
}

/// The JSON manifest stored inside archives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub created_at: String,
    pub file_count: usize,
    pub total_uncompressed_bytes: u64,
    pub files: Vec<ManifestEntry>,
}

/// Result of [`create_cold_archive`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArchiveResult {
    pub success: bool,
    pub archive_path: Option<PathBuf>,
    pub archived_count: usize,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub bytes_saved: u64,
    pub deleted_sources_count: usize,
    pub message: String,
}

/// ZIP timestamp for a local naive time (clamped to the ZIP range).
pub fn zip_datetime(dt: &NaiveDateTime) -> zip::DateTime {
    zip::DateTime::from_date_and_time(
        dt.year().clamp(1980, 2107) as u16,
        dt.month() as u8,
        dt.day() as u8,
        dt.hour() as u8,
        dt.minute() as u8,
        dt.second() as u8,
    )
    .unwrap_or_default()
}

/// Deflate level 9 options stamped with `path`'s mtime.
pub fn deflate_options_for(path: &Path) -> SimpleFileOptions {
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .large_file(true);
    match fs::metadata(path).and_then(|m| m.modified()) {
        Ok(m) => opts.last_modified_time(zip_datetime(&paths::local_naive(m))),
        Err(_) => opts,
    }
}

/// Adds `src` to `zip` under `name`, streaming its contents.
pub fn add_file<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    src: &Path,
    name: &str,
) -> Result<()> {
    zip.start_file(name, deflate_options_for(src))
        .map_err(zip_err)?;
    std::io::copy(&mut fs::File::open(src)?, zip)?;
    Ok(())
}

/// Reads every member fully so CRC mismatches surface (Python `testzip`).
pub fn verify_zip<R: Read + Seek>(reader: R) -> Result<()> {
    let mut z = zip::ZipArchive::new(reader).map_err(zip_err)?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(zip_err)?;
        let name = f.name().to_string();
        std::io::copy(&mut f, &mut std::io::sink()).map_err(|e| {
            SfoError::Config(format!(
                "Archive verification failed! Corrupted member: {name} ({e})"
            ))
        })?;
    }
    Ok(())
}

pub(crate) fn zip_err(e: zip::result::ZipError) -> SfoError {
    SfoError::Io(std::io::Error::other(e))
}

/// Compresses `files`; with `delete_sources`, removes originals only after
/// the archive has been closed and verified.
pub fn create_cold_archive(
    files: &[FileInfo],
    archive_path: &Path,
    base_dir: Option<&Path>,
    delete_sources: bool,
    mut progress: Option<&mut dyn FnMut(usize, usize)>,
) -> ArchiveResult {
    if files.is_empty() {
        return ArchiveResult {
            message: "No files specified for archiving.".into(),
            ..Default::default()
        };
    }
    let archive_path = paths::resolve(archive_path);
    let mut build = || -> Result<(Vec<ManifestEntry>, u64)> {
        if let Some(dir) = archive_path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive_path)?);
        let mut entries = Vec::new();
        let mut raw = 0;
        for (i, f) in files.iter().enumerate() {
            if !f.path.exists() {
                continue;
            }
            raw += f.size_bytes;
            let name = base_dir
                .and_then(|b| f.path.strip_prefix(paths::resolve(b)).ok())
                .map(|rel| {
                    rel.components()
                        .map(|c| c.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_else(|| f.filename.clone());
            add_file(&mut zip, &f.path, &name)?;
            entries.push(ManifestEntry {
                original_path: f.path.display().to_string(),
                archive_name: name,
                size: f.size_bytes,
                mtime: paths::py_isoformat(&f.modified_time),
            });
            if let Some(cb) = progress.as_mut() {
                cb(i + 1, files.len());
            }
        }
        let manifest = Manifest {
            created_at: paths::py_isoformat_utc(&chrono::Utc::now()),
            file_count: entries.len(),
            total_uncompressed_bytes: raw,
            files: entries.clone(),
        };
        zip.start_file(
            MANIFEST_NAME,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(zip_err)?;
        zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
        zip.finish().map_err(zip_err)?;
        verify_zip(fs::File::open(&archive_path)?)?;
        Ok((entries, raw))
    };

    match build() {
        Ok((entries, raw)) => {
            let compressed = fs::metadata(&archive_path).map(|m| m.len()).unwrap_or(0);
            let saved = raw.saturating_sub(compressed);
            let mut deleted = 0;
            if delete_sources {
                for e in &entries {
                    let p = Path::new(&e.original_path);
                    if p.is_file() && fs::remove_file(p).is_ok() {
                        deleted += 1;
                    }
                }
            }
            ArchiveResult {
                success: true,
                archive_path: Some(archive_path),
                archived_count: entries.len(),
                uncompressed_bytes: raw,
                compressed_bytes: compressed,
                bytes_saved: saved,
                deleted_sources_count: deleted,
                message: format!(
                    "Successfully archived {} files ({saved} bytes saved).",
                    entries.len()
                ),
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&archive_path);
            ArchiveResult {
                message: format!("Archive creation failed: {e}"),
                ..Default::default()
            }
        }
    }
}

/// Result of [`restore_archive`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RestoreResult {
    pub success: bool,
    pub restored_count: usize,
    pub restored: Vec<PathBuf>,
    pub skipped_unsafe: Vec<String>,
    pub message: String,
}

/// Extracts to `target_dir`, else to each manifest `original_path`, else
/// next to the archive. Recorded mtimes are restored. Member names that
/// would escape the target folder are skipped.
pub fn restore_archive(archive_path: &Path, target_dir: Option<&Path>) -> RestoreResult {
    let archive_path = paths::resolve(archive_path);
    if !archive_path.exists() {
        return RestoreResult {
            message: format!("Archive not found: {}", archive_path.display()),
            ..Default::default()
        };
    }
    let run = || -> Result<RestoreResult> {
        let mut z = zip::ZipArchive::new(fs::File::open(&archive_path)?).map_err(zip_err)?;
        let manifest: Option<Manifest> = z
            .by_name(MANIFEST_NAME)
            .ok()
            .and_then(|mut f| {
                let mut s = String::new();
                f.read_to_string(&mut s).ok()?;
                Some(s)
            })
            .and_then(|s| serde_json::from_str(&s).ok());
        let parent = archive_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let mut res = RestoreResult::default();
        for i in 0..z.len() {
            let mut member = z.by_index(i).map_err(zip_err)?;
            let name = member.name().to_string();
            if name == MANIFEST_NAME || member.is_dir() {
                continue;
            }
            let entry = manifest
                .as_ref()
                .and_then(|m| m.files.iter().find(|e| e.archive_name == name));
            let Some(rel) = member.enclosed_name() else {
                res.skipped_unsafe.push(name);
                continue;
            };
            let dest = match (target_dir, entry) {
                (Some(t), _) => t.join(&rel),
                (None, Some(e)) => PathBuf::from(&e.original_path),
                (None, None) => parent.join(&rel),
            };
            if let Some(d) = dest.parent() {
                fs::create_dir_all(d)?;
            }
            std::io::copy(&mut member, &mut fs::File::create(&dest)?)?;
            if let Some(dt) = entry.and_then(|e| paths::py_fromisoformat(&e.mtime))
                && let Some(local) = Local.from_local_datetime(&dt).earliest()
            {
                let _ = fsutil::set_mtime(&dest, local.into());
            }
            res.restored.push(dest);
        }
        res.success = true;
        res.restored_count = res.restored.len();
        res.message = format!(
            "Successfully restored {} files from {}.",
            res.restored_count,
            fsutil::name_of(&archive_path)
        );
        Ok(res)
    };
    run().unwrap_or_else(|e| RestoreResult {
        message: format!("Failed to restore archive: {e}"),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_verify_delete_and_restore() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().join("data");
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/a.txt"), "aaaa".repeat(500)).unwrap();
        fs::write(base.join("b.txt"), "bbbb".repeat(500)).unwrap();
        let old = SystemTime::now() - Duration::from_secs(400 * 86_400);
        for p in [base.join("sub/a.txt"), base.join("b.txt")] {
            fsutil::set_mtime(&p, old).unwrap();
        }
        let files: Vec<FileInfo> = fsutil::walk_files(&base)
            .iter()
            .map(|p| FileInfo::from_path(p).unwrap())
            .collect();

        let zip_path = d.path().join("out/cold.zip");
        let mut calls = 0;
        let mut cb = |_: usize, _: usize| calls += 1;
        let res = create_cold_archive(&files, &zip_path, Some(&base), true, Some(&mut cb));
        assert!(res.success, "{res:?}");
        assert_eq!(
            (res.archived_count, res.deleted_sources_count, calls),
            (2, 2, 2)
        );
        assert!(res.bytes_saved > 0);
        assert!(!base.join("b.txt").exists());

        let r = restore_archive(&zip_path, None);
        assert!(r.success, "{r:?}");
        assert_eq!(r.restored_count, 2);
        assert_eq!(
            fs::read_to_string(base.join("sub/a.txt")).unwrap(),
            "aaaa".repeat(500)
        );
        let mt = fs::metadata(base.join("sub/a.txt"))
            .unwrap()
            .modified()
            .unwrap();
        assert!(mt.duration_since(old).unwrap_or_else(|e| e.duration()) < Duration::from_secs(3));

        let r = restore_archive(&zip_path, Some(&d.path().join("elsewhere")));
        assert!(d.path().join("elsewhere/sub/a.txt").exists(), "{r:?}");
        assert!(!create_cold_archive(&[], &zip_path, None, false, None).success);
    }

    #[test]
    fn zip_slip_members_are_skipped() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("evil.zip");
        let mut z = zip::ZipWriter::new(fs::File::create(&p).unwrap());
        z.start_file("../../escape.txt", SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"x").unwrap();
        z.start_file("ok.txt", SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"y").unwrap();
        z.finish().unwrap();
        let r = restore_archive(&p, Some(&d.path().join("t")));
        assert_eq!(r.restored_count, 1);
        assert_eq!(r.skipped_unsafe, ["../../escape.txt"]);
    }

    #[test]
    fn cold_scan_filters_age_hidden_and_size() {
        let d = tempfile::tempdir().unwrap();
        let old = SystemTime::now() - Duration::from_secs(200 * 86_400);
        for (n, s) in [("old.bin", 50), (".hidden", 50), ("tiny.bin", 1)] {
            let p = d.path().join(n);
            fs::write(&p, vec![0u8; s]).unwrap();
            let f = fs::File::options().write(true).open(&p).unwrap();
            f.set_times(fs::FileTimes::new().set_modified(old).set_accessed(old))
                .unwrap();
        }
        fs::write(d.path().join("new.bin"), vec![0u8; 50]).unwrap();
        let cold = scan_cold_files(d.path(), 90, 10, true);
        assert_eq!(
            cold.iter().map(|f| f.filename.as_str()).collect::<Vec<_>>(),
            ["old.bin"]
        );
    }
}
