"""
Enterprise Safety Guardrails & Cryptographic Audit Logger.
Enforces system path protection, blast radius safety caps,
and maintains a tamper-evident SHA-256 hash-chained audit journal.
Zero external dependencies.
"""

import os
import sys
import json
import hashlib
from pathlib import Path
from datetime import datetime, timezone
from typing import List, Dict, Any, Optional, Tuple


class EnterpriseGuardrails:
    """Enforces enterprise safety rules and boundaries."""

    # Default protected system path prefixes
    WINDOWS_PROTECTED_PATHS = [
        Path(os.environ.get("SystemRoot", "C:/Windows")).resolve(),
        Path(os.environ.get("ProgramFiles", "C:/Program Files")).resolve(),
        Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")).resolve(),
        Path("C:/Windows").resolve(),
        Path("C:/System Volume Information").resolve(),
        Path("C:/$Recycle.Bin").resolve(),
    ]

    # Critical user folders that shouldn't be mass-wiped as root
    PROTECTED_ROOT_NAMES = {
        "AppData", "Application Data", "Windows", "System32",
        "SysWOW64", "Recovery", "Boot"
    }

    @classmethod
    def is_protected_path(cls, path: Path) -> Tuple[bool, str]:
        """
        Validates if a path points to a protected system directory or root drive.
        Returns (is_protected, reason).
        """
        try:
            resolved = Path(path).resolve()
        except Exception:
            return True, "Invalid path resolution"

        # 1. Protect root volume drives (e.g. C:\, D:\ directly)
        if len(resolved.parts) <= 1 or resolved.parent == resolved:
            return True, "Cannot target drive root directory directly."

        # 2. Check Windows system directories
        if os.name == "nt":
            for sys_path in cls.WINDOWS_PROTECTED_PATHS:
                try:
                    if resolved == sys_path or sys_path in resolved.parents:
                        return True, f"Target is located within protected system directory: {sys_path}"
                except Exception:
                    continue

        # 3. Check protected folder names in immediate parent hierarchy
        if resolved.name in cls.PROTECTED_ROOT_NAMES:
            return True, f"Directory name '{resolved.name}' is a protected operating system component."

        return False, ""

    @classmethod
    def check_blast_radius(
        cls,
        affected_count: int,
        affected_bytes: int,
        max_files: int = 500,
        max_bytes: int = 10 * 1024 * 1024 * 1024  # 10 GB
    ) -> Tuple[bool, str]:
        """
        Guards against runaway mass-operations.
        Returns (exceeded, warning_message).
        """
        if affected_count > max_files:
            return True, f"Operation impacts {affected_count} files (Safety threshold: {max_files}). Explicit confirmation required."
        if affected_bytes > max_bytes:
            gb = affected_bytes / (1024 * 1024 * 1024)
            max_gb = max_bytes / (1024 * 1024 * 1024)
            return True, f"Operation impacts {gb:.1f} GB of data (Safety threshold: {max_gb:.1f} GB). Explicit confirmation required."
        return False, ""


class AuditLogChain:
    """
    Tamper-evident audit logger using a cryptographic SHA-256 hash chain.
    Every logged entry includes the previous hash, guaranteeing log integrity.
    """

    DEFAULT_AUDIT_FILE = Path.home() / ".smart_file_organizer" / "audit_chain.jsonl"
    GENESIS_HASH = "0000000000000000000000000000000000000000000000000000000000000000"

    def __init__(self, log_path: Optional[Path] = None):
        self.log_path = Path(log_path or self.DEFAULT_AUDIT_FILE).resolve()
        self.log_path.parent.mkdir(parents=True, exist_ok=True)

    def get_latest_hash(self) -> str:
        """Retrieves the cryptographic hash of the most recent audit entry."""
        if not self.log_path.exists() or self.log_path.stat().st_size == 0:
            return self.GENESIS_HASH

        try:
            with open(self.log_path, "r", encoding="utf-8") as f:
                last_line = ""
                for line in f:
                    if line.strip():
                        last_line = line.strip()
                if last_line:
                    record = json.loads(last_line)
                    return record.get("current_hash", self.GENESIS_HASH)
        except Exception:
            pass
        return self.GENESIS_HASH

    def log_event(self, action: str, details: Dict[str, Any], user: Optional[str] = None) -> Dict[str, Any]:
        """Appends a new hashed entry to the tamper-evident audit journal."""
        prev_hash = self.get_latest_hash()
        timestamp = datetime.now(timezone.utc).isoformat()
        operator = user or os.environ.get("USERNAME") or os.environ.get("USER") or "system"

        payload_to_hash = f"{prev_hash}|{timestamp}|{operator}|{action}|{json.dumps(details, sort_keys=True)}"
        current_hash = hashlib.sha256(payload_to_hash.encode("utf-8")).hexdigest()

        entry = {
            "timestamp": timestamp,
            "operator": operator,
            "action": action,
            "details": details,
            "previous_hash": prev_hash,
            "current_hash": current_hash
        }

        with open(self.log_path, "a", encoding="utf-8") as f:
            f.write(json.dumps(entry) + "\n")

        return entry

    def verify_integrity(self) -> Tuple[bool, int, str]:
        """
        Validates the entire hash chain from the beginning.
        Returns (is_valid, total_verified_records, message).
        """
        if not self.log_path.exists() or self.log_path.stat().st_size == 0:
            return True, 0, "Audit log is empty."

        expected_prev_hash = self.GENESIS_HASH
        count = 0

        try:
            with open(self.log_path, "r", encoding="utf-8") as f:
                for line_num, line in enumerate(f, start=1):
                    line = line.strip()
                    if not line:
                        continue

                    record = json.loads(line)
                    prev_hash = record.get("previous_hash", "")
                    current_hash = record.get("current_hash", "")

                    if prev_hash != expected_prev_hash:
                        return False, count, f"Hash chain broken at line {line_num}: previous hash mismatch."

                    # Recalculate hash
                    payload = f"{prev_hash}|{record['timestamp']}|{record['operator']}|{record['action']}|{json.dumps(record['details'], sort_keys=True)}"
                    computed_hash = hashlib.sha256(payload.encode("utf-8")).hexdigest()

                    if computed_hash != current_hash:
                        return False, count, f"Hash chain broken at line {line_num}: entry has been tampered with."

                    expected_prev_hash = current_hash
                    count += 1

            return True, count, f"Audit chain verified clean! All {count} records mathematically validated."
        except Exception as e:
            return False, count, f"Verification failed due to error: {str(e)}"
