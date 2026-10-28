"""
Windows Explorer Context Menu Integration (Shell Extension).
Enables right-click actions on folders and background folder spaces without requiring Administrator privileges.
Enhanced with Background Context Menu registration (%V) and shell icons.
"""

import sys
import os
from pathlib import Path
from typing import Dict, Any, Optional


class WindowsShellIntegration:
    """Manages Windows Explorer context menu entries in HKCU (per-user)."""

    REG_PATH = r"Software\Classes\Directory\shell\SmartFileOrganizer"
    REG_BG_PATH = r"Software\Classes\Directory\Background\shell\SmartFileOrganizer"

    @classmethod
    def is_supported(cls) -> bool:
        """Returns True if running on Windows with winreg available."""
        return os.name == "nt"

    @classmethod
    def is_installed(cls) -> bool:
        """Checks if SFO context menu keys currently exist in registry."""
        if not cls.is_supported():
            return False
        import winreg
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, cls.REG_PATH) as _:
                return True
        except FileNotFoundError:
            return False
        except Exception:
            return False

    @classmethod
    def install(cls, python_exe: Optional[str] = None, main_script: Optional[str] = None) -> Dict[str, Any]:
        r"""
        Installs context menu entries under HKCU:
        1. Right-click on any folder (`Directory\shell`)
        2. Right-click on empty background space inside any folder (`Directory\Background\shell`)
        """
        if not cls.is_supported():
            return {"success": False, "message": "Context menu integration is only supported on Windows."}

        import winreg

        py_exe = python_exe or sys.executable
        script = main_script or str(Path(__file__).resolve().parent.parent.parent / "main.py")

        try:
            # 1. Register main folder context menu
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, cls.REG_PATH) as key:
                winreg.SetValueEx(key, "", 0, winreg.REG_SZ, "📂 Smart File Organizer")
                winreg.SetValueEx(key, "SubCommands", 0, winreg.REG_SZ, "")
                winreg.SetValueEx(key, "Icon", 0, winreg.REG_SZ, "shell32.dll,264")

            shell_key_path = f"{cls.REG_PATH}\\shell"

            # Action 1: Smart Organize
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, f"{shell_key_path}\\1_Organize") as sub1:
                winreg.SetValueEx(sub1, "", 0, winreg.REG_SZ, "🧹 Smart Organize Here")
                with winreg.CreateKey(sub1, "command") as cmd1:
                    winreg.SetValueEx(cmd1, "", 0, winreg.REG_SZ, f'"{py_exe}" "{script}" --source "%1"')

            # Action 2: Find Duplicates
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, f"{shell_key_path}\\2_Dedupe") as sub2:
                winreg.SetValueEx(sub2, "", 0, winreg.REG_SZ, "🔍 Find Duplicates Here")
                with winreg.CreateKey(sub2, "command") as cmd2:
                    winreg.SetValueEx(cmd2, "", 0, winreg.REG_SZ, f'"{py_exe}" "{script}" --source "%1" --dedupe')

            # Action 3: Open in GUI
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, f"{shell_key_path}\\3_GUI") as sub3:
                winreg.SetValueEx(sub3, "", 0, winreg.REG_SZ, "🖥️ Open in SFO GUI")
                with winreg.CreateKey(sub3, "command") as cmd3:
                    winreg.SetValueEx(cmd3, "", 0, winreg.REG_SZ, f'"{py_exe}" "{script}" --source "%1" --gui')

            # 2. Register folder background context menu (%V = current directory)
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, cls.REG_BG_PATH) as bg_key:
                winreg.SetValueEx(bg_key, "", 0, winreg.REG_SZ, "📂 Smart File Organizer")
                winreg.SetValueEx(bg_key, "SubCommands", 0, winreg.REG_SZ, "")
                winreg.SetValueEx(bg_key, "Icon", 0, winreg.REG_SZ, "shell32.dll,264")

            bg_shell_path = f"{cls.REG_BG_PATH}\\shell"

            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, f"{bg_shell_path}\\1_Organize") as bg_sub1:
                winreg.SetValueEx(bg_sub1, "", 0, winreg.REG_SZ, "🧹 Smart Organize Here")
                with winreg.CreateKey(bg_sub1, "command") as bg_cmd1:
                    winreg.SetValueEx(bg_cmd1, "", 0, winreg.REG_SZ, f'"{py_exe}" "{script}" --source "%V"')

            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, f"{bg_shell_path}\\2_GUI") as bg_sub2:
                winreg.SetValueEx(bg_sub2, "", 0, winreg.REG_SZ, "🖥️ Open in SFO GUI")
                with winreg.CreateKey(bg_sub2, "command") as bg_cmd2:
                    winreg.SetValueEx(bg_cmd2, "", 0, winreg.REG_SZ, f'"{py_exe}" "{script}" --source "%V" --gui')

            return {
                "success": True,
                "message": "Windows Explorer folder & background context menu successfully installed for current user."
            }
        except Exception as e:
            return {"success": False, "message": f"Failed to register context menu: {str(e)}"}

    @classmethod
    def uninstall(cls) -> Dict[str, Any]:
        """Removes context menu registry keys."""
        if not cls.is_supported():
            return {"success": False, "message": "Only supported on Windows."}

        import winreg

        def delete_key_recursive(root, subkey):
            try:
                with winreg.OpenKey(root, subkey, 0, winreg.KEY_ALL_ACCESS) as key:
                    while True:
                        try:
                            child = winreg.EnumKey(key, 0)
                            delete_key_recursive(root, f"{subkey}\\{child}")
                        except OSError:
                            break
                winreg.DeleteKey(root, subkey)
            except FileNotFoundError:
                pass
            except Exception:
                pass

        try:
            delete_key_recursive(winreg.HKEY_CURRENT_USER, cls.REG_PATH)
            delete_key_recursive(winreg.HKEY_CURRENT_USER, cls.REG_BG_PATH)
            return {"success": True, "message": "Context menu uninstalled successfully."}
        except Exception as e:
            return {"success": False, "message": f"Failed to uninstall context menu: {str(e)}"}
