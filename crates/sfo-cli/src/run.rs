//! Command router (Python `cli/app.py`). Checks run in the same order as the
//! Python build; the first matching action wins.

use crate::args::Args;
use crate::out::{self, BOLD, GREEN, RED, RESET, YELLOW};
use serde_json::{Map, json};
use sfo_core::archiver;
use sfo_core::cloud;
use sfo_core::config::Config;
use sfo_core::dedup::{DedupAction, Deduplicator};
use sfo_core::doctor::{self, Status};
use sfo_core::executor::TransactionExecutor;
use sfo_core::guardrails::{self, AuditLogChain};
use sfo_core::i18n;
use sfo_core::junk;
use sfo_core::lan_share::LanShareServer;
use sfo_core::models::{ActionRecord, ActionType, OrganizeStrategy, SyncMode, SyncProfile};
use sfo_core::organizer::{Organizer, PlanOptions};
use sfo_core::paths;
use sfo_core::platform::{self, ActionResult, scheduler, shell, startup};
use sfo_core::renamer::{self, RenameOptions};
use sfo_core::reporter::{self, format_bytes};
use sfo_core::scanner::{FileScanner, ScanOptions};
use sfo_core::secrets;
use sfo_core::shredder::{self, ShredMethod};
use sfo_core::syncer;
use sfo_core::treemap;
use sfo_core::undo::UndoManager;
use sfo_core::vault::{self, KdfParams, VaultError};
use sfo_core::watcher::DirectoryWatcher;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

type Code = u8;

fn resolve(p: &str) -> PathBuf {
    paths::resolve(Path::new(p))
}

fn source_or_cwd(a: &Args) -> PathBuf {
    resolve(a.source.as_deref().unwrap_or("."))
}

fn report(r: &ActionResult) -> Code {
    if r.success {
        out::success(&r.message);
        0
    } else {
        out::error(&r.message);
        1
    }
}

fn fb(n: u64) -> String {
    format_bytes(n as f64)
}

/// Refuses a live operation over the blast-radius limit unless `--force`.
/// Returns `true` when the caller must stop.
fn blast_radius_blocks(a: &Args, config: &Config, count: u64, bytes: u64) -> bool {
    if a.force || a.dry_run {
        return false;
    }
    let (max_files, max_bytes) = config.blast_radius();
    match guardrails::check_blast_radius(count, bytes, max_files, max_bytes) {
        Some(warning) => {
            out::error(&warning);
            out::info(
                "Nothing was changed. Re-run with --force to proceed, or narrow the selection.",
            );
            true
        }
        None => false,
    }
}

/// Launches the sibling `sfo-gui` executable.
pub fn launch_gui(source: Option<&str>) -> Code {
    let gui = platform::sibling_exe("sfo-gui");
    if !Path::new(&gui)
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("sfo-gui"))
    {
        out::error("Cannot launch GUI: sfo-gui executable not found next to sfo.");
        return 1;
    }
    let mut cmd = std::process::Command::new(gui);
    if let Some(s) = source {
        cmd.args(["--source", s]);
    }
    match cmd.spawn() {
        Ok(_) => 0,
        Err(e) => {
            out::error(&format!("Cannot launch GUI: {e}"));
            1
        }
    }
}

/// Blocks until Ctrl+C.
fn wait_for_ctrl_c() {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn password(a: &Args, prompt: &str, confirm: bool) -> Option<String> {
    if let Some(p) = &a.vault_pass {
        return Some(p.clone());
    }
    let pw = rpassword::prompt_password(prompt).ok()?;
    if confirm {
        let again = rpassword::prompt_password("Confirm Vault Passphrase: ").ok()?;
        if pw != again {
            out::error("Passphrases do not match. Vault operation aborted.");
            return None;
        }
    }
    Some(pw)
}

fn vault_err(prefix: &str, e: &VaultError) -> Code {
    match e {
        VaultError::InvalidPassword => out::error("Invalid passphrase! Authentication failed."),
        other => out::error(&format!("{prefix}: {other}")),
    }
    1
}

pub fn run(a: Args) -> Code {
    if let Some(l) = &a.lang {
        i18n::set_language(l);
    }
    if a.gui {
        return launch_gui(a.source.as_deref());
    }
    out::banner();

    let config = match a.config.as_deref() {
        Some(p) => Config::load(Some(Path::new(p))),
        None => Config::load(None),
    };
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            out::error(&format!("Failed to load configuration: {e}"));
            return 1;
        }
    };
    let undo = match UndoManager::new(None) {
        Ok(u) => u,
        Err(e) => {
            out::error(&format!("Cannot open session history: {e}"));
            return 1;
        }
    };

    if a.history {
        return history(&undo);
    }
    if a.detect_usb {
        let drives = syncer::removable_drives();
        if drives.is_empty() {
            out::info("No removable USB drives currently detected.");
        } else {
            out::success(&format!("Detected {} removable drive(s):", drives.len()));
            for d in drives {
                out::plain(&format!("  - Path: {} | Label: {}", d.path, d.label));
            }
        }
        return 0;
    }
    if let (Some(src), Some(dst)) = (&a.sync_source, &a.sync_dest) {
        let mode = match a.sync_mode.as_str() {
            "mirror" => SyncMode::Mirror,
            "snapshot" => SyncMode::Snapshot,
            _ => SyncMode::Incremental,
        };
        out::info(&format!(
            "Starting {} sync: {src} -> {dst}",
            a.sync_mode.to_uppercase()
        ));
        let profile = SyncProfile {
            name: "CLI Sync".into(),
            source_dir: src.into(),
            dest_dir: dst.into(),
            mode,
            enabled: true,
            trigger_on_usb: false,
            usb_volume_name: None,
        };
        let r = syncer::sync(&profile, a.dry_run);
        if r.success {
            out::success(&r.message);
        } else {
            out::error(r.errors.first().map_or("Sync failed.", String::as_str));
        }
        return if r.success { 0 } else { 1 };
    }
    if a.install_context_menu {
        return report(&shell::install(None));
    }
    if a.uninstall_context_menu {
        return report(&shell::uninstall());
    }
    if a.analyze_disk {
        return analyze_disk(&source_or_cwd(&a));
    }
    if a.scan_junk || a.clean_junk {
        return junk_cmd(&a, &config);
    }
    if a.scan_secrets || a.quarantine_secrets {
        return secrets_cmd(&a);
    }
    if a.archive_cold {
        return archive_cold(&a, &config);
    }
    if let Some(zip) = &a.restore_archive {
        let zip = resolve(zip);
        let target = a.dest.as_deref().map(resolve);
        out::info(&format!("Restoring archive: {}...", zip.display()));
        let r = archiver::restore_archive(&zip, target.as_deref());
        for s in &r.skipped_unsafe {
            out::warning(&format!("Skipped unsafe archive member: {s}"));
        }
        return report(&ActionResult {
            success: r.success,
            message: r.message,
        });
    }
    if a.detect_cloud {
        out::info("Scanning system for local cloud storage providers...");
        let providers = cloud::detect_all();
        if providers.is_empty() {
            out::warning("No active cloud storage providers detected locally.");
        } else {
            out::success(&format!(
                "Detected {} cloud storage provider(s):",
                providers.len()
            ));
            for p in providers {
                out::plain(&format!(
                    "  - {} [{}] -> {}",
                    out::left(&p.name, 15),
                    p.provider_type,
                    p.local_path.display()
                ));
            }
        }
        return 0;
    }
    if let Some(name) = &a.sync_cloud {
        let src = source_or_cwd(&a);
        let want = name.to_lowercase();
        let Some(p) = cloud::detect_all()
            .into_iter()
            .find(|p| p.name.to_lowercase() == want || p.name.to_lowercase().contains(&want))
        else {
            out::error(&format!(
                "Cloud provider '{name}' not found on this machine."
            ));
            return 1;
        };
        out::info(&format!(
            "Syncing {} to {} ({})...",
            src.display(),
            p.name,
            p.local_path.display()
        ));
        let r = cloud::sync_to_cloud(&src, &p, "SFO_Cloud_Sync", SyncMode::Incremental, a.dry_run);
        return report(&ActionResult {
            success: r.success,
            message: r.message,
        });
    }
    if a.lan_share {
        return lan_share(&a);
    }
    if let Some(target) = &a.shred {
        return shred(&a, target, &config);
    }
    if let Some(src) = &a.vault_lock {
        return vault_lock(&a, src);
    }
    if let Some(v) = &a.vault_unlock {
        return vault_unlock(&a, v);
    }
    if let Some(v) = &a.vault_inspect {
        return vault_inspect(&a, v);
    }
    if let Some(v) = &a.vault_upgrade {
        let v = resolve(v);
        let Some(pw) = password(&a, "Enter Vault Passphrase: ", false) else {
            return 1;
        };
        return match vault::upgrade_vault(&v, &pw, KdfParams::default()) {
            Ok(old) => {
                out::success(&format!(
                    "Vault upgraded from v{old} to v2: {}",
                    v.display()
                ));
                0
            }
            Err(e) => vault_err("Vault upgrade failed", &e),
        };
    }
    if a.doctor {
        return doctor_cmd(&source_or_cwd(&a));
    }
    if a.enable_autostart {
        return report(&startup::enable_autostart(None));
    }
    if a.disable_autostart {
        return report(&startup::disable_autostart());
    }
    if a.verify_audit {
        out::info("Verifying mathematical integrity of cryptographic SHA-256 audit log...");
        let v = match AuditLogChain::new(None) {
            Ok(c) => c.verify_integrity(),
            Err(e) => {
                out::error(&format!("Cannot open audit log: {e}"));
                return 1;
            }
        };
        if v.valid {
            out::success(&v.message);
            return 0;
        }
        out::error(&format!("INTEGRITY ALERT: {}", v.message));
        return 1;
    }
    if let Some(t) = &a.schedule_daily {
        let args = format!("--source \"{}\" -r", source_or_cwd(&a).display());
        return report(&scheduler::create_daily_task("DailySweep", &args, t, None));
    }
    if a.list_tasks {
        let tasks = scheduler::list_sfo_tasks();
        if tasks.is_empty() {
            out::info("No registered SFO scheduled tasks found.");
        } else {
            out::success(&format!(
                "Found {} active SFO scheduled task(s):",
                tasks.len()
            ));
            for t in tasks {
                out::plain(&format!(
                    "  - {} Next Run: {} Status: {}",
                    out::left(&t.name, 20),
                    out::left(&t.next_run, 20),
                    t.status
                ));
            }
        }
        return 0;
    }
    if let Some(t) = &a.delete_task {
        return report(&scheduler::delete_task(t));
    }
    if let Some(id) = &a.undo {
        return undo_cmd(&undo, id);
    }

    organize(&a, &config, &undo)
}

fn history(undo: &UndoManager) -> Code {
    let sessions = match undo.list_sessions(15) {
        Ok(s) => s,
        Err(e) => {
            out::error(&format!("Cannot read session history: {e}"));
            return 1;
        }
    };
    if sessions.is_empty() {
        out::info("No recorded sessions found.");
        return 0;
    }
    let rows: Vec<Vec<String>> = sessions
        .iter()
        .map(|s| {
            vec![
                s.session_id.clone(),
                s.start_time
                    .chars()
                    .take(19)
                    .collect::<String>()
                    .replace('T', " "),
                s.strategy.clone(),
                if s.dry_run { "Dry-Run" } else { "Live" }.into(),
                s.moved_count.to_string(),
                s.duplicate_count.to_string(),
                s.status.clone(),
            ]
        })
        .collect();
    out::info("Recent Organization & Cleanup Sessions:");
    out::table(
        &[
            "Session ID",
            "Date & Time",
            "Strategy",
            "Mode",
            "Moved",
            "Dups",
            "Status",
        ],
        &rows,
    );
    0
}

fn analyze_disk(target: &Path) -> Code {
    out::info(&format!(
        "Analyzing disk usage for: {}...",
        target.display()
    ));
    let root = treemap::analyze_directory(target, 2);
    out::success(&format!("Total directory size: {}", fb(root.size)));
    out::info("\nSubdirectories and files by size:");
    for c in root.children.iter().take(12) {
        let kind = if c.is_dir { "[DIR] " } else { "[FILE]" };
        let pct = if root.size > 0 {
            c.size as f64 / root.size as f64 * 100.0
        } else {
            0.0
        };
        out::plain(&format!(
            "  {kind} {} {} ({pct:4.1}%)",
            out::left(&c.name, 30),
            out::right(&fb(c.size), 10)
        ));
    }
    out::info("\nTop 10 Largest Individual Files:");
    for f in treemap::find_largest_files(target, 10) {
        out::plain(&format!(
            "  - {} {}",
            out::left(&f.name, 35),
            out::right(&fb(f.size), 10)
        ));
    }
    0
}

fn junk_cmd(a: &Args, config: &Config) -> Code {
    let target = source_or_cwd(a);
    out::info(&format!(
        "Scanning for disposable developer bloat in: {}...",
        target.display()
    ));
    let items = junk::scan_directory(&target, 14);
    if items.is_empty() {
        out::success("No stale developer junk discovered!");
        return 0;
    }
    let total: u64 = items.iter().map(|i| i.size_bytes).sum();
    out::warning(&format!(
        "Found {} junk directories totaling {}:",
        items.len(),
        fb(total)
    ));
    for i in &items {
        out::plain(&format!(
            "  [{}] {} ({})",
            i.category,
            out::left(&i.name, 25),
            fb(i.size_bytes)
        ));
    }
    if a.clean_junk {
        if blast_radius_blocks(a, config, items.len() as u64, total) {
            return 1;
        }
        out::info("Proceeding to clean junk...");
        let r = junk::clean_junk(&items, a.dry_run);
        out::success(&r.message);
        for e in &r.errors {
            out::warning(e);
        }
    } else {
        out::info("To clean these items, re-run with: --clean-junk");
    }
    0
}

fn secrets_cmd(a: &Args) -> Code {
    let target = source_or_cwd(a);
    out::info(&format!(
        "Scanning files for exposed secrets & API credentials in: {}...",
        target.display()
    ));
    let findings = secrets::scan_directory(&target, a.recursive);
    if findings.is_empty() {
        out::success("Security scan clean! No exposed secrets detected.");
        return 0;
    }
    out::warning(&format!(
        "SECURITY ALERT: Found {} potential secret leaks:",
        findings.len()
    ));
    for f in &findings {
        out::plain(&format!(
            "  [{}] {} in {}:{} -> {}",
            f.severity,
            out::left(&f.secret_type, 25),
            sfo_core::fsutil::name_of(&f.file_path),
            f.line_number,
            f.masked_snippet
        ));
    }
    if a.quarantine_secrets {
        out::info("Quarantining compromised files...");
        match secrets::quarantine_files(&findings, &target) {
            Ok(q) => out::success(&q.message),
            Err(e) => {
                out::error(&format!("Quarantine failed: {e}"));
                return 1;
            }
        }
    } else {
        out::info("To isolate these files into _Quarantine/, re-run with: --quarantine-secrets");
    }
    0
}

fn archive_cold(a: &Args, config: &Config) -> Code {
    let target = source_or_cwd(a);
    out::info(&format!(
        "Scanning for cold files inactive for >= {} days in: {}...",
        a.archive_days,
        target.display()
    ));
    let cold = archiver::scan_cold_files(&target, a.archive_days, 0, true);
    if cold.is_empty() {
        out::success("No cold files found matching the age criteria.");
        return 0;
    }
    let total: u64 = cold.iter().map(|f| f.size_bytes).sum();
    out::info(&format!(
        "Identified {} cold files ({}).",
        cold.len(),
        fb(total)
    ));
    let dest = a
        .archive_dest
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            target.join(format!(
                "Archive_{}.zip",
                chrono::Local::now().format("%Y%m%d")
            ))
        });
    if a.archive_delete_sources && blast_radius_blocks(a, config, cold.len() as u64, total) {
        return 1;
    }
    out::info(&format!("Compressing into: {}...", dest.display()));
    let r =
        archiver::create_cold_archive(&cold, &dest, Some(&target), a.archive_delete_sources, None);
    report(&ActionResult {
        success: r.success,
        message: r.message,
    })
}

fn lan_share(a: &Args) -> Code {
    let dir = source_or_cwd(a);
    let mut server = LanShareServer::new(&dir, a.lan_port, a.lan_pin.clone());
    let url = match server.start() {
        Ok(u) => u,
        Err(e) => {
            out::error(&format!("Failed to start LAN server: {e}"));
            return 1;
        }
    };
    let rule = "=".repeat(60);
    out::success(&rule);
    out::success("  SFO LAN DROP SERVER IS ONLINE");
    out::success(&format!("  URL:  {url}"));
    if let Some(pin) = &a.lan_pin {
        out::warning(&format!("  PIN:  {pin}"));
    }
    out::info(&format!("  Sharing directory: {}", dir.display()));
    out::info("  Open this URL in any mobile browser or laptop on your Wi-Fi.");
    out::info("  Press Ctrl+C to stop server.");
    out::success(&rule);
    wait_for_ctrl_c();
    server.stop();
    out::info("\nLAN Drop server stopped.");
    0
}

fn shred(a: &Args, target: &str, config: &Config) -> Code {
    let target = resolve(target);
    if !target.exists() {
        out::error(&format!(
            "Target path to shred does not exist: {}",
            target.display()
        ));
        return 1;
    }
    let method = ShredMethod::parse(&a.shred_method).unwrap_or_default();
    let rule = "=".repeat(65);
    out::warning(&rule);
    out::warning("  [!] UNRECOVERABLE DATA SHREDDING");
    out::warning(&format!("  Target: {}", target.display()));
    out::warning(&format!(
        "  Method: {} (Physical byte overwriting & unlinking)",
        method.as_str().to_uppercase()
    ));
    out::warning(&rule);
    let audit = AuditLogChain::new(None).ok();
    if target.is_file() {
        out::info(&format!(
            "Overwriting and shredding file: {}...",
            sfo_core::fsutil::name_of(&target)
        ));
        match shredder::shred_file(&target, method, audit.as_ref(), None) {
            Ok(r) if r.success => {
                out::success(&format!(
                    "File successfully shredded and destroyed in {}s.",
                    r.duration_seconds
                ));
                0
            }
            Ok(r) => {
                out::error(&format!("Shredding failed: {}", r.error_message));
                1
            }
            Err(e) => {
                out::error(&e.to_string());
                1
            }
        }
    } else {
        out::info(&format!(
            "Recursively shredding directory tree: {}...",
            target.display()
        ));
        let files = sfo_core::fsutil::walk_files(&target);
        let bytes: u64 = files
            .iter()
            .filter_map(|f| std::fs::metadata(f).ok())
            .map(|m| m.len())
            .sum();
        if blast_radius_blocks(a, config, files.len() as u64, bytes) {
            return 1;
        }
        match shredder::shred_directory(&target, method, true, audit.as_ref(), None) {
            Ok(b) => {
                out::success(&format!(
                    "Directory shredding complete: {} file(s) destroyed in {}s.",
                    b.shredded_files.len(),
                    b.duration_seconds
                ));
                if !b.failed_files.is_empty() {
                    out::warning(&format!(
                        "Failed to shred {} file(s).",
                        b.failed_files.len()
                    ));
                    for (f, e) in &b.failed_files {
                        out::error(&format!("  - {}: {e}", f.display()));
                    }
                }
                if b.success_rate() == 100.0 { 0 } else { 1 }
            }
            Err(e) => {
                out::error(&e.to_string());
                1
            }
        }
    }
}

fn vault_lock(a: &Args, src: &str) -> Code {
    let src = resolve(src);
    if !src.exists() {
        out::error(&format!(
            "Source path to lock into vault does not exist: {}",
            src.display()
        ));
        return 1;
    }
    let dst = a
        .dest
        .as_deref()
        .map(resolve)
        .unwrap_or_else(|| src.with_extension(vault::EXTENSION));
    let Some(pw) = password(a, "Enter Vault Encryption Passphrase: ", true) else {
        return 1;
    };
    out::info(&format!(
        "Locking {} into encrypted vault: {}...",
        src.display(),
        dst.display()
    ));
    let audit = AuditLogChain::new(None).ok();
    match vault::lock_path(
        &src,
        &dst,
        &pw,
        a.vault_shred_source,
        KdfParams::default(),
        audit.as_ref(),
    ) {
        Ok(v) => {
            let size = std::fs::metadata(&v).map(|m| m.len()).unwrap_or(0);
            out::success(&format!(
                "Vault successfully locked: {} ({})",
                v.display(),
                fb(size)
            ));
            if a.vault_shred_source {
                out::success("Source file(s) securely shredded (DoD 3-pass).");
            }
            0
        }
        Err(e) => vault_err("Vault locking failed", &e),
    }
}

fn vault_unlock(a: &Args, v: &str) -> Code {
    let v = resolve(v);
    if !v.exists() {
        out::error(&format!("Vault file does not exist: {}", v.display()));
        return 1;
    }
    let dest = a
        .dest
        .as_deref()
        .map(resolve)
        .unwrap_or_else(|| resolve("."));
    let Some(pw) = password(a, "Enter Vault Decryption Passphrase: ", false) else {
        return 1;
    };
    out::info(&format!(
        "Unlocking vault {} into {}...",
        sfo_core::fsutil::name_of(&v),
        dest.display()
    ));
    let audit = AuditLogChain::new(None).ok();
    match vault::unlock_vault(&v, &dest, &pw, audit.as_ref()) {
        Ok(files) => {
            out::success(&format!(
                "Vault unlocked successfully! Extracted {} file(s) into {}.",
                files.len(),
                dest.display()
            ));
            0
        }
        Err(e) => vault_err("Vault unlocking failed", &e),
    }
}

fn vault_inspect(a: &Args, v: &str) -> Code {
    let v = resolve(v);
    if !v.exists() {
        out::error(&format!("Vault file does not exist: {}", v.display()));
        return 1;
    }
    let Some(pw) = password(a, "Enter Vault Passphrase: ", false) else {
        return 1;
    };
    match vault::inspect_vault(&v, &pw) {
        Ok(m) => {
            out::success(&format!(
                "Vault Container: {} (format v{})",
                sfo_core::fsutil::name_of(&v),
                m.created_version
            ));
            out::info(&format!(
                "Total Files: {} | Uncompressed Payload: {}",
                m.file_count,
                fb(m.total_uncompressed_bytes)
            ));
            let rule = "-".repeat(65);
            out::plain(&rule);
            for (name, size, date) in &m.files {
                out::plain(&format!(
                    "  - {} {}  {date}",
                    out::left(name, 40),
                    out::right(&fb(*size), 10)
                ));
            }
            out::plain(&rule);
            if m.created_version == 1 {
                out::warning(
                    "Legacy v1 vault. Re-encrypt it with: sfo --vault-upgrade <VAULT_FILE>",
                );
            }
            0
        }
        Err(e) => vault_err("Vault inspection failed", &e),
    }
}

fn doctor_cmd(target: &Path) -> Code {
    out::info(&format!(
        "Running SFO Doctor System Diagnostics on: {}...",
        target.display()
    ));
    let r = doctor::run_full_diagnosis(Some(target));
    let color = if r.overall_status() == "HEALTHY" {
        GREEN
    } else {
        RED
    };
    out::plain(&format!(
        "\n{color}{BOLD}>>> Overall System Status: {} ({} PASS, {} WARN, {} FAIL){RESET}\n",
        r.overall_status(),
        r.pass_count(),
        r.warn_count(),
        r.fail_count()
    ));
    let rows: Vec<Vec<String>> = r
        .checks
        .iter()
        .map(|c| {
            let styled = match c.status {
                Status::Pass => format!("{GREEN}PASS{RESET}"),
                Status::Warn => format!("{YELLOW}WARN{RESET}"),
                Status::Fail => format!("{RED}FAIL{RESET}"),
            };
            vec![c.name.clone(), styled, c.message.clone()]
        })
        .collect();
    out::table(
        &["Diagnostic Check", "Result", "Evaluation & Details"],
        &rows,
    );
    if r.overall_status() == "HEALTHY" {
        0
    } else {
        1
    }
}

fn undo_cmd(undo: &UndoManager, id: &str) -> Code {
    let target = if id == "latest" { None } else { Some(id) };
    out::info(&format!(
        "Initiating rollback for session: {}...",
        target.unwrap_or("latest")
    ));
    match undo.rollback_session(target) {
        Ok(r) if r.success => {
            out::success(&format!("Rollback completed: {}", r.message));
            out::info(&format!("Restored files: {}", r.restored_count));
            if r.failed_count > 0 {
                out::warning(&format!("Failed to restore {} files.", r.failed_count));
                for e in &r.errors {
                    out::warning(&format!("  - {e}"));
                }
            }
            0
        }
        Ok(r) => {
            out::error(&format!("Rollback failed: {}", r.message));
            1
        }
        Err(e) => {
            out::error(&format!("Rollback failed: {e}"));
            1
        }
    }
}

fn organize(a: &Args, config: &Config, undo: &UndoManager) -> Code {
    let source_str = a.source.as_deref().unwrap_or(".");
    let source = resolve(source_str);
    if !source.is_dir() {
        out::error(&format!(
            "Source directory does not exist or is invalid: {}",
            source.display()
        ));
        return 1;
    }
    if let Some(reason) = guardrails::protected_reason(&source) {
        out::error(&format!("Enterprise Guardrail Violation: {reason}"));
        return 1;
    }
    let dest = resolve(a.dest.as_deref().unwrap_or(source_str));
    let strategy = if a.by_date {
        OrganizeStrategy::Date
    } else if a.by_size {
        OrganizeStrategy::Size
    } else {
        OrganizeStrategy::parse(&a.strategy).unwrap_or_default()
    };

    if a.watch {
        out::info(&format!(
            "Starting Background Directory Watcher on: {}",
            source.display()
        ));
        out::info(&format!(
            "Destination: {} | Strategy: {}",
            dest.display(),
            strategy.as_str().to_uppercase()
        ));
        out::info("Files will be organized automatically after download completion.");
        out::warning("Press Ctrl+C to stop the watcher daemon.");
        let mut w =
            DirectoryWatcher::new(&source, Some(&dest), strategy, config.clone(), undo.clone());
        w.smart_receipts = a.smart_receipts;
        let mut handle = w.spawn(Some(Box::new(|f: &str, t: &str| {
            out::success(&format!("[Watcher] Auto-organized: {f} -> {t}"));
        })));
        wait_for_ctrl_c();
        handle.stop();
        out::info("\nWatcher stopped cleanly.");
        return 0;
    }

    out::info(&format!(
        "Scanning directory: {} (Recursive: {})...",
        source.display(),
        a.recursive
    ));
    let exclude = if paths::path_eq(&dest, &source) {
        vec![]
    } else {
        vec![dest.clone()]
    };
    let opts = ScanOptions {
        recursive: a.recursive,
        exclude_dirs: exclude,
        max_depth: None,
        control: None,
    };
    let files = match FileScanner::new(config).scan(&source, &opts, None) {
        Ok(f) => f,
        Err(e) => {
            out::error(&e.to_string());
            return 1;
        }
    };
    if files.is_empty() {
        out::warning("No files found to organize or deduplicate.");
        return 0;
    }
    let total: u64 = files.iter().map(|f| f.size_bytes).sum();
    out::info(&format!(
        "Discovered {} files ({}).",
        files.len(),
        fb(total)
    ));

    let mut records: Vec<ActionRecord> = Vec::new();
    let mode = DedupAction::parse(&a.dedupe_action);
    let report_only = a.dedupe_action == "report";

    if let Some(template) = &a.rename_template {
        out::info(&format!(
            "Planning bulk rename with template: '{template}'..."
        ));
        let opts = RenameOptions {
            template: template.clone(),
            find_text: a.rename_find.clone(),
            replace_text: a.rename_replace.clone(),
            case_transform: a.rename_case.clone(),
            ..Default::default()
        };
        records.extend(renamer::plan_rename(&files, &opts));
    } else if a.visual_dedupe {
        out::info(&format!(
            "Running Perceptual Visual Duplicate Detection (Threshold: {:.0}%)...",
            a.similarity * 100.0
        ));
        let dd = Deduplicator::new(config);
        let groups = dd.find_visual_duplicates(&files, a.similarity);
        if groups.is_empty() {
            out::success("No visual duplicate images found!");
        } else {
            out::warning(&format!(
                "Found {} visual duplicate image clusters!",
                groups.len()
            ));
            if report_only {
                for g in &groups {
                    out::plain(&format!(
                        "  Visual Cluster (pHash: {}..., Avg Sim: {:.0}%):",
                        &g.phash[..8],
                        g.similarity_score * 100.0
                    ));
                    for f in &g.files {
                        let keep = g.suggested_keep.as_ref().is_some_and(|k| k.path == f.path);
                        let tag = if keep {
                            " [ORIGINAL / BEST RES]"
                        } else {
                            " [VISUAL DUPLICATE]"
                        };
                        out::plain(&format!("    - {}{tag}", f.filename));
                    }
                }
            } else {
                records.extend(dd.plan_visual_deduplication_actions(&groups, &dest, mode));
            }
        }
    } else if a.dedupe {
        out::info("Running 3-tiered duplicate identification...");
        let dd = Deduplicator::new(config);
        let mut prog = |stage: &str, count: usize, _: usize| match stage {
            "tier1_complete" => out::info(&format!(
                "  Tier 1 (Size Clustering)      : {count} size clusters"
            )),
            "tier2_complete" => out::info(&format!(
                "  Tier 2 (128KB Partial Hashes) : {count} candidate clusters"
            )),
            "tier3_complete" => out::info(&format!(
                "  Tier 3 (Full SHA-256 Hashes)  : {count} confirmed duplicate groups"
            )),
            _ => {}
        };
        let groups = dd.find_duplicates(&files, Some(&mut prog));
        let wasted: u64 = groups.iter().map(|g| g.wasted_space_bytes()).sum();
        if groups.is_empty() {
            out::success("No duplicate files found!");
        } else {
            out::warning(&format!(
                "Found {} duplicate clusters! Wasted disk space: {}",
                groups.len(),
                fb(wasted)
            ));
            if report_only {
                for g in &groups {
                    out::plain(&format!(
                        "  Hash: {}... ({})",
                        &g.file_hash[..12],
                        fb(g.file_size)
                    ));
                    for f in &g.files {
                        let keep = g.suggested_keep.as_ref().is_some_and(|k| k.path == f.path);
                        out::plain(&format!(
                            "    - {}{}",
                            f.filename,
                            if keep { " [ORIGINAL]" } else { " [DUPLICATE]" }
                        ));
                    }
                }
            } else {
                records.extend(dd.plan_deduplication_actions(&groups, &dest, mode));
            }
        }
    }

    if a.rename_template.is_none() && !(a.dedupe && report_only) {
        let handled: HashSet<&PathBuf> = records.iter().map(|r| &r.source_path).collect();
        let remaining: Vec<_> = files
            .iter()
            .filter(|f| !handled.contains(&f.path))
            .cloned()
            .collect();
        let opts = PlanOptions {
            smart_receipts: a.smart_receipts,
            ..Default::default()
        };
        let planned = Organizer::new(config).plan(&remaining, &dest, strategy, opts);
        records.extend(planned);
    }

    if records.is_empty() {
        out::info("No actions required.");
        return 0;
    }

    let rows: Vec<Vec<String>> = records
        .iter()
        .take(15)
        .map(|r| {
            let action = serde_json::to_value(r.action_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            let sub = r
                .destination_path
                .as_ref()
                .and_then(|p| p.parent())
                .map(sfo_core::fsutil::name_of)
                .unwrap_or_else(|| "-".into());
            vec![
                action,
                sfo_core::fsutil::name_of(&r.source_path)
                    .chars()
                    .take(35)
                    .collect(),
                sub,
            ]
        })
        .collect();
    out::table(&["Action", "Source File", "Destination Subfolder"], &rows);
    if records.len() > 15 {
        out::info(&format!(
            "... and {} more planned actions.",
            records.len() - 15
        ));
    }

    let (max_files, max_bytes) = config.blast_radius();
    if !a.dry_run
        && !a.force
        && let Some(warning) = guardrails::check_records(&records, max_files, max_bytes)
    {
        out::error(&warning);
        out::info(
            "Nothing was changed. Preview with --dry-run, then re-run with --force to proceed.",
        );
        return 1;
    }

    let mode_str = if a.dry_run {
        "DRY-RUN SIMULATION"
    } else {
        "LIVE EXECUTION"
    };
    out::info(&format!(
        "Starting {mode_str} for {} operations...",
        records.len()
    ));
    let session = TransactionExecutor::new(config).execute_session(
        records,
        &source,
        &dest,
        strategy.as_str(),
        a.dry_run,
        None,
    );
    if let Err(e) = undo.save_session(&session) {
        out::warning(&format!(
            "Could not save the session journal (undo unavailable): {e}"
        ));
    }
    if let Ok(chain) = AuditLogChain::new(None) {
        let mut d = Map::new();
        d.insert("session_id".into(), json!(session.session_id));
        d.insert("strategy".into(), json!(session.strategy));
        d.insert("dry_run".into(), json!(session.dry_run));
        d.insert("moved_count".into(), json!(session.moved_count));
        d.insert("source_dir".into(), json!(source.display().to_string()));
        d.insert("dest_dir".into(), json!(dest.display().to_string()));
        let _ = chain.log_event("SESSION_EXECUTE", d, None);
    }

    out::plain(&reporter::console_summary(&session));
    for r in session
        .records
        .iter()
        .filter(|r| r.action_type != ActionType::Skip)
    {
        if r.status == sfo_core::models::RecordStatus::Failed {
            out::warning(&format!(
                "{}: {}",
                sfo_core::fsutil::name_of(&r.source_path),
                r.message
            ));
        }
    }
    if let Some(path) = &a.report {
        match reporter::write_html(&session, Path::new(path)) {
            Ok(p) => out::success(&format!(
                "HTML dashboard generated: {}",
                paths::resolve(&p).display()
            )),
            Err(e) => out::error(&format!("Could not write HTML report: {e}")),
        }
    }
    if !a.dry_run && session.moved_count > 0 {
        out::info(&format!(
            "To undo this session at any time, run: sfo --undo {}",
            session.session_id
        ));
    }
    0
}
