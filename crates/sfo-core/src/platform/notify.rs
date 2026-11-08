//! Native Windows 10/11 toast notifications via PowerShell + WinRT.
//!
//! Title and message travel as environment variables, never spliced into
//! the script, so file names with quotes or `$(...)` cannot inject code
//! (the Python build interpolated them).

use std::process::Command;

const SCRIPT: &str = r#"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$textNodes = $template.GetElementsByTagName("text")
$textNodes.Item(0).AppendChild($template.CreateTextNode($env:SFO_TOAST_TITLE)) > $null
$textNodes.Item(1).AppendChild($template.CreateTextNode($env:SFO_TOAST_MESSAGE)) > $null
$toast = [Windows.UI.Notifications.ToastNotification]::new($template)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("Smart File Organizer").Show($toast)
"#;

/// The PowerShell invocation (exposed for tests).
pub fn toast_command(title: &str, message: &str) -> Command {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("SFO_TOAST_TITLE", title)
        .env("SFO_TOAST_MESSAGE", message);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Shows a toast; `false` off Windows or if PowerShell could not be started.
pub fn notify(title: &str, message: &str) -> bool {
    if !super::is_supported() {
        return false;
    }
    toast_command(title, message).output().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_passed_out_of_band() {
        let cmd = toast_command("Q\"$(rm -rf)", "msg");
        let envs: Vec<_> = cmd
            .get_envs()
            .map(|(k, v)| (k.to_os_string(), v.map(|v| v.to_os_string())))
            .collect();
        assert!(
            envs.iter()
                .any(|(k, v)| k == "SFO_TOAST_TITLE"
                    && v.as_deref() == Some("Q\"$(rm -rf)".as_ref()))
        );
        assert!(
            cmd.get_args()
                .all(|a| !a.to_string_lossy().contains("rm -rf"))
        );
    }
}
