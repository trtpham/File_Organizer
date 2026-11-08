//! Session reports: console text, Markdown, standalone HTML dashboard.

use crate::error::Result;
use crate::models::{RecordStatus, SessionLog};
use crate::pyjson::py_float_repr;
use std::path::{Path, PathBuf};

/// `1536` → `"1.5 KB"` (Python `Reporter.format_bytes`).
pub fn format_bytes(num_bytes: f64) -> String {
    let mut n = num_bytes;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if n.abs() < 1024.0 {
            return format!("{n:3.1} {unit}");
        }
        n /= 1024.0;
    }
    format!("{n:.1} PB")
}

fn status_str(s: RecordStatus) -> &'static str {
    match s {
        RecordStatus::Pending => "PENDING",
        RecordStatus::Success => "SUCCESS",
        RecordStatus::Skipped => "SKIPPED",
        RecordStatus::Failed => "FAILED",
    }
}

fn action_str(r: &crate::models::ActionRecord) -> String {
    serde_json::to_value(r.action_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Minimal HTML escaping (the Python build emitted names unescaped).
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn console_summary(s: &SessionLog) -> String {
    let dur = s
        .end_time
        .map(|e| {
            format!(
                " ({:.2}s)",
                (e - s.start_time).num_microseconds().unwrap_or(0) as f64 / 1e6
            )
        })
        .unwrap_or_default();
    let eq = "=".repeat(60);
    let dash = "-".repeat(60);
    [
        eq.clone(),
        format!(" SMART FILE ORGANIZER - SESSION REPORT{dur}"),
        eq.clone(),
        format!("Session ID     : {}", s.session_id),
        format!(
            "Mode           : {}",
            if s.dry_run {
                "[DRY RUN - SIMULATION]"
            } else {
                "[LIVE EXECUTION]"
            }
        ),
        format!("Strategy       : {}", s.strategy.to_uppercase()),
        format!("Source Dir     : {}", s.source_dir),
        format!("Destination Dir: {}", s.destination_dir),
        dash,
        format!(
            "Total Processed: {} files ({})",
            s.total_files,
            format_bytes(s.total_size_bytes as f64)
        ),
        format!("Moved/Cleaned  : {} files", s.moved_count),
        format!("Skipped        : {} files", s.skipped_count),
        format!("Duplicates     : {} files", s.duplicate_count),
        format!(
            "Space Saved    : {}",
            format_bytes(s.space_saved_bytes as f64)
        ),
        eq,
    ]
    .join("\n")
}

pub fn markdown(s: &SessionLog) -> String {
    let badge = if s.dry_run {
        "🛡️ **DRY-RUN SIMULATION**"
    } else {
        "⚡ **LIVE EXECUTION**"
    };
    let mut lines = vec![
        "# Smart File Organizer Report".to_string(),
        String::new(),
        format!("- **Session ID:** `{}`", s.session_id),
        format!("- **Mode:** {badge}"),
        format!("- **Strategy:** `{}`", s.strategy),
        format!("- **Source:** `{}`", s.source_dir),
        format!("- **Destination:** `{}`", s.destination_dir),
        String::new(),
        "## Summary Metrics".into(),
        String::new(),
        "| Metric | Value |".into(),
        "| :--- | :--- |".into(),
        format!("| Total Files Processed | {} |", s.total_files),
        format!(
            "| Total Size | {} |",
            format_bytes(s.total_size_bytes as f64)
        ),
        format!("| Moved / Organized | {} |", s.moved_count),
        format!("| Skipped Files | {} |", s.skipped_count),
        format!("| Duplicates Resolved | {} |", s.duplicate_count),
        format!(
            "| Space Reclaimed | {} |",
            format_bytes(s.space_saved_bytes as f64)
        ),
        String::new(),
        "## Action Details".into(),
        String::new(),
        "| Action | Source File | Target / Status | Message |".into(),
        "| :--- | :--- | :--- | :--- |".into(),
    ];
    for r in s.records.iter().take(100) {
        let dst = r
            .destination_path
            .as_deref()
            .map(name_of)
            .unwrap_or_else(|| "-".into());
        lines.push(format!(
            "| {} | `{}` | `{}` ({}) | {} |",
            action_str(r),
            name_of(&r.source_path),
            dst,
            status_str(r.status),
            r.message
        ));
    }
    if s.records.len() > 100 {
        lines.push(format!(
            "| ... | *and {} more items* | | |",
            s.records.len() - 100
        ));
    }
    lines.join("\n")
}

const HTML_TEMPLATE: &str = include_str!("../data/report_template.html");

/// Writes the standalone HTML dashboard to `output_path`.
pub fn write_html(s: &SessionLog, output_path: &Path) -> Result<PathBuf> {
    if let Some(dir) = output_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let rows: String = s
        .records
        .iter()
        .map(|r| {
            let badge = match r.status {
                RecordStatus::Success => "badge-success",
                RecordStatus::Skipped => "badge-warning",
                _ => "badge-danger",
            };
            let dst = r.destination_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into());
            format!(
                "\n                <tr class=\"action-row\">\n                    <td><span class=\"badge {badge}\">{}</span></td>\n                    <td title=\"{}\"><strong>{}</strong></td>\n                    <td title=\"{d}\">{d}</td>\n                    <td>{}</td>\n                    <td>{}</td>\n                </tr>\n            ",
                action_str(r),
                esc(&r.source_path.display().to_string()),
                esc(&name_of(&r.source_path)),
                format_bytes(r.file_size as f64),
                esc(&r.message),
                d = esc(&dst),
            )
        })
        .collect();

    let total = s.total_files.max(1) as f64;
    let pct = |n: u64| n as f64 / total * 100.0;
    let (moved, dups, skip) = (
        pct(s.moved_count),
        pct(s.duplicate_count),
        pct(s.skipped_count),
    );
    let html = HTML_TEMPLATE
        .replace("{{SESSION_ID}}", &esc(&s.session_id))
        .replace("{{MODE}}", if s.dry_run { "DRY RUN" } else { "LIVE" })
        .replace("{{STRATEGY}}", &esc(&s.strategy.to_uppercase()))
        .replace("{{TOTAL}}", &s.total_files.to_string())
        .replace("{{MOVED}}", &s.moved_count.to_string())
        .replace("{{DUPS}}", &s.duplicate_count.to_string())
        .replace("{{SAVED}}", &format_bytes(s.space_saved_bytes as f64))
        .replace("{{MOVED_PCT}}", &py_float_repr(moved))
        .replace("{{DUPS_PCT}}", &py_float_repr(dups))
        .replace("{{SKIP_PCT}}", &py_float_repr(skip))
        .replace("{{MOVED_PCT_1}}", &format!("{moved:.1}"))
        .replace("{{DUPS_PCT_1}}", &format!("{dups:.1}"))
        .replace("{{SKIP_PCT_1}}", &format!("{skip:.1}"))
        .replace("{{ROWS}}", &rows);
    std::fs::write(output_path, html)?;
    Ok(output_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ActionRecord, ActionType};

    fn session() -> SessionLog {
        let start = crate::paths::py_fromisoformat("2026-11-08T10:00:00").unwrap();
        let mut s = SessionLog::new("sfo_test".into(), start);
        s.end_time = Some(start + chrono::Duration::milliseconds(1500));
        s.total_files = 3;
        s.moved_count = 1;
        s.skipped_count = 2;
        s.total_size_bytes = 1536;
        let mut r = ActionRecord::new(
            ActionType::Move,
            "a/<evil>.txt".into(),
            Some("b/x.txt".into()),
            10,
        );
        r.status = RecordStatus::Success;
        r.message = "Moved".into();
        s.records = vec![r; 101];
        s
    }

    #[test]
    fn bytes_format() {
        assert_eq!(format_bytes(500.0), "500.0 B");
        assert_eq!(format_bytes(1536.0), "1.5 KB");
        assert_eq!(format_bytes(5.0 * 1024.0 * 1024.0 * 1024.0), "5.0 GB");
    }

    #[test]
    fn console_and_markdown() {
        let s = session();
        let c = console_summary(&s);
        assert!(c.contains("SESSION REPORT (1.50s)"));
        assert!(c.contains("Total Processed: 3 files (1.5 KB)"));
        let md = markdown(&s);
        assert!(md.contains("| MOVE | `<evil>.txt` | `x.txt` (SUCCESS) | Moved |"));
        assert!(md.contains("*and 1 more items*"));
    }

    #[test]
    fn html_is_escaped_and_complete() {
        let d = tempfile::tempdir().unwrap();
        let out = write_html(&session(), &d.path().join("r/report.html")).unwrap();
        let html = std::fs::read_to_string(out).unwrap();
        assert!(html.contains("&lt;evil&gt;.txt") && !html.contains("<evil>"));
        assert!(html.contains("width: 33.33333333333333%"));
        assert!(!html.contains("{{"));
    }
}
