//! Directory scanner with ignore rules and metadata extraction.
//!
//! Mirrors Python `os.walk(followlinks=False)` ordering: each directory's
//! files are yielded before its subdirectories are visited.

use crate::config::Config;
use crate::error::{Result, SfoError};
use crate::models::FileInfo;
use crate::paths;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Scan options. `Default` = non-recursive, no excludes, unlimited depth.
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub recursive: bool,
    pub exclude_dirs: Vec<PathBuf>,
    /// For recursive scans: directories at this depth (root = 0) are listed
    /// but not descended into.
    pub max_depth: Option<usize>,
}

/// Progress callback: `(files_found, current_file_name)`.
pub type ScanProgress<'a> = &'a mut dyn FnMut(usize, &str);

/// Scans directories, filters ignored entries and collects [`FileInfo`].
#[derive(Debug, Clone)]
pub struct FileScanner<'c> {
    config: &'c Config,
}

impl<'c> FileScanner<'c> {
    pub fn new(config: &'c Config) -> Self {
        Self { config }
    }

    /// Non-recursive scan with default options.
    pub fn scan_dir(&self, source_dir: &Path) -> Result<Vec<FileInfo>> {
        self.scan(source_dir, &ScanOptions::default(), None)
    }

    pub fn scan(
        &self,
        source_dir: &Path,
        opts: &ScanOptions,
        mut progress: Option<ScanProgress<'_>>,
    ) -> Result<Vec<FileInfo>> {
        let root = paths::resolve(source_dir);
        if !root.exists() {
            return Err(SfoError::NotFound(root));
        }
        if !root.is_dir() {
            return Err(SfoError::NotADirectory(root));
        }

        let mut found = Vec::new();
        if opts.recursive {
            let excludes: Vec<PathBuf> = opts
                .exclude_dirs
                .iter()
                .map(|p| paths::resolve(p))
                .collect();
            let mut visited = HashSet::new();
            self.walk(
                &root,
                0,
                opts.max_depth,
                &excludes,
                &mut visited,
                &mut found,
                &mut progress,
            );
        } else if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                // `entry.is_file(follow_symlinks=False)`: regular files only.
                let Ok(ft) = entry.file_type() else { continue };
                if !ft.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if self.config.is_ignored(&name) {
                    continue;
                }
                if let Ok(info) = FileInfo::from_path(&entry.path()) {
                    found.push(info);
                    if let Some(cb) = progress.as_mut() {
                        cb(found.len(), &name);
                    }
                }
            }
        }
        Ok(found)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        dir: &Path,
        depth: usize,
        max_depth: Option<usize>,
        excludes: &[PathBuf],
        visited: &mut HashSet<PathBuf>,
        found: &mut Vec<FileInfo>,
        progress: &mut Option<ScanProgress<'_>>,
    ) {
        // Loop protection (junctions / bind mounts).
        if !visited.insert(paths::resolve(dir)) {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            // os.walk: a symlink to a directory is listed as a dir but not followed.
            let is_dir = ft.is_dir() || (ft.is_symlink() && path.is_dir());
            if is_dir {
                if !ft.is_symlink() {
                    subdirs.push((name, path));
                }
                continue;
            }
            if self.config.is_ignored(&name) {
                continue;
            }
            if let Ok(info) = FileInfo::from_path(&path) {
                found.push(info);
                if let Some(cb) = progress.as_mut() {
                    cb(found.len(), &name);
                }
            }
        }

        if max_depth.is_some_and(|m| depth >= m) {
            return;
        }
        for (name, path) in subdirs {
            if self.config.is_ignored(&name) {
                continue;
            }
            let resolved = paths::resolve(&path);
            if excludes.iter().any(|e| paths::path_eq(e, &resolved)) {
                continue;
            }
            self.walk(
                &path,
                depth + 1,
                max_depth,
                excludes,
                visited,
                found,
                progress,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        fs::write(r.join("top.txt"), "t").unwrap();
        fs::write(r.join("skip.tmp"), "x").unwrap();
        fs::create_dir_all(r.join("a/b")).unwrap();
        fs::write(r.join("a/mid.txt"), "m").unwrap();
        fs::write(r.join("a/b/deep.txt"), "d").unwrap();
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".git/HEAD"), "h").unwrap();
        fs::create_dir_all(r.join("excluded")).unwrap();
        fs::write(r.join("excluded/e.txt"), "e").unwrap();
        d
    }

    fn names(files: &[FileInfo]) -> HashSet<String> {
        files.iter().map(|f| f.filename.clone()).collect()
    }

    #[test]
    fn non_recursive_lists_top_level_only() {
        let d = tree();
        let cfg = Config::defaults();
        let files = FileScanner::new(&cfg).scan_dir(d.path()).unwrap();
        assert_eq!(names(&files), HashSet::from(["top.txt".to_string()]));
    }

    #[test]
    fn recursive_respects_ignores_excludes_and_depth() {
        let d = tree();
        let cfg = Config::defaults();
        let s = FileScanner::new(&cfg);
        let opts = ScanOptions {
            recursive: true,
            exclude_dirs: vec![d.path().join("excluded")],
            max_depth: None,
        };
        let all = names(&s.scan(d.path(), &opts, None).unwrap());
        assert_eq!(
            all,
            HashSet::from(["top.txt", "mid.txt", "deep.txt"].map(String::from))
        );

        let opts = ScanOptions {
            recursive: true,
            exclude_dirs: vec![],
            max_depth: Some(1),
        };
        let shallow = names(&s.scan(d.path(), &opts, None).unwrap());
        assert!(shallow.contains("mid.txt") && !shallow.contains("deep.txt"));
    }

    #[test]
    fn progress_and_errors() {
        let d = tree();
        let cfg = Config::defaults();
        let mut calls = 0;
        let mut cb = |_n: usize, _name: &str| calls += 1;
        FileScanner::new(&cfg)
            .scan(d.path(), &ScanOptions::default(), Some(&mut cb))
            .unwrap();
        assert_eq!(calls, 1);
        assert!(matches!(
            FileScanner::new(&cfg).scan_dir(&d.path().join("nope")),
            Err(SfoError::NotFound(_))
        ));
        assert!(matches!(
            FileScanner::new(&cfg).scan_dir(&d.path().join("top.txt")),
            Err(SfoError::NotADirectory(_))
        ));
    }
}
