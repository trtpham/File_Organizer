//! Polling folder watcher: waits until downloads stop growing, then
//! organizes each file in its own undoable session.

use crate::config::Config;
use crate::error::Result;
use crate::executor::TransactionExecutor;
use crate::models::{ActionType, FileInfo, OrganizeStrategy, SyncProfile};
use crate::organizer::{Organizer, PlanOptions};
use crate::paths;
use crate::scanner::{FileScanner, ScanOptions};
use crate::syncer;
use crate::undo::UndoManager;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Suffixes of in-progress downloads / temp files.
pub const TEMP_EXTENSIONS: [&str; 8] = [
    ".crdownload",
    ".tmp",
    ".part",
    ".downloading",
    ".aria2",
    ".sfo_tmp",
    ".download",
    ".opdownload",
];

/// Called with `(source_name, destination_name)` after each organized file.
pub type OnOrganized = Box<dyn FnMut(&str, &str) + Send>;

/// Watcher state; [`DirectoryWatcher::tick`] runs one polling pass.
pub struct DirectoryWatcher {
    pub source_dir: PathBuf,
    pub destination_dir: PathBuf,
    pub strategy: OrganizeStrategy,
    pub smart_receipts: bool,
    pub sync_profiles: Vec<SyncProfile>,
    pub debounce: Duration,
    pub check_interval: Duration,
    config: Config,
    undo: UndoManager,
    states: HashMap<PathBuf, (u64, Instant)>,
    last_drives: HashSet<String>,
}

impl DirectoryWatcher {
    pub fn new(
        source: &Path,
        destination: Option<&Path>,
        strategy: OrganizeStrategy,
        config: Config,
        undo: UndoManager,
    ) -> Self {
        let (debounce, interval) = config.watcher_settings();
        Self {
            source_dir: paths::resolve(source),
            destination_dir: paths::resolve(destination.unwrap_or(source)),
            strategy,
            smart_receipts: false,
            sync_profiles: Vec::new(),
            debounce: Duration::from_secs_f64(debounce.max(0.0)),
            check_interval: Duration::from_secs_f64(interval.max(0.05)),
            config,
            undo,
            states: HashMap::new(),
            last_drives: HashSet::new(),
        }
    }

    fn is_temp(f: &FileInfo) -> bool {
        TEMP_EXTENSIONS.contains(&f.extension.as_str())
            || f.filename.starts_with('~')
            || f.filename.starts_with('.')
    }

    /// One polling pass. Returns `true` while files are still stabilizing.
    pub fn tick(&mut self, on_organized: &mut Option<OnOrganized>) -> Result<bool> {
        let exclude = if paths::path_eq(&self.destination_dir, &self.source_dir) {
            vec![]
        } else {
            vec![self.destination_dir.clone()]
        };
        let opts = ScanOptions {
            recursive: false,
            exclude_dirs: exclude,
            max_depth: None,
            control: None,
        };
        let candidates = FileScanner::new(&self.config).scan(&self.source_dir, &opts, None)?;
        let now = Instant::now();
        let mut seen = HashSet::new();
        let mut active = false;

        for f in candidates {
            seen.insert(f.path.clone());
            if Self::is_temp(&f) {
                continue;
            }
            match self.states.get(&f.path).copied() {
                Some((size, since)) if size == f.size_bytes => {
                    if now.duration_since(since) >= self.debounce {
                        self.states.remove(&f.path);
                        self.organize(&f, on_organized)?;
                    } else {
                        active = true;
                    }
                }
                _ => {
                    self.states.insert(f.path.clone(), (f.size_bytes, now));
                    active = true;
                }
            }
        }
        self.states.retain(|p, _| seen.contains(p));

        if !self.sync_profiles.is_empty() {
            let drives: HashSet<String> = syncer::removable_drives()
                .into_iter()
                .map(|d| d.path)
                .collect();
            if drives.difference(&self.last_drives).next().is_some() {
                for p in self
                    .sync_profiles
                    .iter()
                    .filter(|p| p.enabled && p.trigger_on_usb)
                {
                    syncer::sync(p, false);
                }
            }
            self.last_drives = drives;
        }
        Ok(active)
    }

    fn organize(&self, f: &FileInfo, on_organized: &mut Option<OnOrganized>) -> Result<()> {
        let opts = PlanOptions {
            smart_receipts: self.smart_receipts,
            ..Default::default()
        };
        let records = Organizer::new(&self.config).plan(
            std::slice::from_ref(f),
            &self.destination_dir,
            self.strategy,
            opts,
        );
        let Some(first) = records.first().cloned() else {
            return Ok(());
        };
        if first.action_type == ActionType::Skip {
            return Ok(());
        }
        let session = TransactionExecutor::new(&self.config).execute_session(
            records,
            &self.source_dir,
            &self.destination_dir,
            self.strategy.as_str(),
            false,
            None,
        );
        self.undo.save_session(&session)?;
        if let (Some(cb), Some(dst)) = (on_organized.as_mut(), first.destination_path.as_deref()) {
            cb(&f.filename, &crate::fsutil::name_of(dst));
        }
        Ok(())
    }

    /// Runs on a background thread until the handle is stopped.
    pub fn spawn(mut self, mut on_organized: Option<OnOrganized>) -> WatcherHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                let sleep = match self.tick(&mut on_organized) {
                    Ok(true) => Duration::from_secs(1).min(self.check_interval),
                    _ => self.check_interval,
                };
                // Sleep in small slices so stop() returns promptly.
                let until = Instant::now() + sleep;
                while Instant::now() < until && !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        });
        WatcherHandle {
            stop,
            thread: Some(thread),
        }
    }
}

/// Handle to a background watcher.
pub struct WatcherHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WatcherHandle {
    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Relaxed) && self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn watcher(src: &Path, data: &Path) -> DirectoryWatcher {
        let mut w = DirectoryWatcher::new(
            src,
            None,
            OrganizeStrategy::Extension,
            Config::defaults(),
            UndoManager::new(Some(data)).unwrap(),
        );
        w.debounce = Duration::from_millis(0);
        w.check_interval = Duration::from_millis(50);
        w
    }

    #[test]
    fn organizes_after_stable_and_ignores_partials() {
        let d = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("song.mp3"), "abc").unwrap();
        std::fs::write(d.path().join("movie.mp4.crdownload"), "partial").unwrap();
        let mut w = watcher(d.path(), data.path());
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = Arc::clone(&log);
        let mut cb: Option<OnOrganized> = Some(Box::new(move |a: &str, b: &str| {
            l2.lock().unwrap().push(format!("{a}->{b}"))
        }));

        assert!(w.tick(&mut cb).unwrap(), "first sighting is active");
        assert!(d.path().join("song.mp3").exists());
        assert!(!w.tick(&mut cb).unwrap());
        assert!(d.path().join("Audio/song.mp3").exists());
        assert!(d.path().join("movie.mp4.crdownload").exists());
        assert_eq!(*log.lock().unwrap(), ["song.mp3->song.mp3"]);
        assert_eq!(
            UndoManager::new(Some(data.path()))
                .unwrap()
                .list_sessions(5)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn background_thread_stops() {
        let d = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("doc.pdf"), "pdf").unwrap();
        let mut h = watcher(d.path(), data.path()).spawn(None);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !d.path().join("Documents/doc.pdf").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(h.is_running());
        h.stop();
        assert!(!h.is_running());
        assert!(d.path().join("Documents/doc.pdf").exists());
    }
}
