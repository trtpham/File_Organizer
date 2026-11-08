//! Windows desktop integration: Explorer context menu, Task Scheduler,
//! start-on-login, and toast notifications. On other platforms every
//! operation reports "not supported" instead of failing.

pub mod notify;
pub mod scheduler;
pub mod shell;
pub mod startup;

/// Outcome of a platform action (Python returned `{"success", "message"}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
}

impl ActionResult {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

/// `true` on Windows.
pub fn is_supported() -> bool {
    cfg!(windows)
}

/// The running executable, used as the command target for menu entries,
/// scheduled tasks and autostart.
pub fn current_exe_string() -> String {
    std::env::current_exe()
        .map(|p| crate::paths::resolve(&p).display().to_string())
        .unwrap_or_else(|_| "sfo".into())
}

/// The `sfo` CLI binary next to the running executable (the GUI and CLI ship
/// side by side). Falls back to the current executable.
pub fn sibling_exe(name: &str) -> String {
    let exe_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(&exe_name)))
        .filter(|p| p.is_file())
        .map(|p| crate::paths::resolve(&p).display().to_string())
        .unwrap_or_else(current_exe_string)
}

/// Test-only registry scratch area under HKCU, removed by [`TestKey::drop`].
#[cfg(all(test, windows))]
pub(crate) struct TestKey(pub String);

#[cfg(all(test, windows))]
impl TestKey {
    pub(crate) fn new() -> Self {
        Self(format!(
            r"Software\SmartFileOrganizerTests\{}",
            uuid::Uuid::new_v4().simple()
        ))
    }
}

#[cfg(all(test, windows))]
impl Drop for TestKey {
    fn drop(&mut self) {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(&self.0);
        if let Ok(parent) = hkcu.open_subkey(r"Software\SmartFileOrganizerTests")
            && parent.enum_keys().next().is_none()
        {
            let _ = hkcu.delete_subkey(r"Software\SmartFileOrganizerTests");
        }
    }
}
