//! Perceptual hashing (dHash / aHash) and visual near-duplicate clustering.
//!
//! Python v3.0 used Pillow: EXIF-orientation transpose, `convert("L")`
//! (ITU-R 601 luma) and `resize(..., BOX)`. Those steps are reproduced here so
//! hashes stay comparable between builds.

use crate::models::{FileInfo, VisualDuplicateGroup};
use image::{DynamicImage, ImageDecoder, ImageReader};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Extensions considered for visual matching.
pub const IMAGE_EXTENSIONS: [&str; 7] = [".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tiff", ".tif"];

/// Default similarity threshold (≈ Hamming distance ≤ 6).
pub const DEFAULT_THRESHOLD: f64 = 0.90;

/// Loads an image, applies EXIF orientation, converts to Pillow "L" luma.
fn load_luma(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let mut decoder = ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_decoder()
        .ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    if w == 0 || h == 0 {
        return None;
    }
    // Pillow: L = (R*19595 + G*38470 + B*7471 + 0x8000) >> 16
    let luma = rgb
        .pixels()
        .map(|p| {
            ((u32::from(p[0]) * 19595 + u32::from(p[1]) * 38470 + u32::from(p[2]) * 7471 + 0x8000)
                >> 16) as u8
        })
        .collect();
    Some((luma, w, h))
}

/// Pillow BOX-filter coefficients for one axis.
fn box_coeffs(in_size: usize, out_size: usize) -> Vec<(usize, Vec<f64>)> {
    let scale = in_size as f64 / out_size as f64;
    let filterscale = scale.max(1.0);
    let support = 0.5 * filterscale;
    (0..out_size)
        .map(|xx| {
            let center = (xx as f64 + 0.5) * scale;
            let xmin = ((center - support + 0.5) as isize).max(0) as usize;
            let xmax = ((center + support + 0.5) as isize)
                .min(in_size as isize)
                .max(xmin as isize) as usize;
            let mut k: Vec<f64> = (xmin..xmax)
                .map(|x| {
                    let v = (x as f64 - center + 0.5) / filterscale;
                    if v > -0.5 && v <= 0.5 { 1.0 } else { 0.0 }
                })
                .collect();
            let total: f64 = k.iter().sum();
            if total > 0.0 {
                k.iter_mut().for_each(|w| *w /= total);
            }
            (xmin, k)
        })
        .collect()
}

fn clip_round(v: f64) -> u8 {
    (v + 0.5).floor().clamp(0.0, 255.0) as u8
}

/// Two-pass (horizontal, then vertical) BOX resample like Pillow.
fn resize_box(src: &[u8], w: usize, h: usize, ow: usize, oh: usize) -> Vec<u8> {
    let hc = box_coeffs(w, ow);
    let mut tmp = vec![0u8; ow * h];
    for y in 0..h {
        for (x, (start, k)) in hc.iter().enumerate() {
            let s: f64 = k
                .iter()
                .enumerate()
                .map(|(i, wt)| wt * f64::from(src[y * w + start + i]))
                .sum();
            tmp[y * ow + x] = clip_round(s);
        }
    }
    let vc = box_coeffs(h, oh);
    let mut out = vec![0u8; ow * oh];
    for (y, (start, k)) in vc.iter().enumerate() {
        for x in 0..ow {
            let s: f64 = k
                .iter()
                .enumerate()
                .map(|(i, wt)| wt * f64::from(tmp[(start + i) * ow + x]))
                .sum();
            out[y * ow + x] = clip_round(s);
        }
    }
    out
}

/// 64-bit difference hash over a 9×8 grayscale thumbnail.
pub fn compute_dhash(path: &Path) -> Option<u64> {
    let (luma, w, h) = load_luma(path)?;
    let px = resize_box(&luma, w, h, 9, 8);
    let mut diff = 0u64;
    for row in 0..8 {
        for col in 0..8 {
            if px[row * 9 + col] > px[row * 9 + col + 1] {
                diff |= 1 << (row * 8 + col);
            }
        }
    }
    Some(diff)
}

/// 64-bit average hash over an 8×8 grayscale thumbnail.
pub fn compute_ahash(path: &Path) -> Option<u64> {
    let (luma, w, h) = load_luma(path)?;
    let px = resize_box(&luma, w, h, 8, 8);
    let avg = px.iter().map(|&p| f64::from(p)).sum::<f64>() / px.len() as f64;
    Some(
        px.iter()
            .enumerate()
            .filter(|(_, p)| f64::from(**p) >= avg)
            .fold(0u64, |acc, (i, _)| acc | (1 << i)),
    )
}

pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// 1.0 = identical, 0.0 = every bit differs.
pub fn similarity(a: u64, b: u64) -> f64 {
    (1.0 - f64::from(hamming_distance(a, b)) / 64.0).max(0.0)
}

/// Pixel count from the image header, if readable.
pub fn pixel_count(path: &Path) -> Option<u64> {
    image::image_dimensions(path)
        .ok()
        .map(|(w, h)| u64::from(w) * u64::from(h))
}

/// Clusters visually similar images.
#[derive(Debug, Clone, Copy)]
pub struct VisualSimilarityEngine {
    pub similarity_threshold: f64,
}

impl Default for VisualSimilarityEngine {
    fn default() -> Self {
        Self {
            similarity_threshold: DEFAULT_THRESHOLD,
        }
    }
}

impl VisualSimilarityEngine {
    pub fn new(similarity_threshold: f64) -> Self {
        Self {
            similarity_threshold,
        }
    }

    pub fn find_visual_duplicates(&self, files: &[FileInfo]) -> Vec<VisualDuplicateGroup> {
        let images: Vec<&FileInfo> = files
            .iter()
            .filter(|f| IMAGE_EXTENSIONS.contains(&f.extension.to_lowercase().as_str()))
            .collect();
        if images.len() < 2 {
            return Vec::new();
        }
        let hashes: HashMap<PathBuf, u64> = images
            .par_iter()
            .filter_map(|f| compute_dhash(&f.path).map(|h| (f.path.clone(), h)))
            .collect();

        let mut clusters: Vec<Vec<&FileInfo>> = Vec::new();
        let mut visited: HashSet<&Path> = HashSet::new();
        for (i, f1) in images.iter().enumerate() {
            let Some(&h1) = hashes.get(&f1.path) else {
                continue;
            };
            if !visited.insert(&f1.path) {
                continue;
            }
            let mut cluster = vec![*f1];
            for f2 in &images[i + 1..] {
                let Some(&h2) = hashes.get(&f2.path) else {
                    continue;
                };
                if visited.contains(f2.path.as_path()) {
                    continue;
                }
                if similarity(h1, h2) >= self.similarity_threshold {
                    cluster.push(f2);
                    visited.insert(&f2.path);
                }
            }
            if cluster.len() > 1 {
                clusters.push(cluster);
            }
        }

        clusters
            .into_iter()
            .map(|cluster| {
                // Largest (size, name) first, then highest resolution wins.
                let mut best = cluster[0];
                for f in &cluster[1..] {
                    if (f.size_bytes, &f.filename) > (best.size_bytes, &best.filename) {
                        best = f;
                    }
                }
                let mut max_pixels: Option<u64> = None;
                for f in &cluster {
                    if let Some(px) = pixel_count(&f.path)
                        && max_pixels.is_none_or(|m| px > m)
                    {
                        max_pixels = Some(px);
                        best = f;
                    }
                }
                let best_hash = hashes[&best.path];
                let sims: Vec<f64> = cluster
                    .iter()
                    .filter(|f| f.path != best.path)
                    .filter_map(|f| hashes.get(&f.path).map(|&h| similarity(best_hash, h)))
                    .collect();
                let avg = if sims.is_empty() {
                    1.0
                } else {
                    sims.iter().sum::<f64>() / sims.len() as f64
                };
                VisualDuplicateGroup {
                    phash: format!("{best_hash:016x}"),
                    files: cluster.iter().map(|f| (*f).clone()).collect(),
                    similarity_score: avg,
                    suggested_keep: Some(best.clone()),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    #[test]
    fn hamming_and_similarity() {
        let h1 = 0x1234_5678_9ABC_DEF0u64;
        assert_eq!(hamming_distance(h1, h1), 0);
        assert_eq!(similarity(h1, h1), 1.0);
        assert_eq!(hamming_distance(h1, h1 ^ 1), 1);
        assert_eq!(similarity(h1, h1 ^ 1), 63.0 / 64.0);
    }

    #[test]
    fn clusters_resized_copy_but_not_rotated_gradient() {
        let d = tempfile::tempdir().unwrap();
        let horiz = GrayImage::from_fn(100, 100, |x, _| Luma([(255.0 - f64::from(x) * 2.5) as u8]));
        let vert = GrayImage::from_fn(100, 100, |_, y| Luma([(255.0 - f64::from(y) * 2.5) as u8]));
        let p1 = d.path().join("original.jpg");
        let p2 = d.path().join("resized_copy.png");
        let p3 = d.path().join("different.jpg");
        horiz.save(&p1).unwrap();
        image::imageops::resize(&horiz, 60, 60, image::imageops::FilterType::Triangle)
            .save(&p2)
            .unwrap();
        vert.save(&p3).unwrap();

        let files: Vec<FileInfo> = [&p1, &p2, &p3]
            .iter()
            .map(|p| FileInfo::from_path(p).unwrap())
            .collect();
        let groups = VisualSimilarityEngine::default().find_visual_duplicates(&files);
        assert_eq!(groups.len(), 1);
        let names: HashSet<&str> = groups[0]
            .files
            .iter()
            .map(|f| f.filename.as_str())
            .collect();
        assert_eq!(names, HashSet::from(["original.jpg", "resized_copy.png"]));
        assert_eq!(
            groups[0].suggested_keep.as_ref().unwrap().filename,
            "original.jpg"
        );
        assert_eq!(groups[0].phash.len(), 16);
    }

    #[test]
    fn box_resize_averages() {
        let src: Vec<u8> = vec![0, 100, 200, 255];
        assert_eq!(resize_box(&src, 2, 2, 1, 1), vec![139]);
        assert_eq!(resize_box(&src, 2, 2, 2, 2), src);
    }

    #[test]
    fn unreadable_image_has_no_hash() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("broken.png");
        std::fs::write(&p, b"not an image").unwrap();
        assert!(compute_dhash(&p).is_none());
        assert!(compute_ahash(&p).is_none());
    }
}
