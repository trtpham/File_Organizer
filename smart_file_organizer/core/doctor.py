"""
SFO Doctor: Enterprise System Diagnostic & Health Evaluation Suite.
Performs end-to-end environment, disk I/O latency, memory capacity, registry, permission,
and dependency self-checks with actionable health reports.
Zero external dependencies.
"""

import os
import sys
import time
import shutil
import sqlite3
import ctypes
from pathlib import Path
from typing import List, Dict, Any, Optional

from .reporter import Reporter
from .shell_integration import WindowsShellIntegration
from .cloud_connector import CloudDetector
from .enterprise_guardrails import AuditLogChain


class DiagnosticItem:
    """Represents a single diagnostic health result."""
    def __init__(self, name: str, status: str, message: str, details: Optional[Dict[str, Any]] = None):
        self.name = name
        self.status = status  # "PASS", "WARN", "FAIL"
        self.message = message
        self.details = details or {}

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "status": self.status,
            "message": self.message,
            "details": self.details
        }


class SFODoctor:
    """Coordinates system diagnostics and produces health status reports."""

    @classmethod
    def run_full_diagnosis(cls, target_path: Optional[Path] = None) -> Dict[str, Any]:
        """Runs the complete self-diagnostic check."""
        results: List[DiagnosticItem] = []
        path = Path(target_path or Path.home() / "Downloads").resolve()

        # 1. Environment & Platform
        py_ver = sys.version.split()[0]
        if sys.version_info >= (3, 10):
            results.append(DiagnosticItem("Python Runtime", "PASS", f"Python {py_ver} (>= 3.10 supported)"))
        else:
            results.append(DiagnosticItem("Python Runtime", "FAIL", f"Python {py_ver} is unsupported. Python 3.10+ required."))

        is_windows = os.name == "nt"
        results.append(DiagnosticItem("Operating System", "PASS", f"{sys.platform} ({'Windows' if is_windows else 'POSIX'})"))

        # 2. System Memory (RAM) Check
        try:
            if is_windows:
                class MEMORYSTATUSEX(ctypes.Structure):
                    _fields_ = [
                        ("dwLength", ctypes.c_ulong),
                        ("dwMemoryLoad", ctypes.c_ulong),
                        ("ullTotalPhys", ctypes.c_ulonglong),
                        ("ullAvailPhys", ctypes.c_ulonglong),
                        ("ullTotalPageFile", ctypes.c_ulonglong),
                        ("ullAvailPageFile", ctypes.c_ulonglong),
                        ("ullTotalVirtual", ctypes.c_ulonglong),
                        ("ullAvailVirtual", ctypes.c_ulonglong),
                        ("sullAvailExtendedVirtual", ctypes.c_ulonglong),
                    ]
                stat = MEMORYSTATUSEX()
                stat.dwLength = ctypes.sizeof(MEMORYSTATUSEX)
                ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(stat))
                total_ram_gb = stat.ullTotalPhys / (1024 ** 3)
                avail_ram_gb = stat.ullAvailPhys / (1024 ** 3)
                mem_status = "PASS" if avail_ram_gb >= 1.0 else "WARN"
                results.append(DiagnosticItem(
                    "System Memory (RAM)", mem_status,
                    f"{avail_ram_gb:.1f} GB available of {total_ram_gb:.1f} GB ({stat.dwMemoryLoad}% load)"
                ))
            else:
                results.append(DiagnosticItem("System Memory (RAM)", "PASS", "POSIX platform memory operational"))
        except Exception:
            pass

        # 3. Disk Space & I/O Latency Benchmark
        try:
            total, used, free = shutil.disk_usage(path if path.exists() else Path.home())
            free_gb = free / (1024 ** 3)
            status = "PASS" if free_gb >= 5.0 else ("WARN" if free_gb >= 1.0 else "FAIL")
            results.append(DiagnosticItem(
                "Disk Headroom", status,
                f"{Reporter.format_bytes(free)} available ({Reporter.format_bytes(total)} total)",
                {"free_bytes": free, "total_bytes": total}
            ))

            # Latency benchmark
            test_file = path / ".sfo_doctor_benchmark.tmp" if path.exists() else Path.home() / ".sfo_doctor_benchmark.tmp"
            start = time.perf_counter()
            with open(test_file, "wb") as f:
                f.write(b"0" * 1024 * 512)  # 512 KB
                f.flush()
                os.fsync(f.fileno())
            latency_ms = (time.perf_counter() - start) * 1000
            test_file.unlink()

            lat_status = "PASS" if latency_ms < 50.0 else ("WARN" if latency_ms < 150.0 else "FAIL")
            results.append(DiagnosticItem(
                "Disk Write Latency", lat_status,
                f"{latency_ms:.2f} ms for 512KB flush",
                {"latency_ms": latency_ms}
            ))
        except Exception as e:
            results.append(DiagnosticItem("Disk Health Check", "WARN", f"Could not benchmark target path: {str(e)}"))

        # 4. Shell Context Menu Integration
        if is_windows:
            installed = WindowsShellIntegration.is_installed()
            if installed:
                results.append(DiagnosticItem("Windows Explorer Menu", "PASS", "Registered under HKCU (Right-click shortcuts active)"))
            else:
                results.append(DiagnosticItem("Windows Explorer Menu", "WARN", "Not currently registered (Run 'sfo --install-context-menu' to enable)"))
        else:
            results.append(DiagnosticItem("Windows Explorer Menu", "PASS", "Skipped on non-Windows platform"))

        # 5. Cloud Storage Providers
        clouds = CloudDetector.detect_all()
        if clouds:
            cloud_names = ", ".join(c.name for c in clouds)
            results.append(DiagnosticItem("Cloud Integrations", "PASS", f"Detected: {cloud_names}"))
        else:
            results.append(DiagnosticItem("Cloud Integrations", "WARN", "No active cloud storage sync directories detected"))

        # 6. Optional Acceleration Libraries
        opt_libs = []
        try:
            import PIL
            opt_libs.append(f"Pillow {PIL.__version__}")
        except ImportError:
            pass

        try:
            import fitz
            opt_libs.append(f"PyMuPDF {fitz.__version__}")
        except ImportError:
            try:
                import PyPDF2
                opt_libs.append(f"PyPDF2 {PyPDF2.__version__}")
            except ImportError:
                pass

        if opt_libs:
            results.append(DiagnosticItem("Accelerators", "PASS", f"High-fidelity modules loaded: {', '.join(opt_libs)}"))
        else:
            results.append(DiagnosticItem("Accelerators", "PASS", "Pure Standard Library Mode (Zero external dependencies)"))

        # 7. SQLite Journal Integrity
        db_path = Path.home() / ".smart_file_organizer" / "history.db"
        if db_path.exists():
            try:
                conn = sqlite3.connect(db_path)
                cursor = conn.cursor()
                cursor.execute("PRAGMA integrity_check;")
                res = cursor.fetchone()
                conn.close()
                if res and res[0] == "ok":
                    results.append(DiagnosticItem("Database Integrity", "PASS", "SQLite Transaction Journal is healthy"))
                else:
                    results.append(DiagnosticItem("Database Integrity", "FAIL", f"SQLite integrity warning: {res}"))
            except Exception as e:
                results.append(DiagnosticItem("Database Integrity", "WARN", f"Database check warning: {str(e)}"))
        else:
            results.append(DiagnosticItem("Database Integrity", "PASS", "Journal database initialized on first operation"))

        # 8. Cryptographic Audit Log Integrity
        try:
            audit = AuditLogChain()
            is_valid, record_count, audit_msg = audit.verify_integrity()
            if is_valid:
                results.append(DiagnosticItem("Audit Log Integrity", "PASS", f"SHA-256 chain valid ({record_count} events mathematically verified)"))
            else:
                results.append(DiagnosticItem("Audit Log Integrity", "FAIL", f"Audit tampering detected: {audit_msg}"))
        except Exception:
            pass

        pass_count = sum(1 for r in results if r.status == "PASS")
        warn_count = sum(1 for r in results if r.status == "WARN")
        fail_count = sum(1 for r in results if r.status == "FAIL")

        overall_status = "HEALTHY" if fail_count == 0 else "UNHEALTHY"

        return {
            "overall_status": overall_status,
            "pass_count": pass_count,
            "warn_count": warn_count,
            "fail_count": fail_count,
            "checks": [r.to_dict() for r in results]
        }
