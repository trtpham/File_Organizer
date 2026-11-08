//! Windows Task Scheduler (`schtasks.exe`) integration.

use super::ActionResult;
use std::process::Command;

/// Every SFO task is named `SmartFileOrganizer_<name>`.
pub const FOLDER_PREFIX: &str = "SmartFileOrganizer";

/// One row of [`list_sfo_tasks`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub name: String,
    pub next_run: String,
    pub status: String,
}

pub fn full_task_name(task_name: &str) -> String {
    format!("{FOLDER_PREFIX}_{task_name}")
}

/// `HH:MM`, 24-hour.
pub fn valid_time(t: &str) -> bool {
    let Some((h, m)) = t.split_once(':') else {
        return false;
    };
    h.len() == 2
        && m.len() == 2
        && h.parse::<u8>().is_ok_and(|h| h < 24)
        && m.parse::<u8>().is_ok_and(|m| m < 60)
}

/// Arguments for `schtasks /Create` (daily at `start_time`, replace if present).
pub fn create_args(task_name: &str, exe: &str, arguments: &str, start_time: &str) -> Vec<String> {
    let tr = format!("\"{exe}\" {arguments}");
    [
        "/Create",
        "/TN",
        &full_task_name(task_name),
        "/TR",
        &tr,
        "/SC",
        "DAILY",
        "/ST",
        start_time,
        "/F",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn schtasks(args: &[String]) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new("schtasks");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.output()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).trim().to_string()
}

/// Creates or replaces a daily task running the `sfo` CLI with `arguments`.
pub fn create_daily_task(
    task_name: &str,
    arguments: &str,
    start_time: &str,
    exe: Option<&str>,
) -> ActionResult {
    if !super::is_supported() {
        return ActionResult::fail("Task Scheduler is only supported on Windows.");
    }
    if !valid_time(start_time) {
        return ActionResult::fail(format!(
            "Invalid start time '{start_time}'. Use HH:MM (24-hour)."
        ));
    }
    let exe = exe
        .map(str::to_string)
        .unwrap_or_else(|| super::sibling_exe("sfo"));
    match schtasks(&create_args(task_name, &exe, arguments, start_time)) {
        Ok(o) if o.status.success() => ActionResult::ok(format!(
            "Successfully scheduled daily task '{task_name}' at {start_time}."
        )),
        Ok(o) => {
            let err = if o.stderr.is_empty() {
                text(&o.stdout)
            } else {
                text(&o.stderr)
            };
            ActionResult::fail(format!("schtasks error: {err}"))
        }
        Err(e) => ActionResult::fail(format!("Failed to invoke schtasks: {e}")),
    }
}

pub fn delete_task(task_name: &str) -> ActionResult {
    if !super::is_supported() {
        return ActionResult::fail("Only supported on Windows.");
    }
    let args: Vec<String> = ["/Delete", "/TN", &full_task_name(task_name), "/F"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    match schtasks(&args) {
        Ok(o) if o.status.success() => ActionResult::ok(format!("Task '{task_name}' deleted.")),
        Ok(o) => {
            let err = text(&o.stderr);
            ActionResult::fail(if err.is_empty() {
                "Task not found.".into()
            } else {
                err
            })
        }
        Err(e) => ActionResult::fail(e.to_string()),
    }
}

/// Parses `schtasks /Query /FO CSV /NH` output.
pub fn parse_task_csv(csv: &str) -> Vec<ScheduledTask> {
    csv.lines()
        .filter(|l| l.contains(FOLDER_PREFIX))
        .filter_map(|l| {
            let parts: Vec<String> = l
                .split("\",\"")
                .map(|p| p.trim_matches(|c| c == ' ' || c == '"').to_string())
                .collect();
            (parts.len() >= 3).then(|| ScheduledTask {
                name: parts[0]
                    .trim_start_matches('\\')
                    .replace(&format!("{FOLDER_PREFIX}_"), ""),
                next_run: parts[1].clone(),
                status: parts[2].clone(),
            })
        })
        .collect()
}

pub fn list_sfo_tasks() -> Vec<ScheduledTask> {
    if !super::is_supported() {
        return Vec::new();
    }
    let args: Vec<String> = ["/Query", "/FO", "CSV", "/NH"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    match schtasks(&args) {
        Ok(o) if o.status.success() => parse_task_csv(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_create_command() {
        let a = create_args(
            "DailySweep",
            r"C:\Program Files\SFO\sfo.exe",
            r#"--source "D:\Down loads" -r"#,
            "18:00",
        );
        assert_eq!(a[2], "SmartFileOrganizer_DailySweep");
        assert_eq!(
            a[4],
            r#""C:\Program Files\SFO\sfo.exe" --source "D:\Down loads" -r"#
        );
        assert_eq!(&a[5..], ["/SC", "DAILY", "/ST", "18:00", "/F"]);
    }

    #[test]
    fn time_validation() {
        assert!(valid_time("18:00") && valid_time("00:59"));
        assert!(
            !valid_time("24:00") && !valid_time("8:00") && !valid_time("18:60") && !valid_time("x")
        );
        if !crate::platform::is_supported() {
            assert!(!create_daily_task("X", "", "18:00", None).success);
        } else {
            assert!(
                create_daily_task("X", "", "25:00", None)
                    .message
                    .starts_with("Invalid start time")
            );
        }
    }

    #[test]
    fn parses_csv() {
        let csv = "\"\\SmartFileOrganizer_DailySweep\",\"11/9/2026 6:00:00 PM\",\"Ready\"\n\"\\Other\",\"N/A\",\"Ready\"\n";
        assert_eq!(
            parse_task_csv(csv),
            [ScheduledTask {
                name: "DailySweep".into(),
                next_run: "11/9/2026 6:00:00 PM".into(),
                status: "Ready".into()
            }]
        );
        let _ = list_sfo_tasks();
    }
}
