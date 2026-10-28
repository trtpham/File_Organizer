"""
Sensitive Data & Secret Scanner (DLP).
Enhanced with Shannon Entropy analysis, expanded enterprise token patterns (Slack, Stripe, JWT),
and secure credential redaction.
"""

import os
import re
import math
import shutil
from pathlib import Path
from collections import Counter
from typing import List, Dict, Any, Optional
from datetime import datetime


class SecretFinding:
    """Represents an identified sensitive data leak or secret pattern."""

    def __init__(
        self,
        file_path: Path,
        line_number: int,
        secret_type: str,
        masked_snippet: str,
        severity: str = "HIGH",
        entropy: Optional[float] = None
    ):
        self.file_path = file_path
        self.line_number = line_number
        self.secret_type = secret_type
        self.masked_snippet = masked_snippet
        self.severity = severity  # CRITICAL, HIGH, MEDIUM
        self.entropy = entropy

    def to_dict(self) -> Dict[str, Any]:
        return {
            "file": str(self.file_path),
            "filename": self.file_path.name,
            "line": self.line_number,
            "type": self.secret_type,
            "snippet": self.masked_snippet,
            "severity": self.severity,
            "entropy": round(self.entropy, 2) if self.entropy is not None else None
        }


class SecretScanner:
    """Scans text and code files for credentials, tokens, and personal identifiers."""

    # High-confidence credential patterns
    PATTERNS = [
        ("AWS Access Key", r"\b(AKIA[0-9A-Z]{16})\b", "CRITICAL"),
        ("OpenAI API Key", r"\b(sk-[a-zA-Z0-9T3BlbkFJ]{20,})\b", "CRITICAL"),
        ("GitHub Personal Token", r"\b(ghp_[a-zA-Z0-9]{36})\b", "CRITICAL"),
        ("Slack Token", r"\b(xox[pboa]-[0-9A-Za-z\-]{20,})\b", "CRITICAL"),
        ("Stripe Secret Key", r"\b(sk_live_[0-9a-zA-Z]{24,})\b", "CRITICAL"),
        ("JSON Web Token (JWT)", r"\b(eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b", "HIGH"),
        ("Google Cloud API Key", r"\b(AIza[0-9A-Za-z\-_]{35})\b", "HIGH"),
        ("Private Encryption Key", r"-----BEGIN (?:RSA|OPENSSH|DSA|EC) PRIVATE KEY-----", "CRITICAL"),
        ("Database Connection String", r"(?:postgres|mysql|mongodb|redis):\/\/[a-zA-Z0-9_\-\.]+:[^@\s]+@", "HIGH"),
        ("Vietnam Citizen ID (CCCD)", r"(?i)(?:cccd|cmnd|căn cước)[\s:]*([0-9]{9}|[0-9]{12})\b", "MEDIUM"),
        ("Generic Password Assignment", r'(?i)(?:password|passwd|secret_key)\s*[:=]\s*["\']([^"\']{8,})["\']', "HIGH")
    ]

    SCANNABLE_EXTS = {
        ".env", ".txt", ".json", ".yaml", ".yml", ".py", ".js", ".ts",
        ".csv", ".sql", ".xml", ".ini", ".conf", ".cfg", ".sh", ".pem", ".key"
    }

    @staticmethod
    def calculate_shannon_entropy(text: str) -> float:
        """Calculates Shannon entropy (bits per symbol) to detect randomized secrets."""
        if not text:
            return 0.0
        length = len(text)
        counts = Counter(text)
        entropy = 0.0
        for count in counts.values():
            p = count / length
            entropy -= p * math.log2(p)
        return entropy

    @classmethod
    def scan_file(cls, path: Path) -> List[SecretFinding]:
        """Scans an individual file for sensitive information."""
        findings = []
        is_key_or_env = (
            path.name.startswith(".env") or
            path.name.startswith("id_") or
            path.name in ("credentials", "secrets", "config")
        )
        if path.suffix.lower() not in cls.SCANNABLE_EXTS and not is_key_or_env:
            return findings

        try:
            # Skip massive files (> 5MB) to prevent freezing
            if path.stat().st_size > 5 * 1024 * 1024:
                return findings

            with open(path, "r", encoding="utf-8", errors="ignore") as f:
                for line_no, line in enumerate(f, start=1):
                    for label, regex, severity in cls.PATTERNS:
                        match = re.search(regex, line)
                        if match:
                            raw_val = match.group(0)
                            entropy = cls.calculate_shannon_entropy(raw_val)
                            # Mask middle part of the secret safely
                            if len(raw_val) > 8:
                                masked = raw_val[:4] + "*" * (len(raw_val) - 8) + raw_val[-4:]
                            else:
                                masked = "***REDACTED***"

                            findings.append(SecretFinding(
                                file_path=path,
                                line_number=line_no,
                                secret_type=label,
                                masked_snippet=masked,
                                severity=severity,
                                entropy=entropy
                            ))
                            break
        except Exception:
            pass

        return findings

    @classmethod
    def scan_directory(cls, root_dir: Path, recursive: bool = True) -> List[SecretFinding]:
        """Scans all supported files in root_dir for exposed secrets."""
        all_findings = []
        root_path = Path(root_dir).resolve()

        if recursive:
            for root, _, files in os.walk(root_path):
                for fname in files:
                    file_path = Path(root) / fname
                    findings = cls.scan_file(file_path)
                    if findings:
                        all_findings.extend(findings)
        else:
            try:
                for entry in os.scandir(root_path):
                    if entry.is_file():
                        findings = cls.scan_file(Path(entry.path))
                        if findings:
                            all_findings.extend(findings)
            except OSError:
                pass

        return all_findings

    @classmethod
    def quarantine_files(cls, findings: List[SecretFinding], quarantine_root: Path) -> Dict[str, Any]:
        """Safely quarantines compromised files into an isolated directory."""
        quarantine_dir = Path(quarantine_root) / "_Quarantine"
        quarantine_dir.mkdir(parents=True, exist_ok=True)

        quarantined_files = set(f.file_path for f in findings)
        moved_count = 0
        errors = []

        for src in quarantined_files:
            if not src.exists():
                continue
            dest = quarantine_dir / f"{src.name}.quarantined"
            try:
                shutil.move(str(src), str(dest))
                moved_count += 1
            except Exception as e:
                errors.append(f"Failed to isolate {src.name}: {str(e)}")

        return {
            "success": len(errors) == 0,
            "quarantined_count": moved_count,
            "quarantine_dir": str(quarantine_dir),
            "errors": errors,
            "message": f"Quarantined {moved_count} compromised files to '{quarantine_dir}'."
        }
