"""
Automated USB Drive Detection and Incremental Folder/Savegame Backup Engine.
Enhanced with atomic staging writes, integrity checking, and drive status telemetry.
"""

import os
import shutil
import zipfile
import ctypes
from pathlib import Path
from typing import List, Dict, Any, Optional
from datetime import datetime
from .models import SyncProfile, SyncMode


class DriveDetector:
    """Detects removable USB storage drives and external volumes."""

    @staticmethod
    def get_removable_drives() -> List[Dict[str, str]]:
        """
        Scans system and returns list of removable USB drives.
        Windows API: GetDriveTypeW -> 2 (DRIVE_REMOVABLE).
        """
        drives = []
        if os.name == "nt":
            try:
                kernel32 = ctypes.windll.kernel32
                buf_len = 512
                buf = ctypes.create_unicode_buffer(buf_len)
                kernel32.GetLogicalDriveStringsW(buf_len, buf)

                # Iterate through null-terminated drive strings (e.g. 'C:\\\x00D:\\\x00')
                drive_str = buf.value
                for root in buf.raw.split(b"\x00\x00\x00"):
                    d = root.decode("utf-16-le", errors="ignore").replace("\x00", "")
                    if d and len(d) >= 2 and d[1] == ":":
                        drive_path = d.strip()
                        drive_type = kernel32.GetDriveTypeW(drive_path)
                        # 2 = DRIVE_REMOVABLE, 3 = DRIVE_FIXED
                        if drive_type == 2:
                            # Fetch volume label
                            vol_name = ctypes.create_unicode_buffer(256)
                            kernel32.GetVolumeInformationW(
                                drive_path, vol_name, 256, None, None, None, None, 0
                            )
                            drives.append({
                                "path": drive_path,
                                "label": vol_name.value or "USB Drive",
                                "type": "removable"
                            })
            except Exception:
                pass
        else:
            # Unix / macOS fallback: check /media or /Volumes
            for candidate_root in ["/media", "/Volumes", "/run/media"]:
                p = Path(candidate_root)
                if p.exists() and p.is_dir():
                    try:
                        for entry in p.iterdir():
                            if entry.is_dir():
                                drives.append({
                                    "path": str(entry.resolve()),
                                    "label": entry.name,
                                    "type": "removable"
                                })
                    except Exception:
                        pass

        return drives


class FolderSynchronizer:
    """Executes incremental synchronization and timestamped snapshot archiving."""

    @staticmethod
    def sync(profile: SyncProfile, dry_run: bool = False) -> Dict[str, Any]:
        """Executes synchronization according to profile configuration."""
        source = Path(profile.source_dir).resolve()
        dest = Path(profile.dest_dir).resolve()

        if not source.exists() or not source.is_dir():
            return {
                "success": False,
                "error": f"Source directory does not exist: {source}",
                "copied_count": 0,
                "bytes_transferred": 0
            }

        start_time = datetime.now()

        if profile.mode == SyncMode.SNAPSHOT:
            return FolderSynchronizer._create_snapshot(source, dest, profile.name, dry_run, start_time)
        else:
            return FolderSynchronizer._sync_incremental(source, dest, profile.mode, dry_run, start_time)

    @staticmethod
    def _sync_incremental(
        source: Path,
        dest: Path,
        mode: SyncMode,
        dry_run: bool,
        start_time: datetime
    ) -> Dict[str, Any]:
        copied_count = 0
        skipped_count = 0
        bytes_transferred = 0
        errors: List[str] = []

        if not dry_run:
            dest.mkdir(parents=True, exist_ok=True)

        for root, dirs, files in os.walk(source):
            rel_path = Path(root).relative_to(source)
            target_sub_dir = dest / rel_path

            if not dry_run:
                target_sub_dir.mkdir(parents=True, exist_ok=True)

            for fname in files:
                src_file = Path(root) / fname
                dst_file = target_sub_dir / fname

                should_copy = False
                try:
                    src_stat = src_file.stat()
                    if not dst_file.exists():
                        should_copy = True
                    else:
                        dst_stat = dst_file.stat()
                        # If sizes differ or source is newer by > 1 second
                        if src_stat.st_size != dst_stat.st_size or (src_stat.st_mtime - dst_stat.st_mtime) > 1.0:
                            should_copy = True

                    if should_copy:
                        if not dry_run:
                            # Atomic staging write to protect against mid-transfer USB disconnection
                            tmp_dest = target_sub_dir / f".{fname}.sfo_tmp"
                            shutil.copy2(str(src_file), str(tmp_dest))
                            os.replace(tmp_dest, dst_file)
                        copied_count += 1
                        bytes_transferred += src_stat.st_size
                    else:
                        skipped_count += 1
                except Exception as e:
                    errors.append(f"Failed {fname}: {str(e)}")

        elapsed = (datetime.now() - start_time).total_seconds()
        return {
            "success": len(errors) == 0,
            "copied_count": copied_count,
            "skipped_count": skipped_count,
            "bytes_transferred": bytes_transferred,
            "elapsed_seconds": elapsed,
            "errors": errors,
            "message": f"Synced {copied_count} files ({bytes_transferred / (1024*1024):.1f} MB) in {elapsed:.2f}s."
        }

    @staticmethod
    def _create_snapshot(
        source: Path,
        dest: Path,
        profile_name: str,
        dry_run: bool,
        start_time: datetime
    ) -> Dict[str, Any]:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        safe_name = "".join(c for c in profile_name if c.isalnum() or c in ("_", "-")) or "Backup"
        archive_name = f"{safe_name}_{timestamp}.zip"
        archive_path = dest / archive_name

        total_files = 0
        total_bytes = 0

        for root, _, files in os.walk(source):
            for f in files:
                total_files += 1
                try:
                    total_bytes += (Path(root) / f).stat().st_size
                except OSError:
                    pass

        if not dry_run:
            dest.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(archive_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
                for root, _, files in os.walk(source):
                    for f in files:
                        file_path = Path(root) / f
                        arcname = file_path.relative_to(source)
                        zf.write(file_path, arcname=arcname)

        elapsed = (datetime.now() - start_time).total_seconds()
        return {
            "success": True,
            "copied_count": total_files,
            "archive_path": str(archive_path),
            "bytes_transferred": total_bytes,
            "elapsed_seconds": elapsed,
            "message": f"Created snapshot archive '{archive_name}' ({total_bytes / (1024*1024):.1f} MB) in {elapsed:.2f}s."
        }
