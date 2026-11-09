//! Progress reporting and cooperative cancellation for long operations.
//!
//! A [`Control`] is shared (via `Arc`) between the thread doing the work and
//! the UI. Workers call [`Control::stage`] / [`Control::tick`] and check
//! [`Control::is_cancelled`] between items; the UI reads [`Control::snapshot`]
//! and calls [`Control::cancel`].

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Control {
    cancelled: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    stage: Mutex<String>,
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (stage, done, total) = self.snapshot();
        f.debug_struct("Control")
            .field("stage", &stage)
            .field("done", &done)
            .field("total", &total)
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Point-in-time view for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub stage: String,
    pub done: u64,
    /// 0 when the total is not known yet (show an indeterminate bar).
    pub total: u64,
}

impl Control {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Asks the worker to stop at the next checkpoint.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Starts a new phase: resets the counter and sets its total (0 = unknown).
    pub fn stage(&self, name: &str, total: u64) {
        if let Ok(mut s) = self.stage.lock() {
            *s = name.to_string();
        }
        self.done.store(0, Ordering::SeqCst);
        self.total.store(total, Ordering::SeqCst);
    }

    /// Counts `n` more finished items.
    pub fn tick(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (String, u64, u64) {
        let stage = self.stage.lock().map(|s| s.clone()).unwrap_or_default();
        (
            stage,
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    pub fn view(&self) -> Snapshot {
        let (stage, done, total) = self.snapshot();
        Snapshot { stage, done, total }
    }
}

/// `true` when an optional control has been cancelled.
pub fn cancelled(ctl: Option<&Control>) -> bool {
    ctl.is_some_and(Control::is_cancelled)
}

/// Ticks an optional control.
pub fn tick(ctl: Option<&Control>, n: u64) {
    if let Some(c) = ctl {
        c.tick(n);
    }
}

/// Sets the stage of an optional control.
pub fn stage(ctl: Option<&Control>, name: &str, total: u64) {
    if let Some(c) = ctl {
        c.stage(name, total);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_ticks_and_cancel() {
        let c = Control::new();
        c.stage("Hashing", 10);
        c.tick(3);
        assert_eq!(
            c.view(),
            Snapshot {
                stage: "Hashing".into(),
                done: 3,
                total: 10
            }
        );
        c.stage("Moving", 0);
        assert_eq!(c.view().done, 0);
        assert!(!cancelled(Some(&c)));
        c.cancel();
        assert!(cancelled(Some(&c)) && !cancelled(None));
    }
}
