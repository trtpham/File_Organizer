//! Headless click-through of every tab (egui_kittest): buttons are clicked
//! by their visible labels, confirmations are answered, real jobs run on
//! temporary folders, and the files on disk are checked afterwards.

use crate::app::App;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

type H<'a> = Harness<'a, Option<App>>;

fn app<'h>(h: &'h mut H<'_>) -> &'h mut App {
    h.state_mut()
        .as_mut()
        .expect("app initialised on the first frame")
}

/// Steps frames until the background job finished and the UI settled.
fn settle(h: &mut H<'_>) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        h.step();
        if app(h).idle() || Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    for _ in 0..3 {
        h.step();
    }
    assert!(app(h).idle(), "job did not finish within 60 s");
}

fn click(h: &mut H<'_>, label: &str) {
    h.get_by_label(label).click();
    h.step();
    h.step();
}

/// Answers "Yes" on the confirmation dialog, then waits for the job.
fn confirm(h: &mut H<'_>) {
    click(h, "Yes");
    settle(h);
}

/// Dismisses every info/result dialog, returning their bodies.
fn dismiss_all(h: &mut H<'_>) -> Vec<String> {
    let mut bodies = Vec::new();
    while let Some(d) = app(h).dialogs.first().cloned() {
        assert!(
            d.confirm.is_none(),
            "unexpected confirmation: {} / {}",
            d.title,
            d.body
        );
        bodies.push(format!("{}: {}", d.title, d.body));
        click(h, "OK");
    }
    bodies
}

fn write(p: &Path, data: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, data).unwrap();
}

#[test]
fn click_through_all_tabs() {
    let data = tempfile::tempdir().unwrap();
    // Single test in this binary: no other thread reads the environment yet.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(sfo_core::DATA_DIR_ENV, data.path());
    }
    let ws = tempfile::tempdir().unwrap();
    let ws_path = ws.path().to_path_buf();
    let ws_s = ws_path.display().to_string();

    let mut h: H<'_> = Harness::builder()
        .with_size(egui::vec2(1120.0, 760.0))
        .build_state(
            |ctx, st: &mut Option<App>| match st {
                // Like eframe: build the app (fonts, theme) before the first
                // frame that draws, because fonts load on the next frame.
                None => {
                    *st = Some(App::new(ctx, None));
                    ctx.request_repaint();
                }
                Some(a) => a.frame(ctx),
            },
            None,
        );
    h.step();
    h.step();
    dismiss_all(&mut h);

    // ---------------------------------------------------------- Organize
    write(&ws_path.join("org/photo.jpg"), b"JPEG");
    write(&ws_path.join("org/report.pdf"), b"PDF");
    write(&ws_path.join("org/notes.txt"), b"plain notes");
    let org = ws_path.join("org").display().to_string();
    {
        let a = app(&mut h);
        a.org.source = org.clone();
        a.org.dest = String::new(); // empty destination = same as source
        a.org.smart_receipts = false;
    }
    click(&mut h, "🔍 Preview Actions");
    settle(&mut h);
    assert_eq!(app(&mut h).org.planned_moves, 3);
    assert_eq!(
        app(&mut h).org.dest,
        org,
        "empty destination falls back to the source"
    );
    app(&mut h).org.dry_run = false;
    click(&mut h, "⚡ Execute Now");
    assert_eq!(app(&mut h).dialogs[0].title, "Confirm Execution");
    confirm(&mut h);
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].contains("Files Organized: 3"), "{msgs:?}");
    assert!(ws_path.join("org/Images/photo.jpg").exists());
    assert!(ws_path.join("org/Documents/report.pdf").exists());

    // ------------------------------------------------------ Undo & History
    click(&mut h, "⏪ Undo & History");
    assert_eq!(app(&mut h).tab, 7, "clicking a tab switches to it");
    assert_eq!(app(&mut h).sys.sessions.len(), 1);
    app(&mut h).sys.history_sel = Some(0);
    click(&mut h, "⏪ Undo Selected Session");
    confirm(&mut h);
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].contains("Restored 3 files"), "{msgs:?}");
    assert!(ws_path.join("org/photo.jpg").exists());
    assert!(!ws_path.join("org/Images").exists());
    assert_eq!(app(&mut h).sys.sessions[0].status, "ROLLED_BACK");

    // ---------------------------------------------------- Duplicate Center
    let dup = b"DUPLICATE CONTENT ".repeat(100);
    write(&ws_path.join("dup/a.bin"), &dup);
    write(&ws_path.join("dup/b.bin"), &dup);
    write(&ws_path.join("dup/c.bin"), b"unique");
    {
        let a = app(&mut h);
        a.tab = 1;
        a.dup.folder = ws_path.join("dup").display().to_string();
    }
    h.step();
    click(&mut h, "🔍 Scan Duplicates");
    settle(&mut h);
    assert_eq!(app(&mut h).dup.stats.0, 1);
    click(&mut h, "📦 Move Duplicates");
    confirm(&mut h);
    settle(&mut h); // the move triggers a rescan
    dismiss_all(&mut h);
    assert_eq!(
        fs::read_dir(ws_path.join("dup/_Duplicates"))
            .unwrap()
            .count(),
        1
    );

    // ------------------------------------------------------- Smart Renamer
    write(&ws_path.join("ren/old_name.txt"), b"x");
    {
        let a = app(&mut h);
        a.tab = 2;
        a.ren.folder = ws_path.join("ren").display().to_string();
        a.ren.template = "{counter}_{name}".into();
    }
    h.step();
    click(&mut h, "🔍 Preview Rename");
    settle(&mut h);
    click(&mut h, "🏷️ Rename Files");
    confirm(&mut h);
    settle(&mut h);
    dismiss_all(&mut h);
    assert!(ws_path.join("ren/001_old_name.txt").exists());

    // ----------------------------------------------------- Storage Treemap
    {
        let a = app(&mut h);
        a.tab = 3;
        a.store.folder = ws_s.clone();
    }
    h.step();
    click(&mut h, "📊 Analyze Storage");
    settle(&mut h);
    assert!(app(&mut h).store.root.is_some());
    assert!(!app(&mut h).store.largest_rows.is_empty());

    // ------------------------------------------------- Sync: USB / folder
    write(&ws_path.join("src/save.dat"), b"save game");
    {
        let a = app(&mut h);
        a.tab = 4;
        a.sync.sub = 0;
        a.sync.usb_source = ws_path.join("src").display().to_string();
        a.sync.usb_dest = ws_path.join("backup").display().to_string();
    }
    h.step();
    click(&mut h, "💾 Sync to USB");
    settle(&mut h);
    dismiss_all(&mut h);
    assert!(ws_path.join("backup/save.dat").exists());
    assert!(app(&mut h).sync.usb_log.contains("Synced 1 files"));

    // ------------------------------------------------------------ LAN Drop
    {
        let a = app(&mut h);
        a.sync.sub = 2;
        a.sync.lan_folder = ws_path.join("src").display().to_string();
        a.sync.lan_port = "0".into();
    }
    h.step();
    click(&mut h, &sfo_core::i18n::t("start_lan"));
    dismiss_all(&mut h);
    assert!(
        app(&mut h)
            .sync
            .lan
            .as_ref()
            .is_some_and(|s| s.is_running())
    );
    click(&mut h, &sfo_core::i18n::t("stop_lan"));
    dismiss_all(&mut h);
    assert!(app(&mut h).sync.lan.is_none());

    // -------------------------------------------------------- Cold storage
    write(&ws_path.join("cold/old.log"), b"old data");
    {
        let a = app(&mut h);
        a.sync.sub = 3;
        a.sync.cold_folder = ws_path.join("cold").display().to_string();
        a.sync.cold_days = "0".into();
    }
    h.step();
    click(&mut h, "🔍 Find Cold Files");
    settle(&mut h);
    dismiss_all(&mut h);
    assert_eq!(app(&mut h).sync.cold_files.len(), 1);
    click(&mut h, "🗄️ Compress Cold Files");
    confirm(&mut h);
    settle(&mut h);
    let msgs = dismiss_all(&mut h);
    assert!(
        msgs.iter().any(|m| m.starts_with("Archiving Complete")),
        "{msgs:?}"
    );
    assert!(
        fs::read_dir(ws_path.join("cold"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with(".zip"))
    );

    // ---------------------------------------------------------- Dev junk
    write(&ws_path.join("proj/__pycache__/x.pyc"), b"pyc");
    {
        let a = app(&mut h);
        a.tab = 5;
        a.sec.sub = 0;
        a.sec.junk_folder = ws_path.join("proj").display().to_string();
    }
    h.step();
    click(&mut h, "🔍 Scan Junk");
    settle(&mut h);
    assert_eq!(app(&mut h).sec.junk.len(), 1);
    click(&mut h, "🗑️ Clean Discovered Junk");
    confirm(&mut h);
    settle(&mut h);
    dismiss_all(&mut h);
    assert!(!ws_path.join("proj/__pycache__").exists());

    // ------------------------------------------------------ Secret scanner
    write(&ws_path.join("leak/.env"), b"AWS=AKIAABCDEFGHIJKLMNOP\n");
    {
        let a = app(&mut h);
        a.sec.sub = 1;
        a.sec.secret_folder = ws_path.join("leak").display().to_string();
    }
    h.step();
    click(&mut h, "🔍 Scan Secrets");
    settle(&mut h);
    dismiss_all(&mut h);
    assert_eq!(app(&mut h).sec.secrets.len(), 1);
    click(&mut h, "🛡️ Quarantine Compromised Files");
    confirm(&mut h);
    settle(&mut h);
    dismiss_all(&mut h);
    assert!(ws_path.join("leak/_Quarantine/.env.quarantined").exists());

    // ------------------------------------------------------------ Shredder
    let victim = ws_path.join("shred/secret.doc");
    write(&victim, b"wipe me");
    {
        let a = app(&mut h);
        a.sec.sub = 2;
        a.sec.shred_path = victim.display().to_string();
        a.sec.shred_method = 1; // quick
    }
    h.step();
    click(&mut h, "🔥 Permanently Shred Target");
    assert!(app(&mut h).dialogs[0].danger);
    confirm(&mut h);
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].starts_with("Erasure Complete"), "{msgs:?}");
    assert!(!victim.exists());

    // --------------------------------------------------------------- Vault
    let doc = ws_path.join("vault/plan.txt");
    write(&doc, b"top secret");
    let vault = ws_path.join("vault/box.sfovault");
    {
        let a = app(&mut h);
        a.sec.sub = 3;
        a.sec.vault_src = doc.display().to_string();
        a.sec.vault_pw = "pw-123".into();
        a.test_pick = Some(vault.clone());
    }
    h.step();
    click(&mut h, "🔒 Lock into Vault");
    settle(&mut h);
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].starts_with("Vault Locked"), "{msgs:?}");
    assert!(vault.exists());
    {
        let a = app(&mut h);
        a.sec.vault_file = vault.display().to_string();
        a.sec.vault_unlock_pw = "wrong".into();
    }
    click(&mut h, "📋 Inspect");
    settle(&mut h);
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].starts_with("Authentication Failed"), "{msgs:?}");
    let out = ws_path.join("vault_out");
    {
        let a = app(&mut h);
        a.sec.vault_unlock_pw = "pw-123".into();
        a.test_pick = Some(out.clone());
    }
    click(&mut h, "🔓 Unlock Vault");
    settle(&mut h);
    dismiss_all(&mut h);
    assert_eq!(fs::read(out.join("plan.txt")).unwrap(), b"top secret");

    // -------------------------------------------------------------- Doctor
    app(&mut h).tab = 6;
    h.step();
    click(&mut h, &sfo_core::i18n::t("run_diagnostics"));
    settle(&mut h);
    assert!(app(&mut h).sys.doctor_status.contains("PASS"));
    assert!(!app(&mut h).sys.doctor_rows.is_empty());

    // ------------------------------------------------------------ Settings
    app(&mut h).tab = 8;
    h.step();
    click(&mut h, "🔒 Verify SHA-256 Audit Log");
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].starts_with("Audit Integrity Validated"), "{msgs:?}");
    assert!(!app(&mut h).sys.category_rows.is_empty());

    // ---------------------------------------------- Guardrail on drive root
    {
        let a = app(&mut h);
        a.tab = 0;
        a.org.source = if cfg!(windows) {
            "C:\\".into()
        } else {
            "/".into()
        };
    }
    h.step();
    click(&mut h, "🔍 Preview Actions");
    let msgs = dismiss_all(&mut h);
    assert!(
        msgs[0].starts_with("Enterprise Safety Violation"),
        "{msgs:?}"
    );

    // ------------------------------------------ Organize: include subfolders
    write(&ws_path.join("deep/top.txt"), b"t");
    write(&ws_path.join("deep/nested/pic.png"), b"p");
    {
        let a = app(&mut h);
        a.tab = 0;
        a.org.source = ws_path.join("deep").display().to_string();
        a.org.dest = String::new();
        a.org.dry_run = false;
        a.org.recursive = false;
    }
    h.step();
    click(&mut h, "🔍 Preview Actions");
    settle(&mut h);
    assert_eq!(app(&mut h).org.files, 1, "subfolders skipped by default");
    click(&mut h, "Include subfolders");
    assert!(app(&mut h).org.recursive);
    click(&mut h, "🔍 Preview Actions");
    settle(&mut h);
    assert_eq!(app(&mut h).org.files, 2, "nested file included");

    // -------------------------------------------- Blast radius confirmation
    app(&mut h)
        .config
        .set("blast_radius", serde_json::json!({"max_files": 1}));
    click(&mut h, "⚡ Execute Now");
    let d = app(&mut h).dialogs[0].clone();
    assert!(d.danger, "over the limit the confirmation turns red");
    assert!(
        d.body.starts_with("⚠ Operation impacts 2 files"),
        "{}",
        d.body
    );
    click(&mut h, "No");
    assert!(
        ws_path.join("deep/nested/pic.png").exists(),
        "declining changes nothing"
    );
    app(&mut h)
        .config
        .set("blast_radius", serde_json::json!({"max_files": 500}));

    // ------------------------------------------------ Progress and Cancel
    for i in 0..3000 {
        write(&ws_path.join(format!("big/{}/f{i}.bin", i % 20)), b"x");
    }
    {
        let a = app(&mut h);
        a.tab = 1;
        a.dup.folder = ws_path.join("big").display().to_string();
        a.dup.stats = (7, 7, 7);
    }
    h.step();
    click(&mut h, "🔍 Scan Duplicates");
    assert!(app(&mut h).job.as_ref().is_some_and(|j| j.cancellable));
    click(&mut h, "✖ Cancel");
    settle(&mut h);
    assert_eq!(app(&mut h).status, "Cancelled. Nothing was changed.");
    assert_eq!(
        app(&mut h).dup.stats,
        (7, 7, 7),
        "a cancelled scan leaves old results alone"
    );

    // ----------------------------------------------------------- Watcher
    write(&ws_path.join("inbox/song.mp3"), b"mp3");
    {
        let a = app(&mut h);
        a.tab = 0;
        a.org.source = ws_path.join("inbox").display().to_string();
        a.org.dest = String::new();
        a.config.set(
            "watcher",
            serde_json::json!({"debounce_seconds": 0.0, "check_interval_seconds": 0.1}),
        );
    }
    h.step();
    let sessions_before = app(&mut h).sys.sessions.len();
    click(&mut h, "👁 Start Watching");
    let deadline = Instant::now() + Duration::from_secs(20);
    while app(&mut h).org.watch_count == 0 && Instant::now() < deadline {
        h.step();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(app(&mut h).org.watch_count, 1);
    assert!(ws_path.join("inbox/Audio/song.mp3").exists());
    assert!(
        app(&mut h).sys.sessions.len() > sessions_before,
        "watcher sessions are undoable"
    );
    click(&mut h, "🛑 Stop Watching");
    assert!(app(&mut h).org.watcher.is_none());

    // ------------------------------------------------- Regex rules editor
    app(&mut h).tab = 8;
    h.step();
    click(&mut h, "➕ Add Rule");
    {
        let a = app(&mut h);
        a.sys.rules[0] = ("../bad".into(), "../escape".into());
    }
    h.step();
    h.step();
    assert!(
        h.query_by_label("Fix the rules marked ✖ before saving.")
            .is_some()
    );
    {
        let a = app(&mut h);
        a.sys.rules[0] = ("^inv".into(), "Bills".into());
        a.sys.rule_test = "INV_2026.pdf".into();
    }
    h.step();
    h.step();
    let _ = h.get_by_label("→ Bills");
    click(&mut h, "💾 Save Rules");
    let msgs = dismiss_all(&mut h);
    assert!(msgs[0].starts_with("Rules Saved"), "{msgs:?}");
    let saved = fs::read_to_string(data.path().join("config.json")).unwrap();
    assert!(saved.contains("^inv") && saved.contains("Bills"));
    assert_eq!(
        app(&mut h).config.custom_regex_rules(),
        [("^inv".to_string(), "Bills".to_string())]
    );

    // ------------------------------------------- Vietnamese interface text
    sfo_core::i18n::set_language("vi");
    app(&mut h).tab = 0;
    h.step();
    h.step();
    let _ = h.get_by_label("🧹 Sắp xếp & Dọn dẹp");
    let _ = h.get_by_label("⚡ Thực hiện Ngay");
    let _ = h.get_by_label("Thư mục Nguồn:");
    sfo_core::i18n::set_language("en");

    // Everything above ran against temp folders only.
    assert!(app(&mut h).sys.history_rows.len() >= 3);
}
