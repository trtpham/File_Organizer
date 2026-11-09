//! Exact (3-tier hashing) and visual duplicate detection and resolution.

use crate::config::Config;
use crate::error::SfoError;
use crate::hasher;
use crate::models::{ActionRecord, ActionType, DuplicateGroup, FileInfo, VisualDuplicateGroup};
use crate::progress::{self, Control};
use crate::visual_hasher::{self, VisualSimilarityEngine};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;

/// How duplicates are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DedupAction {
    #[default]
    MoveToDuplicates,
    Delete,
}

impl DedupAction {
    /// `"delete"` → Delete; anything else moves, like Python.
    pub fn parse(s: &str) -> Self {
        if s == "delete" {
            Self::Delete
        } else {
            Self::MoveToDuplicates
        }
    }
}

/// Progress callback: `(stage, survivors, inputs)` with stage
/// `tier1_complete` / `tier2_complete` / `tier3_complete`.
pub type DedupProgress<'a> = &'a mut dyn FnMut(&str, usize, usize);

/// Groups `items` by `key`, keeping first-seen group order.
fn group_ordered<T, K: Hash + Eq>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> Option<K>,
) -> Vec<Vec<T>> {
    let mut index: HashMap<K, usize> = HashMap::new();
    let mut groups: Vec<Vec<T>> = Vec::new();
    for item in items {
        let Some(k) = key(&item) else { continue };
        match index.get(&k) {
            Some(&i) => groups[i].push(item),
            None => {
                index.insert(k, groups.len());
                groups.push(vec![item]);
            }
        }
    }
    groups
}

/// First element with the greatest key (Python `max` semantics).
fn first_max_by<T, K: PartialOrd>(items: &[T], key: impl Fn(&T) -> K) -> Option<&T> {
    let mut best: Option<(&T, K)> = None;
    for it in items {
        let k = key(it);
        if best.as_ref().is_none_or(|(_, bk)| k > *bk) {
            best = Some((it, k));
        }
    }
    best.map(|(t, _)| t)
}

/// First element with the smallest key (Python `min` semantics).
fn first_min_by<T, K: PartialOrd>(items: &[T], key: impl Fn(&T) -> K) -> Option<&T> {
    let mut best: Option<(&T, K)> = None;
    for it in items {
        let k = key(it);
        if best.as_ref().is_none_or(|(_, bk)| k < *bk) {
            best = Some((it, k));
        }
    }
    best.map(|(t, _)| t)
}

/// Finds and plans resolution of duplicate files.
#[derive(Debug, Clone)]
pub struct Deduplicator<'c> {
    config: &'c Config,
}

impl<'c> Deduplicator<'c> {
    pub fn new(config: &'c Config) -> Self {
        Self { config }
    }

    fn setting(&self, key: &str, default: &str) -> String {
        self.config
            .deduplication_settings()
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    }

    /// Size → partial hash → full SHA-256. Zero-byte files are ignored.
    pub fn find_duplicates(
        &self,
        files: &[FileInfo],
        progress: Option<DedupProgress<'_>>,
    ) -> Vec<DuplicateGroup> {
        self.find_duplicates_ctl(files, progress, None)
            .expect("only a Control can cancel")
    }

    /// [`Self::find_duplicates`] with progress stages and cancellation.
    pub fn find_duplicates_ctl(
        &self,
        files: &[FileInfo],
        mut progress: Option<DedupProgress<'_>>,
        ctl: Option<&Control>,
    ) -> crate::Result<Vec<DuplicateGroup>> {
        let check = || {
            if progress::cancelled(ctl) {
                Err(SfoError::Cancelled)
            } else {
                Ok(())
            }
        };
        let tier1: Vec<Vec<FileInfo>> =
            group_ordered(files.iter().filter(|f| f.size_bytes > 0).cloned(), |f| {
                Some(f.size_bytes)
            })
            .into_iter()
            .filter(|g| g.len() > 1)
            .collect();
        if let Some(cb) = progress.as_mut() {
            cb("tier1_complete", tier1.len(), files.len());
        }

        let mut flat1: Vec<FileInfo> = tier1.iter().flatten().cloned().collect();
        progress::stage(ctl, "Hashing file starts", flat1.len() as u64);
        hasher::populate_hashes_ctl(&mut flat1, false, None, None, ctl);
        check()?;
        let tier2: Vec<Vec<FileInfo>> =
            group_ordered(flat1, |f| f.partial_hash.clone().map(|h| (f.size_bytes, h)))
                .into_iter()
                .filter(|g| g.len() > 1)
                .collect();
        if let Some(cb) = progress.as_mut() {
            cb("tier2_complete", tier2.len(), tier1.len());
        }

        let mut flat2: Vec<FileInfo> = tier2.iter().flatten().cloned().collect();
        progress::stage(ctl, "Hashing full files", flat2.len() as u64);
        hasher::populate_hashes_ctl(&mut flat2, true, None, None, ctl);
        check()?;
        let policy = self.setting("keep_policy", "oldest");
        let groups: Vec<DuplicateGroup> =
            group_ordered(flat2, |f| f.full_hash.clone().map(|h| (f.size_bytes, h)))
                .into_iter()
                .filter(|g| g.len() > 1)
                .map(|g| DuplicateGroup {
                    file_hash: g[0].full_hash.clone().unwrap_or_default(),
                    file_size: g[0].size_bytes,
                    suggested_keep: select_keep_file(&g, &policy).cloned(),
                    files: g,
                })
                .collect();
        if let Some(cb) = progress.as_mut() {
            cb("tier3_complete", groups.len(), tier2.len());
        }
        Ok(groups)
    }

    /// One record per non-kept file: `DEDUPLICATE` into `<dest>/_Duplicates`
    /// or `DELETE`.
    pub fn plan_deduplication_actions(
        &self,
        groups: &[DuplicateGroup],
        destination_dir: &Path,
        mode: DedupAction,
    ) -> Vec<ActionRecord> {
        let root = destination_dir.join(self.setting("duplicates_folder_name", "_Duplicates"));
        let mut records = Vec::new();
        for g in groups {
            let keep_name = keep_name(g.suggested_keep.as_ref());
            for f in &g.files {
                if g.suggested_keep.as_ref().is_some_and(|k| k.path == f.path) {
                    continue;
                }
                let mut r = match mode {
                    DedupAction::Delete => {
                        ActionRecord::new(ActionType::Delete, f.path.clone(), None, f.size_bytes)
                    }
                    DedupAction::MoveToDuplicates => ActionRecord::new(
                        ActionType::Deduplicate,
                        f.path.clone(),
                        Some(root.join(&f.filename)),
                        f.size_bytes,
                    ),
                };
                r.file_hash = f.full_hash.clone();
                r.message = format!("Duplicate of {keep_name}");
                records.push(r);
            }
        }
        records
    }

    pub fn find_visual_duplicates(
        &self,
        files: &[FileInfo],
        similarity_threshold: f64,
    ) -> Vec<VisualDuplicateGroup> {
        VisualSimilarityEngine::new(similarity_threshold).find_visual_duplicates(files)
    }

    /// [`Self::find_visual_duplicates`] with progress and cancellation.
    pub fn find_visual_duplicates_ctl(
        &self,
        files: &[FileInfo],
        similarity_threshold: f64,
        ctl: Option<&Control>,
    ) -> crate::Result<Vec<VisualDuplicateGroup>> {
        VisualSimilarityEngine::new(similarity_threshold).find_visual_duplicates_ctl(files, ctl)
    }

    /// Like [`Self::plan_deduplication_actions`], into `_Duplicates/Visual_Duplicates`.
    pub fn plan_visual_deduplication_actions(
        &self,
        groups: &[VisualDuplicateGroup],
        destination_dir: &Path,
        mode: DedupAction,
    ) -> Vec<ActionRecord> {
        let root = destination_dir
            .join(self.setting("duplicates_folder_name", "_Duplicates"))
            .join("Visual_Duplicates");
        let mut records = Vec::new();
        for g in groups {
            let msg = format!(
                "Visual duplicate of {} (Sim: {:.0}%)",
                keep_name(g.suggested_keep.as_ref()),
                g.similarity_score * 100.0
            );
            for f in &g.files {
                if g.suggested_keep.as_ref().is_some_and(|k| k.path == f.path) {
                    continue;
                }
                let mut r = match mode {
                    DedupAction::Delete => {
                        ActionRecord::new(ActionType::Delete, f.path.clone(), None, f.size_bytes)
                    }
                    DedupAction::MoveToDuplicates => ActionRecord::new(
                        ActionType::Deduplicate,
                        f.path.clone(),
                        Some(root.join(&f.filename)),
                        f.size_bytes,
                    ),
                };
                r.file_hash = Some(g.phash.clone());
                r.message = msg.clone();
                records.push(r);
            }
        }
        records
    }
}

fn keep_name(keep: Option<&FileInfo>) -> String {
    keep.and_then(|k| k.path.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".into())
}

/// Keep policies: `oldest` (default), `newest`, `largest`, `shortest_path`,
/// `highest_resolution`.
pub fn select_keep_file<'a>(files: &'a [FileInfo], policy: &str) -> Option<&'a FileInfo> {
    match policy {
        "newest" => first_max_by(files, |f| f.modified_time),
        "largest" => first_max_by(files, |f| f.size_bytes),
        "shortest_path" => first_min_by(files, |f| f.path.as_os_str().len()),
        "highest_resolution" => {
            let mut best = files.first();
            let mut max_px: Option<u64> = None;
            for f in files {
                if let Some(px) = visual_hasher::pixel_count(&f.path)
                    && max_px.is_none_or(|m| px > m)
                {
                    max_px = Some(px);
                    best = Some(f);
                }
            }
            best
        }
        _ => first_min_by(files, |f| f.created_time),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::FileScanner;

    fn workspace() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let dup = b"DUPLICATE_FILE_CONTENT_IDENTICAL_1234567890".repeat(200);
        for n in ["original_file.pdf", "copy_of_file.pdf", "another_copy.pdf"] {
            std::fs::write(d.path().join(n), &dup).unwrap();
        }
        // Same size, different content: survives tier 1, dies at tier 2.
        let mut near = dup.clone();
        near[0] = b'X';
        std::fs::write(d.path().join("near.pdf"), near).unwrap();
        std::fs::write(d.path().join("empty1.txt"), b"").unwrap();
        std::fs::write(d.path().join("empty2.txt"), b"").unwrap();
        std::fs::write(d.path().join("unique.txt"), b"unique").unwrap();
        d
    }

    #[test]
    fn finds_exact_duplicates() {
        let d = workspace();
        let cfg = Config::defaults();
        let files = FileScanner::new(&cfg).scan_dir(d.path()).unwrap();
        let mut stages = Vec::new();
        let mut cb = |s: &str, a: usize, b: usize| stages.push((s.to_string(), a, b));
        let groups = Deduplicator::new(&cfg).find_duplicates(&files, Some(&mut cb));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 3);
        assert!(groups[0].suggested_keep.is_some());
        assert_eq!(groups[0].wasted_space_bytes(), 2 * groups[0].file_size);
        assert_eq!(stages[0], ("tier1_complete".into(), 1, 7));
        assert_eq!(stages[2].0, "tier3_complete");
    }

    #[test]
    fn plans_moves_and_deletes() {
        let d = workspace();
        let cfg = Config::defaults();
        let files = FileScanner::new(&cfg).scan_dir(d.path()).unwrap();
        let dd = Deduplicator::new(&cfg);
        let groups = dd.find_duplicates(&files, None);

        let moves = dd.plan_deduplication_actions(&groups, d.path(), DedupAction::MoveToDuplicates);
        assert_eq!(moves.len(), 2);
        for r in &moves {
            assert_eq!(r.action_type, ActionType::Deduplicate);
            assert_eq!(
                r.destination_path
                    .as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap(),
                "_Duplicates"
            );
            assert!(r.message.starts_with("Duplicate of "));
        }
        let deletes =
            dd.plan_deduplication_actions(&groups, d.path(), DedupAction::parse("delete"));
        assert!(
            deletes
                .iter()
                .all(|r| r.action_type == ActionType::Delete && r.destination_path.is_none())
        );
    }

    #[test]
    fn keep_policies_use_first_extreme() {
        let d = tempfile::tempdir().unwrap();
        let mk = |n: &str, s: usize| {
            let p = d.path().join(n);
            std::fs::write(&p, vec![b'a'; s]).unwrap();
            FileInfo::from_path(&p).unwrap()
        };
        let files = vec![mk("aa.bin", 5), mk("b.bin", 9), mk("c.bin", 9)];
        assert_eq!(
            select_keep_file(&files, "largest").unwrap().filename,
            "b.bin"
        );
        assert_eq!(
            select_keep_file(&files, "shortest_path").unwrap().filename,
            "b.bin"
        );
        assert_eq!(
            select_keep_file(&files, "highest_resolution")
                .unwrap()
                .filename,
            "aa.bin"
        );
        assert!(select_keep_file(&[], "oldest").is_none());
    }
}
