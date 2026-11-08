//! Disk usage tree and slice-and-dice treemap layout.

use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};

pub const PALETTE: [&str; 10] = [
    "#38bdf8", "#818cf8", "#a78bfa", "#f472b6", "#fb7185", "#f59e0b", "#10b981", "#06b6d4",
    "#2dd4bf", "#e879f9",
];

/// A file or directory with its total size.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub children: Vec<DiskNode>,
    /// `(x1, y1, x2, y2)`.
    pub rect: (f64, f64, f64, f64),
    pub color: &'static str,
}

impl DiskNode {
    fn new(name: String, path: PathBuf, is_dir: bool) -> Self {
        Self {
            name,
            path,
            is_dir,
            size: 0,
            children: Vec::new(),
            rect: (0.0, 0.0, 0.0, 0.0),
            color: PALETTE[0],
        }
    }
}

/// Size tree of `root`; nodes deeper than `max_depth` are summarized.
pub fn analyze_directory(root: &Path, max_depth: usize) -> DiskNode {
    let root = paths::resolve(root);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let mut node = DiskNode::new(name, root, true);
    populate(&mut node, 0, max_depth);
    node
}

fn populate(node: &mut DiskNode, depth: usize, max_depth: usize) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(&node.path) {
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if ft.is_file() {
                let Ok(meta) = e.metadata() else { continue };
                total += meta.len();
                if depth < max_depth {
                    let mut child = DiskNode::new(name, e.path(), false);
                    child.size = meta.len();
                    node.children.push(child);
                }
            } else if ft.is_dir() {
                let mut child = DiskNode::new(name, e.path(), true);
                child.size = if depth < max_depth {
                    populate(&mut child, depth + 1, max_depth)
                } else {
                    crate::fsutil::dir_size(&child.path)
                };
                total += child.size;
                node.children.push(child);
            }
        }
    }
    node.size = total;
    node.children.sort_by_key(|c| std::cmp::Reverse(c.size));
    total
}

/// One entry of [`find_largest_files`].
#[derive(Debug, Clone, PartialEq)]
pub struct LargeFile {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub size_mb: f64,
}

/// Largest files below `root`, biggest first.
pub fn find_largest_files(root: &Path, limit: usize) -> Vec<LargeFile> {
    let mut files: Vec<LargeFile> = crate::fsutil::walk_files(&paths::resolve(root))
        .into_iter()
        .filter_map(|p| {
            let size = fs::metadata(&p).ok()?.len();
            Some(LargeFile {
                name: crate::fsutil::name_of(&p),
                path: p,
                size,
                size_mb: size as f64 / (1024.0 * 1024.0),
            })
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.size));
    files.truncate(limit);
    files
}

/// Slice-and-dice layout of `node.children` inside the box. With
/// `recursive`, one more level of directory children is laid out.
pub fn layout_treemap(
    node: &mut DiskNode,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color_idx: usize,
    recursive: bool,
) {
    node.rect = (x1, y1, x2, y2);
    let (w, h) = (x2 - x1, y2 - y1);
    if node.children.is_empty() || node.size == 0 || w <= 1.0 || h <= 1.0 {
        return;
    }
    let total: u64 = node.children.iter().map(|c| c.size).sum();
    if total == 0 {
        return;
    }
    let horizontal = w >= h;
    let mut offset = if horizontal { x1 } else { y1 };
    for (idx, child) in node.children.iter_mut().enumerate() {
        let ratio = child.size as f64 / total as f64;
        child.color = PALETTE[(color_idx + idx) % PALETTE.len()];
        child.rect = if horizontal {
            let cw = w * ratio;
            let r = (offset, y1, offset + cw, y2);
            offset += cw;
            r
        } else {
            let ch = h * ratio;
            let r = (x1, offset, x2, offset + ch);
            offset += ch;
            r
        };
        if recursive && child.is_dir && !child.children.is_empty() {
            let (a, b, c, d) = child.rect;
            layout_treemap(child, a, b, c, d, color_idx + idx + 1, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_sort_and_layout() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("big/deep/deeper")).unwrap();
        fs::write(d.path().join("big/a.bin"), vec![0u8; 600]).unwrap();
        fs::write(d.path().join("big/deep/deeper/z.bin"), vec![0u8; 200]).unwrap();
        fs::write(d.path().join("small.txt"), vec![0u8; 200]).unwrap();

        let mut root = analyze_directory(d.path(), 1);
        assert_eq!(root.size, 1000);
        assert_eq!(root.children[0].name, "big");
        assert_eq!(root.children[0].size, 800);
        // depth 1: "deep" is summarized without children.
        let deep = root.children[0]
            .children
            .iter()
            .find(|c| c.name == "deep")
            .unwrap();
        assert_eq!(deep.size, 200);
        assert!(deep.children.is_empty());

        layout_treemap(&mut root, 0.0, 0.0, 1000.0, 500.0, 0, true);
        assert_eq!(root.children[0].rect, (0.0, 0.0, 800.0, 500.0));
        assert_eq!(root.children[1].rect, (800.0, 0.0, 1000.0, 500.0));
        assert_eq!(root.children[1].color, PALETTE[1]);

        let largest = find_largest_files(d.path(), 2);
        assert_eq!(
            largest.iter().map(|f| f.size).collect::<Vec<_>>(),
            [600, 200]
        );
    }
}
