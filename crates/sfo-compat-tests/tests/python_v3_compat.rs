//! Data written by the Python v3.0 build (see `scripts/gen_compat_fixtures.py`)
//! must load, verify and roll back in the Rust build.

use serde_json::Value;
use sfo_compat_tests::fixtures_dir;
use sfo_core::config::Config;
use sfo_core::guardrails::{self, AuditLogChain};
use sfo_core::models::{ActionType, RecordStatus};
use sfo_core::undo::UndoManager;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn read_json(rel: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(fixtures_dir().join(rel)).unwrap()).unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

#[test]
fn config_lookups_match_python() {
    let exp = read_json("config/expected.json");
    let cfg = Config::defaults();

    for (ext, want) in exp["category_for_extension"].as_object().unwrap() {
        assert_eq!(
            cfg.category_for_extension(ext),
            want.as_str().unwrap(),
            "ext {ext:?}"
        );
    }
    for (name, want) in exp["is_ignored"].as_object().unwrap() {
        let mut want = want.as_bool().unwrap();
        // Fixture came from Windows, where fnmatch is case-insensitive.
        if !cfg!(windows) && name == "Thumbs.DB" {
            want = false;
        }
        assert_eq!(cfg.is_ignored(name), want, "is_ignored({name:?})");
    }
    for (size, want) in exp["size_folder"].as_object().unwrap() {
        assert_eq!(
            cfg.size_folder(size.parse().unwrap()),
            want.as_str().unwrap(),
            "size {size}"
        );
    }
    assert_eq!(cfg.collision_strategy().as_str(), exp["collision_strategy"]);
    assert_eq!(cfg.date_format(), exp["date_format"]);
}

#[test]
fn python_audit_chain_verifies_and_extends() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("audit_chain.jsonl");
    fs::copy(fixtures_dir().join("audit/audit_chain.jsonl"), &log).unwrap();
    let chain = AuditLogChain::new(Some(&log)).unwrap();

    // Each Python hash must be reproduced byte-for-byte.
    for line in fs::read_to_string(&log).unwrap().lines() {
        let r: Value = serde_json::from_str(line).unwrap();
        let s = |k: &str| r[k].as_str().unwrap().to_string();
        let got = guardrails::chain_hash(
            &s("previous_hash"),
            &s("timestamp"),
            &s("operator"),
            &s("action"),
            &r["details"],
        );
        assert_eq!(got, s("current_hash"), "action {}", s("action"));
    }
    let v = chain.verify_integrity();
    assert!(v.valid && v.verified == 3, "{v:?}");

    let mut details = serde_json::Map::new();
    details.insert("from".into(), "rust".into());
    chain
        .log_event("RUST_APPEND", details, Some("Phạm"))
        .unwrap();
    let v = chain.verify_integrity();
    assert!(v.valid && v.verified == 4, "{v:?}");
}

#[test]
fn python_history_lists_and_parses() {
    let meta = read_json("undo/meta.json");
    let data = tempfile::tempdir().unwrap();
    copy_dir(&fixtures_dir().join("undo"), data.path());
    let undo = UndoManager::new(Some(data.path())).unwrap();

    let sessions = undo.list_sessions(20).unwrap();
    assert_eq!(sessions.len(), 3);
    let by_id: BTreeMap<_, _> = sessions
        .iter()
        .map(|s| (s.session_id.as_str(), s))
        .collect();
    let ids = &meta["sessions"];
    assert!(by_id[ids["dry_run"].as_str().unwrap()].dry_run);
    assert_eq!(by_id[ids["dry_run"].as_str().unwrap()].status, "DRY_RUN");
    assert_eq!(by_id[ids["organize"].as_str().unwrap()].moved_count, 5);

    let organize = undo
        .get_session(ids["organize"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(organize.records.len(), 5);
    assert!(
        organize
            .records
            .iter()
            .all(|r| r.action_type == ActionType::Move && r.status == RecordStatus::Success)
    );
    let rename = undo
        .get_session(ids["rename"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(rename.records[0].action_type, ActionType::Rename);

    // Latest undoable session is the rename (newest, not a dry run).
    assert_eq!(
        undo.get_latest_session().unwrap().unwrap().session_id,
        ids["rename"]
    );

    // Journal-only fallback: no DB rows, only the JSON files.
    let journals_only = tempfile::tempdir().unwrap();
    copy_dir(
        &fixtures_dir().join("undo/journals"),
        &journals_only.path().join("journals"),
    );
    let undo2 = UndoManager::new(Some(journals_only.path())).unwrap();
    assert!(undo2.list_sessions(20).unwrap().is_empty());
    assert_eq!(
        undo2
            .get_session(ids["organize"].as_str().unwrap())
            .unwrap()
            .unwrap(),
        organize
    );
}

#[test]
fn python_sessions_roll_back_in_rust() {
    let meta = read_json("undo/meta.json");
    let old_root = meta["root"].as_str().unwrap();
    let data = tempfile::tempdir().unwrap();
    let ws = tempfile::tempdir().unwrap();
    let new_root = sfo_core::paths::resolve(ws.path()).display().to_string();
    copy_dir(&fixtures_dir().join("undo"), data.path());

    // Re-home the recorded paths onto this machine's temp workspace.
    let conn = rusqlite::Connection::open(data.path().join("history.db")).unwrap();
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT session_id, data_json FROM sessions")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let esc = |s: &str| {
        serde_json::to_string(s)
            .unwrap()
            .trim_matches('"')
            .to_string()
    };
    for (id, json) in rows {
        let json = if cfg!(windows) {
            json.replace(&esc(old_root), &esc(&new_root))
        } else {
            json.replace(r"\\", "/")
                .replace(&old_root.replace('\\', "/"), &esc(&new_root))
        };
        conn.execute(
            "UPDATE sessions SET data_json = ?1 WHERE session_id = ?2",
            [&json, &id],
        )
        .unwrap();
    }
    drop(conn);

    for (rel, content) in meta["state_after_sessions"].as_object().unwrap() {
        let p = sfo_core::paths::join_rel(ws.path(), rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content.as_str().unwrap()).unwrap();
    }

    let undo = UndoManager::new(Some(data.path())).unwrap();
    let first = undo.rollback_session(None).unwrap();
    assert!(first.success, "{first:?}");
    assert_eq!(
        first.session_id.as_deref(),
        meta["sessions"]["rename"].as_str()
    );
    assert_eq!(first.restored_count, 1);
    assert!(ws.path().join("Code/script.py").exists());

    let second = undo.rollback_session(None).unwrap();
    assert!(second.success && second.failed_count == 0, "{second:?}");
    assert_eq!(
        second.session_id.as_deref(),
        meta["sessions"]["organize"].as_str()
    );

    for (name, content) in meta["original_files"].as_object().unwrap() {
        assert_eq!(
            fs::read_to_string(ws.path().join(name)).unwrap(),
            content.as_str().unwrap(),
            "{name}"
        );
    }
    for dir in ["Images", "Documents", "Code"] {
        assert!(!ws.path().join(dir).exists(), "{dir} should be pruned");
    }
    let statuses: Vec<String> = undo
        .list_sessions(10)
        .unwrap()
        .into_iter()
        .map(|s| s.status)
        .collect();
    assert_eq!(statuses.iter().filter(|s| *s == "ROLLED_BACK").count(), 2);
}
