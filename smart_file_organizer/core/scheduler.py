"""
Windows Task Scheduler (schtasks) Integration.
Manages automated OS-level background sweeps (Daily Downloads Clean,
Weekly Secret DLP Scan, Monthly Cold Storage Archiving).
Zero external dependencies.
"""

import os
import sys
import subprocess
from pathlib import Path
from typing import List, Dict, Any, Optional


class WindowsTaskScheduler:
    """Manages persistent scheduled tasks in the Windows Task Scheduler."""

    FOLDER_PREFIX = "SmartFileOrganizer"

    @classmethod
    def is_supported(cls) -> bool:
        return os.name == "nt"

    @classmethod
    def create_daily_task(
        cls,
        task_name: str,
        arguments: str,
        start_time: str = "18:00",
        python_exe: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Creates or updates a scheduled daily maintenance task.
        frequency: DAILY, WEEKLY, MONTHLY
        """
        if not cls.is_supported():
            return {"success": False, "message": "Task Scheduler is only supported on Windows."}

        py_exe = python_exe or sys.executable
        main_script = Path(__file__).resolve().parent.parent.parent / "main.py"
        full_command = f'\\"{py_exe}\\" \\"{main_script}\\" {arguments}'
        full_task_name = f"{cls.FOLDER_PREFIX}_{task_name}"

        cmd = [
            "schtasks", "/Create",
            "/TN", full_task_name,
            "/TR", full_command,
            "/SC", "DAILY",
            "/ST", start_time,
            "/F"
        ]

        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
            if res.returncode == 0:
                return {
                    "success": True,
                    "task_name": full_task_name,
                    "message": f"Successfully scheduled daily task '{task_name}' at {start_time}."
                }
            return {
                "success": False,
                "message": f"schtasks error: {res.stderr.strip() or res.stdout.strip()}"
            }
        except Exception as e:
            return {"success": False, "message": f"Failed to invoke schtasks: {str(e)}"}

    @classmethod
    def delete_task(cls, task_name: str) -> Dict[str, Any]:
        """Removes a scheduled task from Windows."""
        if not cls.is_supported():
            return {"success": False, "message": "Only supported on Windows."}

        full_task_name = f"{cls.FOLDER_PREFIX}_{task_name}"
        cmd = ["schtasks", "/Delete", "/TN", full_task_name, "/F"]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
            if res.returncode == 0:
                return {"success": True, "message": f"Task '{task_name}' deleted."}
            return {"success": False, "message": res.stderr.strip() or "Task not found."}
        except Exception as e:
            return {"success": False, "message": str(e)}

    @classmethod
    def list_sfo_tasks(cls) -> List[Dict[str, str]]:
        """Queries all scheduled tasks under the SFO prefix."""
        if not cls.is_supported():
            return []

        cmd = ["schtasks", "/Query", "/FO", "CSV", "/NH"]
        tasks = []
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
            if res.returncode == 0:
                for line in res.stdout.splitlines():
                    if cls.FOLDER_PREFIX in line:
                        parts = [p.strip(' "') for p in line.split('","')]
                        if len(parts) >= 3:
                            tasks.append({
                                "name": parts[0].replace(f"{cls.FOLDER_PREFIX}_", ""),
                                "next_run": parts[1],
                                "status": parts[2]
                            })
        except Exception:
            pass
        return tasks
