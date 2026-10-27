"""
Developer and Application Junk Sweeper.
Discovers and cleans heavy inactive node_modules, .venv, build caches, and temp files.
"""

import os
import shutil
import tempfile
from pathlib import Path
from typing import List, Dict, Any, Optional
from datetime import datetime, timedelta


class JunkCategory:
    NODE_MODULES = "Node.js Modules"
    VIRTUAL_ENV = "Python Virtualenvs"
    PYTHON_CACHE = "Python Bytecode Cache"
    BUILD_ARTIFACTS = "Build Artifacts (Target/Dist)"
    SYSTEM_TEMP = "System Temp Files"


class JunkItem:
    """Represents a discovered disposable developer or temporary artifact."""

    def __init__(
        self,
        name: str,
        path: Path,
        category: str,
        size_bytes: int,
        last_modified: datetime,
        description: str
    ):
        self.name = name
        self.path = path
        self.category = category
        self.size_bytes = size_bytes
        self.last_modified = last_modified
        self.description = description

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "path": str(self.path),
            "category": self.category,
            "size_bytes": self.size_bytes,
            "last_modified": self.last_modified.isoformat(),
            "description": self.description,
        }


class DevJunkCleaner:
    """Scans and cleans heavy development caches and temporary bloat."""

    @classmethod
    def scan_directory(cls, root_dir: Path, inactive_days: int = 14) -> List[JunkItem]:
        """Scans for developer junk directories older than inactive_days."""
        results: List[JunkItem] = []
        root_path = Path(root_dir).resolve()
        cutoff = datetime.now() - timedelta(days=inactive_days)

        # Common junk directory names
        target_dirs = {
            "node_modules": (JunkCategory.NODE_MODULES, "JavaScript dependency package tree"),
            ".venv": (JunkCategory.VIRTUAL_ENV, "Python isolated virtual environment"),
            "venv": (JunkCategory.VIRTUAL_ENV, "Python isolated virtual environment"),
            "__pycache__": (JunkCategory.PYTHON_CACHE, "Compiled Python .pyc files"),
            ".pytest_cache": (JunkCategory.PYTHON_CACHE, "Pytest test run cache"),
            ".mypy_cache": (JunkCategory.PYTHON_CACHE, "Mypy type checker cache"),
            "target": (JunkCategory.BUILD_ARTIFACTS, "Rust/Java compiled binary build artifacts"),
            ".gradle": (JunkCategory.BUILD_ARTIFACTS, "Gradle build system cache")
        }

        try:
            for root, dirs, _ in os.walk(root_path):
                # Check each subdirectory
                for d in list(dirs):
                    if d in target_dirs:
                        junk_dir = Path(root) / d
                        try:
                            stat = junk_dir.stat()
                            mtime = datetime.fromtimestamp(stat.st_mtime)

                            # If inactive beyond cutoff or bytecode cache
                            if mtime < cutoff or d in ("__pycache__", ".pytest_cache"):
                                size = cls._calculate_folder_size(junk_dir)
                                cat, desc = target_dirs[d]
                                results.append(JunkItem(
                                    name=d,
                                    path=junk_dir,
                                    category=cat,
                                    size_bytes=size,
                                    last_modified=mtime,
                                    description=desc
                                ))
                                # Don't recurse into discovered junk folders
                                dirs.remove(d)
                        except (PermissionError, FileNotFoundError, OSError):
                            pass
        except (PermissionError, OSError):
            pass

        return results

    @classmethod
    def scan_system_temp(cls) -> List[JunkItem]:
        """Scans the operating system temp directory for disposable files."""
        results: List[JunkItem] = []
        temp_dir = Path(tempfile.gettempdir())
        cutoff = datetime.now() - timedelta(days=3)

        try:
            for entry in os.scandir(temp_dir):
                entry_path = Path(entry.path)
                try:
                    stat = entry.stat()
                    mtime = datetime.fromtimestamp(stat.st_mtime)
                    if mtime < cutoff:
                        if entry.is_file():
                            size = stat.st_size
                        else:
                            size = cls._calculate_folder_size(entry_path)

                        if size > 1024 * 1024:  # Only report items > 1MB
                            results.append(JunkItem(
                                name=entry.name,
                                path=entry_path,
                                category=JunkCategory.SYSTEM_TEMP,
                                size_bytes=size,
                                last_modified=mtime,
                                description="Stale operating system temporary file/folder"
                            ))
                except (PermissionError, FileNotFoundError, OSError):
                    continue
        except (PermissionError, OSError):
            pass

        return results

    @staticmethod
    def clean_junk(items: List[JunkItem], dry_run: bool = False) -> Dict[str, Any]:
        """Removes the specified junk items and reclaims storage space."""
        cleaned_count = 0
        freed_bytes = 0
        errors: List[str] = []

        for item in items:
            if not item.path.exists():
                continue

            if dry_run:
                cleaned_count += 1
                freed_bytes += item.size_bytes
            else:
                try:
                    if item.path.is_file():
                        item.path.unlink()
                    else:
                        shutil.rmtree(item.path)
                    cleaned_count += 1
                    freed_bytes += item.size_bytes
                except Exception as e:
                    errors.append(f"Cannot delete {item.path.name}: {str(e)}")

        return {
            "success": len(errors) == 0,
            "cleaned_count": cleaned_count,
            "freed_bytes": freed_bytes,
            "errors": errors,
            "message": f"Cleaned {cleaned_count} items, freeing {freed_bytes / (1024*1024):.1f} MB disk space."
        }

    @staticmethod
    def _calculate_folder_size(path: Path) -> int:
        total = 0
        for root, _, files in os.walk(path):
            for f in files:
                try:
                    total += (Path(root) / f).stat().st_size
                except OSError:
                    pass
        return total
