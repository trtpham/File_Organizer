//! Start-on-login via `HKCU\...\CurrentVersion\Run`.

use super::ActionResult;

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const APP_NAME: &str = "SmartFileOrganizer";

/// Default command: the `sfo` CLI watching the user's Downloads folder.
///
/// The folder must be explicit: programs started from the Run key get
/// `C:\Windows\System32` as their working directory, which the guardrails
/// refuse, so a bare `--watch` (as in v3.0) exited at every login.
pub fn default_command() -> String {
    let downloads = dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    format!(
        "\"{}\" --source \"{}\" --watch",
        super::sibling_exe("sfo"),
        downloads.display()
    )
}

pub fn is_autostart_enabled() -> bool {
    is_enabled_at(RUN_KEY, APP_NAME)
}

pub fn enable_autostart(command: Option<&str>) -> ActionResult {
    let cmd = command.map(str::to_string).unwrap_or_else(default_command);
    enable_at(RUN_KEY, APP_NAME, &cmd)
}

pub fn disable_autostart() -> ActionResult {
    disable_at(RUN_KEY, APP_NAME)
}

#[cfg(windows)]
fn is_enabled_at(key: &str, name: &str) -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(key)
        .and_then(|k| k.get_raw_value(name))
        .is_ok()
}

#[cfg(not(windows))]
fn is_enabled_at(_key: &str, _name: &str) -> bool {
    false
}

#[cfg(windows)]
pub(crate) fn enable_at(key: &str, name: &str, command: &str) -> ActionResult {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    match hkcu
        .create_subkey(key)
        .and_then(|(k, _)| k.set_value(name, &command))
    {
        Ok(()) => ActionResult::ok("Smart File Organizer registered for Windows startup."),
        Err(e) => ActionResult::fail(format!("Failed to enable autostart: {e}")),
    }
}

#[cfg(not(windows))]
pub(crate) fn enable_at(_key: &str, _name: &str, _command: &str) -> ActionResult {
    ActionResult::fail("Autostart is only supported on Windows.")
}

#[cfg(windows)]
pub(crate) fn disable_at(key: &str, name: &str) -> ActionResult {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    match hkcu
        .open_subkey_with_flags(key, KEY_SET_VALUE)
        .and_then(|k| k.delete_value(name))
    {
        Ok(()) => ActionResult::ok("Smart File Organizer removed from Windows startup."),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            ActionResult::ok("Autostart was not enabled.")
        }
        Err(e) => ActionResult::fail(format!("Failed to disable autostart: {e}")),
    }
}

#[cfg(not(windows))]
pub(crate) fn disable_at(_key: &str, _name: &str) -> ActionResult {
    ActionResult::fail("Autostart is only supported on Windows.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_watches() {
        let cmd = default_command();
        assert!(cmd.ends_with("\" --watch"), "{cmd}");
        assert!(
            cmd.contains("--source \""),
            "watcher needs an explicit folder: {cmd}"
        );
        let _ = is_autostart_enabled();
    }

    #[cfg(windows)]
    #[test]
    fn enable_disable_in_scratch_key() {
        let k = crate::platform::TestKey::new();
        assert!(!is_enabled_at(&k.0, "SFOTest"));
        assert!(enable_at(&k.0, "SFOTest", r#""C:\x\sfo.exe" --watch"#).success);
        assert!(is_enabled_at(&k.0, "SFOTest"));
        assert!(disable_at(&k.0, "SFOTest").success);
        assert!(!is_enabled_at(&k.0, "SFOTest"));
        assert_eq!(
            disable_at(&k.0, "SFOTest").message,
            "Autostart was not enabled."
        );
    }
}
