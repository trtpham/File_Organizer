//! Cancellation stops long operations safely; blast radius counts planned work.

use sfo_core::SfoError;
use sfo_core::config::Config;
use sfo_core::dedup::Deduplicator;
use sfo_core::executor::TransactionExecutor;
use sfo_core::guardrails;
use sfo_core::models::{ActionRecord, ActionType, OrganizeStrategy, RecordStatus};
use sfo_core::organizer::{Organizer, PlanOptions};
use sfo_core::progress::Control;
use sfo_core::scanner::{FileScanner, ScanOptions};
use std::fs;

fn workspace(n: usize) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for i in 0..n {
        fs::create_dir_all(d.path().join(format!("sub{}", i % 3))).unwrap();
        fs::write(
            d.path().join(format!("sub{}/f{i}.txt", i % 3)),
            b"same content",
        )
        .unwrap();
    }
    d
}

#[test]
fn scan_reports_progress_and_obeys_cancel() {
    let d = workspace(9);
    let cfg = Config::defaults();
    let ctl = Control::new();
    let opts = ScanOptions {
        recursive: true,
        control: Some(ctl.clone()),
        ..Default::default()
    };
    let files = FileScanner::new(&cfg).scan(d.path(), &opts, None).unwrap();
    assert_eq!(files.len(), 9);
    let v = ctl.view();
    assert_eq!((v.stage.as_str(), v.done), ("Scanning files", 9));

    ctl.cancel();
    assert!(matches!(
        FileScanner::new(&cfg).scan(d.path(), &opts, None),
        Err(SfoError::Cancelled)
    ));
}

#[test]
fn cancelled_session_moves_nothing_more_but_stays_recorded() {
    let d = workspace(6);
    let cfg = Config::defaults();
    let files = FileScanner::new(&cfg)
        .scan(
            d.path(),
            &ScanOptions {
                recursive: true,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    let records = Organizer::new(&cfg).plan(
        &files,
        d.path(),
        OrganizeStrategy::Extension,
        PlanOptions::default(),
    );

    // Cancel after the first file has moved.
    let ctl = Control::new();
    let c2 = ctl.clone();
    let mut cb = move |i: usize, _: usize, _: &ActionRecord| {
        if i == 1 {
            c2.cancel();
        }
    };
    let s = TransactionExecutor::new(&cfg).execute_session_ctl(
        records,
        d.path(),
        d.path(),
        "extension",
        false,
        Some(&mut cb),
        Some(&ctl),
    );
    assert_eq!(s.moved_count, 1);
    assert_eq!(
        s.records.len(),
        6,
        "every planned file stays in the journal"
    );
    let cancelled = s
        .records
        .iter()
        .filter(|r| r.message.starts_with("Cancelled"))
        .count();
    assert_eq!(cancelled, 5);
    assert!(
        s.records
            .iter()
            .filter(|r| r.message.starts_with("Cancelled"))
            .all(|r| r.status == RecordStatus::Skipped)
    );
    assert_eq!(fs::read_dir(d.path().join("Documents")).unwrap().count(), 1);
}

#[test]
fn dedupe_and_walk_cancel() {
    let d = workspace(4);
    let cfg = Config::defaults();
    let files = FileScanner::new(&cfg)
        .scan(
            d.path(),
            &ScanOptions {
                recursive: true,
                ..Default::default()
            },
            None,
        )
        .unwrap();
    let ctl = Control::new();
    let groups = Deduplicator::new(&cfg)
        .find_duplicates_ctl(&files, None, Some(&ctl))
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(ctl.view().stage, "Hashing full files");

    ctl.cancel();
    assert!(matches!(
        Deduplicator::new(&cfg).find_duplicates_ctl(&files, None, Some(&ctl)),
        Err(SfoError::Cancelled)
    ));
    assert!(matches!(
        sfo_core::fsutil::walk_files_ctl(d.path(), Some(&ctl)),
        Err(SfoError::Cancelled)
    ));
    assert!(matches!(
        sfo_core::treemap::analyze_directory_ctl(d.path(), 2, Some(&ctl)),
        Err(SfoError::Cancelled)
    ));
    assert!(matches!(
        sfo_core::secrets::scan_directory_ctl(d.path(), true, Some(&ctl)),
        Err(SfoError::Cancelled)
    ));
}

#[test]
fn blast_radius_counts_only_real_work() {
    let mk = |t, size| {
        let mut r = ActionRecord::new(t, "a".into(), Some("b".into()), size);
        r.status = RecordStatus::Pending;
        r
    };
    let recs = vec![
        mk(ActionType::Move, 10),
        mk(ActionType::Move, 10),
        mk(ActionType::Skip, 999),
    ];
    assert!(guardrails::check_records(&recs, 2, 100).is_none());
    assert!(
        guardrails::check_records(&recs, 1, 100)
            .unwrap()
            .contains("2 files")
    );
    assert!(
        guardrails::check_records(&recs, 5, 15)
            .unwrap()
            .contains("GB")
    );

    let mut cfg = Config::defaults();
    assert_eq!(cfg.blast_radius(), (500, 10 * 1024 * 1024 * 1024));
    cfg.set("blast_radius", serde_json::json!({"max_files": 3}));
    assert_eq!(cfg.blast_radius(), (3, 10 * 1024 * 1024 * 1024));
}
