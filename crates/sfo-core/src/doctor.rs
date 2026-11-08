//! SFO Doctor: environment, memory, disk, integration and integrity checks.

use crate::guardrails::AuditLogChain;
use crate::reporter::format_bytes;
use crate::{cloud, paths, platform};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Check outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Warn,
    Fail,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warn => "WARN",
            Self::Fail => "FAIL",
        }
    }
}

/// One diagnostic row.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticItem {
    pub name: String,
    pub status: Status,
    pub message: String,
}

fn item(name: &str, status: Status, message: impl Into<String>) -> DiagnosticItem {
    DiagnosticItem {
        name: name.into(),
        status,
        message: message.into(),
    }
}

/// Full report.
#[derive(Debug, Clone, PartialEq)]
pub struct DoctorReport {
    pub checks: Vec<DiagnosticItem>,
}

impl DoctorReport {
    fn count(&self, s: Status) -> usize {
        self.checks.iter().filter(|c| c.status == s).count()
    }
    pub fn pass_count(&self) -> usize {
        self.count(Status::Pass)
    }
    pub fn warn_count(&self) -> usize {
        self.count(Status::Warn)
    }
    pub fn fail_count(&self) -> usize {
        self.count(Status::Fail)
    }
    /// `HEALTHY` unless any check failed.
    pub fn overall_status(&self) -> &'static str {
        if self.fail_count() == 0 {
            "HEALTHY"
        } else {
            "UNHEALTHY"
        }
    }
}

fn disk_for(path: &Path) -> Option<(u64, u64)> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter(|d| paths::is_within(path, d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| (d.available_space(), d.total_space()))
}

/// Runs every check against `target` (default `~/Downloads`).
pub fn run_full_diagnosis(target: Option<&Path>) -> DoctorReport {
    let mut out = Vec::new();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let path = paths::resolve(
        &target
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join("Downloads")),
    );
    let probe_dir = if path.is_dir() {
        path.clone()
    } else {
        home.clone()
    };

    out.push(item(
        "SFO Runtime",
        Status::Pass,
        format!(
            "Smart File Organizer {} (native Rust build)",
            crate::VERSION
        ),
    ));
    let family = if cfg!(windows) { "Windows" } else { "POSIX" };
    out.push(item(
        "Operating System",
        Status::Pass,
        format!(
            "{}-{} ({family})",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    ));

    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let (total, avail) = (sys.total_memory(), sys.available_memory());
    if total > 0 {
        let gib = 1024.0 * 1024.0 * 1024.0;
        let load = (total.saturating_sub(avail)) as f64 / total as f64 * 100.0;
        let status = if avail as f64 / gib >= 1.0 {
            Status::Pass
        } else {
            Status::Warn
        };
        out.push(item(
            "System Memory (RAM)",
            status,
            format!(
                "{:.1} GB available of {:.1} GB ({load:.0}% load)",
                avail as f64 / gib,
                total as f64 / gib
            ),
        ));
    }

    match disk_for(&probe_dir) {
        Some((free, total)) => {
            let free_gb = free as f64 / (1024.0 * 1024.0 * 1024.0);
            let status = if free_gb >= 5.0 {
                Status::Pass
            } else if free_gb >= 1.0 {
                Status::Warn
            } else {
                Status::Fail
            };
            out.push(item(
                "Disk Headroom",
                status,
                format!(
                    "{} available ({} total)",
                    format_bytes(free as f64),
                    format_bytes(total as f64)
                ),
            ));
        }
        None => out.push(item(
            "Disk Headroom",
            Status::Warn,
            "Could not determine the volume for the target path",
        )),
    }

    let bench = probe_dir.join(".sfo_doctor_benchmark.tmp");
    let start = Instant::now();
    let res = std::fs::File::create(&bench).and_then(|mut f| {
        f.write_all(&vec![b'0'; 512 * 1024])?;
        f.sync_all()
    });
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    let _ = std::fs::remove_file(&bench);
    match res {
        Ok(()) => {
            let status = if latency_ms < 50.0 {
                Status::Pass
            } else if latency_ms < 150.0 {
                Status::Warn
            } else {
                Status::Fail
            };
            out.push(item(
                "Disk Write Latency",
                status,
                format!("{latency_ms:.2} ms for 512KB flush"),
            ));
        }
        Err(e) => out.push(item(
            "Disk Health Check",
            Status::Warn,
            format!("Could not benchmark target path: {e}"),
        )),
    }

    if platform::is_supported() {
        if platform::shell::is_installed() {
            out.push(item(
                "Windows Explorer Menu",
                Status::Pass,
                "Registered under HKCU (Right-click shortcuts active)",
            ));
        } else {
            out.push(item(
                "Windows Explorer Menu",
                Status::Warn,
                "Not currently registered (Run 'sfo --install-context-menu' to enable)",
            ));
        }
    } else {
        out.push(item(
            "Windows Explorer Menu",
            Status::Pass,
            "Skipped on non-Windows platform",
        ));
    }

    let clouds = cloud::detect_all();
    if clouds.is_empty() {
        out.push(item(
            "Cloud Integrations",
            Status::Warn,
            "No active cloud storage sync directories detected",
        ));
    } else {
        let names: Vec<&str> = clouds.iter().map(|c| c.name.as_str()).collect();
        out.push(item(
            "Cloud Integrations",
            Status::Pass,
            format!("Detected: {}", names.join(", ")),
        ));
    }

    out.push(item(
        "Built-in Modules",
        Status::Pass,
        "Image decoding, EXIF, PDF text and encryption compiled in (no external dependencies)",
    ));

    let db = crate::default_data_dir().join("history.db");
    if db.exists() {
        let check =
            rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .and_then(|c| {
                    c.query_row("PRAGMA integrity_check;", [], |r| r.get::<_, String>(0))
                });
        out.push(match check {
            Ok(r) if r == "ok" => item(
                "Database Integrity",
                Status::Pass,
                "SQLite Transaction Journal is healthy",
            ),
            Ok(r) => item(
                "Database Integrity",
                Status::Fail,
                format!("SQLite integrity warning: {r}"),
            ),
            Err(e) => item(
                "Database Integrity",
                Status::Warn,
                format!("Database check warning: {e}"),
            ),
        });
    } else {
        out.push(item(
            "Database Integrity",
            Status::Pass,
            "Journal database initialized on first operation",
        ));
    }

    if let Ok(chain) = AuditLogChain::new(None) {
        let v = chain.verify_integrity();
        out.push(if v.valid {
            item(
                "Audit Log Integrity",
                Status::Pass,
                format!(
                    "SHA-256 chain valid ({} events mathematically verified)",
                    v.verified
                ),
            )
        } else {
            item(
                "Audit Log Integrity",
                Status::Fail,
                format!("Audit tampering detected: {}", v.message),
            )
        });
    }

    DoctorReport { checks: out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_diagnosis_has_core_checks() {
        let d = tempfile::tempdir().unwrap();
        let r = run_full_diagnosis(Some(d.path()));
        assert!(["HEALTHY", "UNHEALTHY"].contains(&r.overall_status()));
        assert!(r.pass_count() >= 3);
        let names: Vec<&str> = r.checks.iter().map(|c| c.name.as_str()).collect();
        for n in [
            "SFO Runtime",
            "Operating System",
            "Disk Headroom",
            "Disk Write Latency",
            "Database Integrity",
        ] {
            assert!(names.contains(&n), "{n} missing from {names:?}");
        }
        assert!(!d.path().join(".sfo_doctor_benchmark.tmp").exists());
        assert_eq!(
            r.pass_count() + r.warn_count() + r.fail_count(),
            r.checks.len()
        );
    }
}
