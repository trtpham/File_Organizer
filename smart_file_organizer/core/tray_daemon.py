"""
Windows System Tray Daemon & Native Notification Manager.
Provides OS startup configuration (HKCU Run), desktop toast notifications,
and background daemon coordination.
Zero external dependencies.
"""

import os
import sys
import subprocess
from pathlib import Path
from typing import Optional, Dict, Any


class WindowsStartupManager:
    """Manages Windows auto-start on boot via HKCU Run registry key."""

    REG_KEY_PATH = r"Software\Microsoft\Windows\CurrentVersion\Run"
    APP_NAME = "SmartFileOrganizer"

    @classmethod
    def is_supported(cls) -> bool:
        return os.name == "nt"

    @classmethod
    def is_autostart_enabled(cls) -> bool:
        if not cls.is_supported():
            return False
        import winreg
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, cls.REG_KEY_PATH, 0, winreg.KEY_READ) as key:
                winreg.QueryValueEx(key, cls.APP_NAME)
                return True
        except FileNotFoundError:
            return False
        except Exception:
            return False

    @classmethod
    def enable_autostart(cls, command: Optional[str] = None) -> Dict[str, Any]:
        """Registers application to launch on user login."""
        if not cls.is_supported():
            return {"success": False, "message": "Autostart is only supported on Windows."}

        import winreg
        if not command:
            py_exe = sys.executable
            main_script = Path(__file__).resolve().parent.parent.parent / "main.py"
            # Launch in background / minimized
            command = f'"{py_exe}" "{main_script}" --watch'

        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, cls.REG_KEY_PATH, 0, winreg.KEY_SET_VALUE) as key:
                winreg.SetValueEx(key, cls.APP_NAME, 0, winreg.REG_SZ, command)
            return {"success": True, "message": "Smart File Organizer registered for Windows startup."}
        except Exception as e:
            return {"success": False, "message": f"Failed to enable autostart: {str(e)}"}

    @classmethod
    def disable_autostart(cls) -> Dict[str, Any]:
        """Removes application from Windows startup."""
        if not cls.is_supported():
            return {"success": False, "message": "Autostart is only supported on Windows."}

        import winreg
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, cls.REG_KEY_PATH, 0, winreg.KEY_SET_VALUE) as key:
                winreg.DeleteValue(key, cls.APP_NAME)
            return {"success": True, "message": "Smart File Organizer removed from Windows startup."}
        except FileNotFoundError:
            return {"success": True, "message": "Autostart was not enabled."}
        except Exception as e:
            return {"success": False, "message": f"Failed to disable autostart: {str(e)}"}


class DesktopNotifier:
    """Sends native Windows 10/11 desktop toast notifications."""

    @classmethod
    def notify(cls, title: str, message: str) -> bool:
        """Sends a native toast alert without requiring third-party libraries."""
        if os.name != "nt":
            return False

        # Escape strings for PowerShell
        clean_title = title.replace('"', '`"').replace("'", "''")
        clean_msg = message.replace('"', '`"').replace("'", "''")

        ps_script = f"""
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$textNodes = $template.GetElementsByTagName("text")
$textNodes.Item(0).AppendChild($template.CreateTextNode("{clean_title}")) > $null
$textNodes.Item(1).AppendChild($template.CreateTextNode("{clean_msg}")) > $null
$toast = [Windows.UI.Notifications.ToastNotification]::new($template)
$notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("Smart File Organizer")
$notifier.Show($toast)
"""
        try:
            # Run PowerShell silently with hidden window
            flags = 0x08000000  # CREATE_NO_WINDOW
            subprocess.run(
                ["powershell", "-NoProfile", "-NonInteractive", "-Command", ps_script],
                creationflags=flags,
                timeout=5,
                capture_output=True
            )
            return True
        except Exception:
            return False
