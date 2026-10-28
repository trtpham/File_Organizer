"""
Cold Storage & Compression Archiver.
Identifies inactive or old files, compresses them into organized ZIP archives with level 9 compression,
stores internal manifest metadata, performs archive integrity self-testing, and restores original timestamps.
Zero external dependencies.
"""

import os
import time
import json
import zipfile
from pathlib import Path
from typing import List, Dict, Any, Optional, Callable
from datetime import datetime, timezone

from .models import FileInfo


class ColdStorageAnalyzer:
    """Scans and filters files based on inactivity and access/modification age."""

    @staticmethod
    def scan_cold_files(
        source_dir: Path,
        inactive_days: int = 90,
        min_size_bytes: int = 0,
        recursive: bool = True
    ) -> List[FileInfo]:
        """
        Finds files that have not been modified or accessed for >= inactive_days.
        """
        source_dir = Path(source_dir).resolve()
        if not source_dir.exists():
            return []

        now = time.time()
        age_threshold_seconds = inactive_days * 86400
        cold_files: List[FileInfo] = []

        walker = source_dir.rglob("*") if recursive else source_dir.glob("*")
        for path in walker:
            try:
                if not path.is_file():
                    continue
                # Skip hidden and system files
                if path.name.startswith(".") or path.name.startswith("~"):
                    continue

                stat = path.stat()
                # Most recent interaction timestamp (mtime or atime)
                last_interaction = max(stat.st_mtime, getattr(stat, "st_atime", stat.st_mtime))
                file_age = now - last_interaction

                if file_age >= age_threshold_seconds and stat.st_size >= min_size_bytes:
                    cold_files.append(FileInfo.from_path(path))
            except (PermissionError, FileNotFoundError, OSError):
                continue

        cold_files.sort(key=lambda x: x.size_bytes, reverse=True)
        return cold_files


class ColdStorageArchiver:
    """Packages cold files into compressed zip archives with rollback manifests."""

    MANIFEST_NAME = "sfo_archive_manifest.json"

    @classmethod
    def create_cold_archive(
        cls,
        files: List[FileInfo],
        archive_path: Path,
        base_dir: Optional[Path] = None,
        delete_sources: bool = False,
        progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> Dict[str, Any]:
        """
        Compresses a list of files into an archive using maximum Deflate compression (level 9).
        Performs cryptographic CRC / testzip integrity verification before removing original files.
        """
        if not files:
            return {"success": False, "message": "No files specified for archiving.", "archived_count": 0, "bytes_saved": 0}

        archive_path = Path(archive_path).resolve()
        archive_path.parent.mkdir(parents=True, exist_ok=True)

        manifest_entries = []
        total_raw_size = 0
        deleted_files = []
        total = len(files)

        try:
            with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
                for idx, file_info in enumerate(files, start=1):
                    src = file_info.path
                    if not src.exists():
                        continue

                    total_raw_size += file_info.size_bytes
                    if base_dir:
                        try:
                            rel_arcname = src.relative_to(base_dir).as_posix()
                        except ValueError:
                            rel_arcname = src.name
                    else:
                        rel_arcname = src.name

                    zf.write(src, arcname=rel_arcname)
                    manifest_entries.append({
                        "original_path": str(src),
                        "archive_name": rel_arcname,
                        "size": file_info.size_bytes,
                        "mtime": file_info.modified_time.isoformat()
                    })
                    if progress_callback:
                        progress_callback(idx, total)

                # Write manifest inside archive
                manifest_data = {
                    "created_at": datetime.now(timezone.utc).isoformat(),
                    "file_count": len(manifest_entries),
                    "total_uncompressed_bytes": total_raw_size,
                    "files": manifest_entries
                }
                zf.writestr(cls.MANIFEST_NAME, json.dumps(manifest_data, indent=2))

            # Integrity verification before source deletion
            with zipfile.ZipFile(archive_path, mode="r") as zf:
                bad_file = zf.testzip()
                if bad_file is not None:
                    raise RuntimeError(f"Archive verification failed! Corrupted member: {bad_file}")

            compressed_size = archive_path.stat().st_size
            bytes_saved = max(0, total_raw_size - compressed_size)

            # If requested, safely delete originals now that zip is closed & verified
            if delete_sources:
                for item in manifest_entries:
                    try:
                        p = Path(item["original_path"])
                        if p.exists() and p.is_file():
                            p.unlink()
                            deleted_files.append(str(p))
                    except Exception:
                        pass

            return {
                "success": True,
                "archive_path": str(archive_path),
                "archived_count": len(manifest_entries),
                "uncompressed_bytes": total_raw_size,
                "compressed_bytes": compressed_size,
                "bytes_saved": bytes_saved,
                "deleted_sources_count": len(deleted_files),
                "message": f"Successfully archived {len(manifest_entries)} files ({bytes_saved} bytes saved)."
            }

        except Exception as e:
            if archive_path.exists():
                try:
                    archive_path.unlink()
                except OSError:
                    pass
            return {"success": False, "message": f"Archive creation failed: {str(e)}", "archived_count": 0, "bytes_saved": 0}

    @classmethod
    def restore_archive(cls, archive_path: Path, target_dir: Optional[Path] = None) -> Dict[str, Any]:
        """
        Extracts files from an archive back to their original or specified directory,
        preserving original file modification timestamps.
        """
        archive_path = Path(archive_path).resolve()
        if not archive_path.exists():
            return {"success": False, "message": f"Archive not found: {archive_path}"}

        restored_files = []
        try:
            with zipfile.ZipFile(archive_path, mode="r") as zf:
                manifest = None
                if cls.MANIFEST_NAME in zf.namelist():
                    try:
                        manifest = json.loads(zf.read(cls.MANIFEST_NAME).decode("utf-8"))
                    except Exception:
                        pass

                for member in zf.infolist():
                    if member.filename == cls.MANIFEST_NAME:
                        continue

                    entry = None
                    if manifest:
                        entry = next((f for f in manifest.get("files", []) if f["archive_name"] == member.filename), None)

                    if target_dir:
                        dest = target_dir / member.filename
                    elif entry:
                        dest = Path(entry["original_path"])
                    else:
                        dest = archive_path.parent / member.filename

                    dest.parent.mkdir(parents=True, exist_ok=True)
                    with zf.open(member) as source, open(dest, "wb") as target:
                        target.write(source.read())

                    # Restore original timestamp if recorded
                    if entry and entry.get("mtime"):
                        try:
                            dt = datetime.fromisoformat(entry["mtime"])
                            ts = dt.timestamp()
                            os.utime(dest, (ts, ts))
                        except Exception:
                            pass

                    restored_files.append(str(dest))

            return {
                "success": True,
                "restored_count": len(restored_files),
                "message": f"Successfully restored {len(restored_files)} files from {archive_path.name}."
            }
        except Exception as e:
            return {"success": False, "message": f"Failed to restore archive: {str(e)}"}
