//! End-to-end tests of the `sfo` binary (port of Python `tests/test_cli.py`).
//! Each test gets its own `SFO_DATA_DIR`, so history and the audit log never
//! touch the real `~/.smart_file_organizer`.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

struct Env {
    data: tempfile::TempDir,
    ws: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            data: tempfile::tempdir().unwrap(),
            ws: tempfile::tempdir().unwrap(),
        }
    }

    fn sfo(&self, args: &[&str]) -> (i32, String) {
        let o: Output = Command::new(env!("CARGO_BIN_EXE_sfo"))
            .args(args)
            .env("SFO_DATA_DIR", self.data.path())
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        (o.status.code().unwrap_or(-1), text)
    }

    fn populate(&self) -> &Path {
        let ws = self.ws.path();
        for (n, c) in [
            ("photo1.jpg", &b"JPEG_IMAGE_DATA_12345"[..]),
            ("photo2.png", b"PNG_IMAGE_DATA_67890"),
            ("report.pdf", b"PDF_DOCUMENT_CONTENT_ABCDE"),
            ("notes.txt", b"PLAIN_TEXT_NOTES"),
            ("archive.zip", b"PK_ZIP_ARCHIVE_DATA"),
            ("script.py", b"print('Hello world!')"),
            ("download.crdownload", b"INCOMPLETE"),
        ] {
            fs::write(ws.join(n), c).unwrap();
        }
        let dup = b"DUPLICATE_FILE_CONTENT_IDENTICAL_1234567890".repeat(200);
        for n in ["original_file.pdf", "copy_of_file.pdf", "another_copy.pdf"] {
            fs::write(ws.join(n), &dup).unwrap();
        }
        ws
    }
}

fn s(p: &Path) -> String {
    p.display().to_string()
}

#[test]
fn help_and_version() {
    let e = Env::new();
    let (code, out) = e.sfo(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("Smart File Organizer"));
    assert!(out.contains("--dedupe-action") && out.contains("--vault-upgrade"));
    let (code, out) = e.sfo(&["--version"]);
    assert_eq!(code, 0);
    assert!(out.contains("4.0.0"));
    assert_eq!(
        e.sfo(&["--strategy", "nope"]).0,
        2,
        "argparse-style usage errors exit 2"
    );
}

#[test]
fn dry_run_then_live_then_undo_and_history() {
    let e = Env::new();
    let ws = e.populate();
    let (code, out) = e.sfo(&["--source", &s(ws), "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("DRY RUN") || out.contains("DRY-RUN"));
    assert!(out.contains("photo1.jpg"));
    assert!(ws.join("photo1.jpg").exists());

    let (code, out) = e.sfo(&["--source", &s(ws)]);
    assert_eq!(code, 0, "{out}");
    assert!(ws.join("Images/photo1.jpg").exists());
    assert!(
        ws.join("download.crdownload").exists(),
        "ignored files stay"
    );
    assert!(out.contains("sfo --undo sfo_"));

    let (code, out) = e.sfo(&["--history"]);
    assert_eq!(code, 0);
    assert!(out.contains("Live") && out.contains("Dry-Run") && out.contains("COMPLETED"));

    let (code, out) = e.sfo(&["--undo"]);
    assert_eq!(code, 0, "{out}");
    assert!(ws.join("photo1.jpg").exists());
    assert!(!ws.join("Images").exists());
    assert!(e.sfo(&["--verify-audit"]).1.contains("verified clean"));
}

#[test]
fn history_empty() {
    let e = Env::new();
    let (code, out) = e.sfo(&["--history"]);
    assert_eq!(code, 0);
    assert!(out.contains("No recorded sessions found."));
}

#[test]
fn dedupe_report_and_move() {
    let e = Env::new();
    let ws = e.populate();
    let (code, out) = e.sfo(&["--source", &s(ws), "--dedupe", "--dedupe-action", "report"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Found 1 duplicate clusters"));
    assert!(out.contains("[ORIGINAL]") && out.contains("[DUPLICATE]"));
    assert!(ws.join("copy_of_file.pdf").exists());

    let (code, out) = e.sfo(&["--source", &s(ws), "--dedupe"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(fs::read_dir(ws.join("_Duplicates")).unwrap().count(), 2);
}

#[test]
fn rename_template() {
    let e = Env::new();
    let ws = e.ws.path();
    fs::write(ws.join("old_document.txt"), "x").unwrap();
    let (code, out) = e.sfo(&["--source", &s(ws), "--rename-template", "{counter}_{name}"]);
    assert_eq!(code, 0, "{out}");
    assert!(ws.join("001_old_document.txt").exists());
}

#[test]
fn detect_cloud() {
    let (code, out) = Env::new().sfo(&["--detect-cloud"]);
    assert_eq!(code, 0);
    assert!(out.to_lowercase().contains("cloud storage provider"));
}

#[test]
fn archive_cold_empty() {
    let e = Env::new();
    let (code, out) = e.sfo(&[
        "--source",
        &s(e.ws.path()),
        "--archive-cold",
        "--archive-days",
        "365",
    ]);
    assert_eq!(code, 0);
    assert!(out.to_lowercase().contains("cold files"));
}

#[test]
fn shred_file() {
    let e = Env::new();
    let f = e.ws.path().join("to_shred.bin");
    fs::write(&f, [0x12u8, 0x34, 0x56].repeat(50)).unwrap();
    let (code, out) = e.sfo(&["--shred", &s(&f), "--shred-method", "quick_1pass"]);
    assert_eq!(code, 0, "{out}");
    assert!(!f.exists());
    assert!(out.to_lowercase().contains("successfully shredded"));
    assert_eq!(e.sfo(&["--shred", &s(&f)]).0, 1, "missing target fails");
}

#[test]
fn vault_lifecycle() {
    let e = Env::new();
    let ws = e.ws.path();
    let secret = ws.join("confidential.doc");
    fs::write(&secret, "Company Secret Roadmap 2026").unwrap();
    let vault = ws.join("my_secrets.sfovault");
    let pw = "TestPassword#123";

    let (code, out) = e.sfo(&[
        "--vault-lock",
        &s(&secret),
        "--dest",
        &s(&vault),
        "--vault-pass",
        pw,
        "--vault-shred-source",
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(vault.exists() && !secret.exists());

    let (code, out) = e.sfo(&["--vault-inspect", &s(&vault), "--vault-pass", pw]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("confidential.doc") && out.contains("format v2"));

    let (code, out) = e.sfo(&["--vault-inspect", &s(&vault), "--vault-pass", "wrong"]);
    assert_eq!(code, 1);
    assert!(out.contains("Invalid passphrase"));

    let extract = ws.join("unlocked_data");
    let (code, out) = e.sfo(&[
        "--vault-unlock",
        &s(&vault),
        "--dest",
        &s(&extract),
        "--vault-pass",
        pw,
    ]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        fs::read_to_string(extract.join("confidential.doc")).unwrap(),
        "Company Secret Roadmap 2026"
    );
}

#[test]
fn guardrail_blocks_drive_root() {
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let (code, out) = Env::new().sfo(&["--source", root, "--dry-run"]);
    assert_eq!(code, 1);
    assert!(out.contains("Enterprise Guardrail Violation"));
}

#[test]
fn doctor_runs() {
    let e = Env::new();
    let (code, out) = e.sfo(&["--doctor", "--source", &s(e.ws.path())]);
    assert!(code == 0 || code == 1);
    assert!(out.contains("Overall System Status") && out.contains("Disk Headroom"));
}

#[test]
fn scan_secrets_and_junk() {
    let e = Env::new();
    let ws = e.ws.path();
    fs::write(ws.join(".env"), "AWS=AKIAABCDEFGHIJKLMNOP\n").unwrap();
    let (code, out) = e.sfo(&["--source", &s(ws), "--scan-secrets"]);
    assert_eq!(code, 0);
    assert!(out.contains("AKIA************MNOP"));
    fs::create_dir_all(ws.join("p/__pycache__")).unwrap();
    let (code, out) = e.sfo(&["--source", &s(ws), "--scan-junk"]);
    assert_eq!(code, 0);
    assert!(out.contains("__pycache__"));
}
