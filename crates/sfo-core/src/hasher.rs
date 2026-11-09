//! Tiered hasher: partial (first 128 KB) then full SHA-256, with a
//! process-wide cache keyed by `(path, mtime, size)`.

use crate::models::FileInfo;
use crate::paths;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

/// Bytes read for the partial hash.
pub const DEFAULT_PARTIAL_BYTES: u64 = 131_072;
/// Streaming buffer for full hashes.
pub const BUFFER_SIZE: usize = 2_097_152;
/// Sentinel partial hash for zero-byte files (Python v3.0 value).
pub const EMPTY_FILE: &str = "empty_file";

/// Supported full-hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HashAlgorithm {
    #[default]
    Sha256,
    Md5,
}

impl HashAlgorithm {
    /// Python accepts `"md5"`; anything else means SHA-256.
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("md5") {
            Self::Md5
        } else {
            Self::Sha256
        }
    }
}

type CacheKey = (PathBuf, SystemTime, u64);

#[derive(Default, Clone)]
struct CacheEntry {
    partial: Option<String>,
    full: HashMap<HashAlgorithm, String>,
}

fn cache() -> &'static Mutex<HashMap<CacheKey, CacheEntry>> {
    static C: OnceLock<Mutex<HashMap<CacheKey, CacheEntry>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

fn cache_key(path: &Path) -> Option<CacheKey> {
    let meta = std::fs::metadata(path).ok()?;
    Some((path.to_path_buf(), meta.modified().ok()?, meta.len()))
}

/// SHA-256 of the first `chunk_bytes` bytes. `None` if unreadable.
pub fn partial_hash(path: &Path, chunk_bytes: u64) -> Option<String> {
    let p = paths::resolve(path);
    let key = cache_key(&p)?;
    if let Some(hit) = cache()
        .lock()
        .ok()?
        .get(&key)
        .and_then(|e| e.partial.clone())
    {
        return Some(hit);
    }
    let mut data = Vec::new();
    File::open(&p)
        .ok()?
        .take(chunk_bytes)
        .read_to_end(&mut data)
        .ok()?;
    if data.is_empty() {
        return Some(EMPTY_FILE.into());
    }
    let digest = hex::encode(Sha256::digest(&data));
    if let Ok(mut c) = cache().lock() {
        c.entry(key).or_default().partial = Some(digest.clone());
    }
    Some(digest)
}

/// Full-content digest. `progress` receives cumulative bytes read.
pub fn full_hash(
    path: &Path,
    algorithm: HashAlgorithm,
    mut progress: Option<&mut dyn FnMut(u64)>,
) -> Option<String> {
    let p = paths::resolve(path);
    let key = cache_key(&p)?;
    if let Some(hit) = cache()
        .lock()
        .ok()?
        .get(&key)
        .and_then(|e| e.full.get(&algorithm).cloned())
    {
        return Some(hit);
    }
    let mut file = File::open(&p).ok()?;
    let mut buf = vec![0u8; BUFFER_SIZE];
    let mut sha = Sha256::new();
    let mut md5 = md5::Md5::new();
    let mut read_total = 0u64;
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        match algorithm {
            HashAlgorithm::Sha256 => sha.update(&buf[..n]),
            HashAlgorithm::Md5 => md5.update(&buf[..n]),
        }
        read_total += n as u64;
        if let Some(cb) = progress.as_mut() {
            cb(read_total);
        }
    }
    let digest = match algorithm {
        HashAlgorithm::Sha256 => hex::encode(sha.finalize()),
        HashAlgorithm::Md5 => hex::encode(md5.finalize()),
    };
    if let Ok(mut c) = cache().lock() {
        c.entry(key)
            .or_default()
            .full
            .insert(algorithm, digest.clone());
    }
    Some(digest)
}

/// SHA-256 full hash, no progress.
pub fn sha256_file(path: &Path) -> Option<String> {
    full_hash(path, HashAlgorithm::Sha256, None)
}

/// Fills `partial_hash` and, if `full`, `full_hash` on `info`.
pub fn populate_hashes(info: &mut FileInfo, full: bool) {
    if info.partial_hash.is_none() {
        info.partial_hash = partial_hash(&info.path, DEFAULT_PARTIAL_BYTES);
    }
    if full && info.full_hash.is_none() {
        info.full_hash = sha256_file(&info.path);
    }
}

/// Parallel [`populate_hashes`]. `max_workers` defaults to
/// `min(32, cpus * 2)`. `progress` gets `(completed, total)` from worker threads.
pub fn populate_hashes_parallel(
    files: &mut [FileInfo],
    full: bool,
    max_workers: Option<usize>,
    progress: Option<&(dyn Fn(usize, usize) + Sync)>,
) {
    populate_hashes_ctl(files, full, max_workers, progress, None);
}

/// [`populate_hashes_parallel`] that ticks `ctl` per file and stops picking up
/// new files once it is cancelled (files already being hashed finish).
pub fn populate_hashes_ctl(
    files: &mut [FileInfo],
    full: bool,
    max_workers: Option<usize>,
    progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    ctl: Option<&crate::progress::Control>,
) {
    if files.is_empty() {
        return;
    }
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let workers = max_workers.unwrap_or_else(|| (cpus * 2).min(32)).max(1);
    let total = files.len();
    let done = AtomicUsize::new(0);
    let work = |files: &mut [FileInfo]| {
        files.par_iter_mut().for_each(|f| {
            if crate::progress::cancelled(ctl) {
                return;
            }
            populate_hashes(f, full);
            crate::progress::tick(ctl, 1);
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(cb) = progress {
                cb(n, total);
            }
        });
    };
    match rayon::ThreadPoolBuilder::new().num_threads(workers).build() {
        Ok(pool) => pool.install(|| work(files)),
        Err(_) => work(files),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_and_full_hash() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("test.dat");
        std::fs::write(&p, vec![b'A'; 100_000]).unwrap();
        let mut info = FileInfo::from_path(&p).unwrap();
        populate_hashes(&mut info, true);
        assert!(info.partial_hash.is_some());
        assert_eq!(info.full_hash.as_deref().map(str::len), Some(64));
        // File smaller than the partial window: partial == full.
        assert_eq!(info.partial_hash, info.full_hash);
    }

    #[test]
    fn identical_files_identical_hashes_and_known_vectors() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("f1.bin"), d.path().join("f2.bin"));
        std::fs::write(&a, b"RANDOM_PAYLOAD_TEST_9999").unwrap();
        std::fs::write(&b, b"RANDOM_PAYLOAD_TEST_9999").unwrap();
        assert_eq!(sha256_file(&a), sha256_file(&b));

        let abc = d.path().join("abc");
        std::fs::write(&abc, b"abc").unwrap();
        assert_eq!(
            sha256_file(&abc).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            full_hash(&abc, HashAlgorithm::Md5, None).unwrap(),
            "900150983cd24fb0d6963f7d28e17f72"
        );
    }

    #[test]
    fn empty_file_sentinel_and_missing() {
        let d = tempfile::tempdir().unwrap();
        let e = d.path().join("empty");
        std::fs::write(&e, b"").unwrap();
        assert_eq!(
            partial_hash(&e, DEFAULT_PARTIAL_BYTES).as_deref(),
            Some(EMPTY_FILE)
        );
        assert!(sha256_file(&d.path().join("missing")).is_none());
    }

    #[test]
    fn parallel_batch() {
        let d = tempfile::tempdir().unwrap();
        let mut files: Vec<FileInfo> = (0..5)
            .map(|i| {
                let p = d.path().join(format!("batch_{i}.dat"));
                std::fs::write(&p, format!("BATCH_CONTENT_{i}").repeat(1000)).unwrap();
                FileInfo::from_path(&p).unwrap()
            })
            .collect();
        let calls = AtomicUsize::new(0);
        let cb = |_: usize, _: usize| {
            calls.fetch_add(1, Ordering::Relaxed);
        };
        populate_hashes_parallel(&mut files, true, Some(2), Some(&cb));
        assert_eq!(calls.load(Ordering::Relaxed), 5);
        assert!(
            files
                .iter()
                .all(|f| f.full_hash.as_deref().map(str::len) == Some(64))
        );
    }
}
