//! Explorer right-click menu under HKCU (no administrator rights needed).

use super::ActionResult;

pub const REG_PATH: &str = r"Software\Classes\Directory\shell\SmartFileOrganizer";
pub const REG_BG_PATH: &str = r"Software\Classes\Directory\Background\shell\SmartFileOrganizer";

const TITLE: &str = "📂 Smart File Organizer";
const ICON: &str = "shell32.dll,264";

/// `(subkey, label, extra args)`; `{dir}` is `%1` (folder) or `%V` (background).
const FOLDER_ACTIONS: [(&str, &str, &str); 3] = [
    ("1_Organize", "🧹 Smart Organize Here", ""),
    ("2_Dedupe", "🔍 Find Duplicates Here", " --dedupe"),
    ("3_GUI", "🖥️ Open in SFO GUI", " --gui"),
];
const BACKGROUND_ACTIONS: [(&str, &str, &str); 2] = [
    ("1_Organize", "🧹 Smart Organize Here", ""),
    ("2_GUI", "🖥️ Open in SFO GUI", " --gui"),
];

/// Command line stored for one menu entry.
pub fn command_line(exe: &str, placeholder: &str, extra: &str) -> String {
    format!("\"{exe}\" --source \"{placeholder}\"{extra}")
}

pub fn is_installed() -> bool {
    is_installed_at(REG_PATH)
}

/// Registers both menus, pointing at `exe` (default: the `sfo` CLI binary).
pub fn install(exe: Option<&str>) -> ActionResult {
    let exe = exe
        .map(str::to_string)
        .unwrap_or_else(|| super::sibling_exe("sfo"));
    install_at(REG_PATH, REG_BG_PATH, &exe)
}

pub fn uninstall() -> ActionResult {
    uninstall_at(REG_PATH, REG_BG_PATH)
}

#[cfg(windows)]
fn is_installed_at(path: &str) -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(path)
        .is_ok()
}

#[cfg(not(windows))]
fn is_installed_at(_path: &str) -> bool {
    false
}

#[cfg(windows)]
pub(crate) fn install_at(path: &str, bg_path: &str, exe: &str) -> ActionResult {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let write =
        |root: &str, placeholder: &str, actions: &[(&str, &str, &str)]| -> std::io::Result<()> {
            let (key, _) = hkcu.create_subkey(root)?;
            key.set_value("", &TITLE)?;
            key.set_value("SubCommands", &"")?;
            key.set_value("Icon", &ICON)?;
            for (sub, label, extra) in actions {
                let (entry, _) = hkcu.create_subkey(format!(r"{root}\shell\{sub}"))?;
                entry.set_value("", label)?;
                let (cmd, _) = entry.create_subkey("command")?;
                cmd.set_value("", &command_line(exe, placeholder, extra))?;
            }
            Ok(())
        };
    match write(path, "%1", &FOLDER_ACTIONS)
        .and_then(|()| write(bg_path, "%V", &BACKGROUND_ACTIONS))
    {
        Ok(()) => ActionResult::ok(
            "Windows Explorer folder & background context menu successfully installed for current user.",
        ),
        Err(e) => ActionResult::fail(format!("Failed to register context menu: {e}")),
    }
}

#[cfg(not(windows))]
pub(crate) fn install_at(_path: &str, _bg_path: &str, _exe: &str) -> ActionResult {
    ActionResult::fail("Context menu integration is only supported on Windows.")
}

#[cfg(windows)]
pub(crate) fn uninstall_at(path: &str, bg_path: &str) -> ActionResult {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    for p in [path, bg_path] {
        match hkcu.delete_subkey_all(p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return ActionResult::fail(format!("Failed to uninstall context menu: {e}")),
        }
    }
    ActionResult::ok("Context menu uninstalled successfully.")
}

#[cfg(not(windows))]
pub(crate) fn uninstall_at(_path: &str, _bg_path: &str) -> ActionResult {
    ActionResult::fail("Only supported on Windows.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines() {
        assert_eq!(
            command_line(r"C:\SFO\sfo.exe", "%1", " --dedupe"),
            r#""C:\SFO\sfo.exe" --source "%1" --dedupe"#
        );
        assert_eq!(command_line("sfo", "%V", ""), r#""sfo" --source "%V""#);
    }

    #[cfg(windows)]
    #[test]
    fn install_query_uninstall_in_scratch_key() {
        let k = crate::platform::TestKey::new();
        let (dir, bg) = (format!(r"{}\Dir", k.0), format!(r"{}\Bg", k.0));
        assert!(!is_installed_at(&dir));
        assert!(install_at(&dir, &bg, r"C:\x\sfo.exe").success);
        assert!(is_installed_at(&dir));
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let cmd: String = hkcu
            .open_subkey(format!(r"{dir}\shell\2_Dedupe\command"))
            .unwrap()
            .get_value("")
            .unwrap();
        assert_eq!(cmd, r#""C:\x\sfo.exe" --source "%1" --dedupe"#);
        let bg_cmd: String = hkcu
            .open_subkey(format!(r"{bg}\shell\2_GUI\command"))
            .unwrap()
            .get_value("")
            .unwrap();
        assert!(bg_cmd.contains("%V") && bg_cmd.ends_with("--gui"));
        assert!(uninstall_at(&dir, &bg).success);
        assert!(!is_installed_at(&dir));
        assert!(uninstall_at(&dir, &bg).success, "uninstall is idempotent");
    }

    #[test]
    fn real_query_does_not_panic() {
        let _ = is_installed();
    }
}
