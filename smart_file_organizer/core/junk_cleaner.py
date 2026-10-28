"""
Developer and Application Junk Sweeper.
Discovers and cleans heavy inactive node_modules, .venv, build caches, and temp files.
Enhanced with modern dev ecosystems (Rust, Next.js, Flutter, Turborepo, Gradle).
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

    # Common junk directory names and descriptions
    TARGET_DIRS = {
        "node_modules": (JunkCategory.NODE_MODULES, "JavaScript dependency package tree"),
        ".venv": (JunkCategory.VIRTUAL_ENV, "Python isolated virtual environment"),
        "venv": (JunkCategory.VIRTUAL_ENV, "Python isolated virtual environment"),
        "__pycache__": (JunkCategory.PYTHON_CACHE, "Compiled Python .pyc files"),
        ".pytest_cache": (JunkCategory.PYTHON_CACHE, "Pytest test run cache"),
        ".mypy_cache": (JunkCategory.PYTHON_CACHE, "Mypy type checker cache"),
        "target": (JunkCategory.BUILD_ARTIFACTS, "Rust/Java compiled binary build artifacts"),
        ".gradle": (JunkCategory.BUILD_ARTIFACTS, "Gradle build system cache"),
        ".dart_tool": (JunkCategory.BUILD_ARTIFACTS, "Dart/Flutter build cache"),
        ".next": (JunkCategory.BUILD_ARTIFACTS, "Next.js build artifacts"),
        ".nuxt": (JunkCategory.BUILD_ARTIFACTS, "Nuxt.js build artifacts"),
        ".turbo": (JunkCategory.BUILD_ARTIFACTS, "Turborepo cache"),
        "DerivedData": (JunkCategory.BUILD_ARTIFACTS, "Xcode compiled intermediates")
    }

    @classmethod
    def scan_directory(cls, root_dir: Path, inactive_days: int = 14) -> List[JunkItem]:
        """Scans for developer junk directories older than inactive_days."""
        results: List[JunkItem] = []
        root_path = Path(root_dir).resolve()
        cutoff = datetime.now() - timedelta(days=inactive_days)

        try:
            for root, dirs, _ in os.walk(root_path):
                # Never touch git directories
                if ".git" in dirs:
                    dirs.remove(".git")

                # Check each subdirectory
                for d in list(dirs):
                    if d in cls.TARGET_DIRS:
                        junk_dir = Path(root) / d
                        try:
                            stat = junk_dir.stat()
                            mtime = datetime.fromtimestamp(stat.st_mtime)

                            # If inactive beyond cutoff or bytecode cache
                            if mtime < cutoff or d in ("__pycache__", ".pytest_cache", ".mypy_cache"):
                                size = cls._calculate_folder_size(junk_dir)
                                cat, desc = cls.TARGET_DIRS[d]
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
                            results.append(JunkItem(
                                name=entry.name,
                                path=entry_path,
                                category=JunkCategory.SYSTEM_TEMP,
                                size_bytes=size,
                                last_modified=mtime,
                                description="Temporary system file"
                            ))
                        elif entry.is_dir(follow_symlinks=False):
                            size = cls._calculate_folder_size(entry_path)
                            results.append(JunkItem(
                                name=entry.name,
                                path=entry_path,
                                category=JunkCategory.SYSTEM_TEMP,
                                size_bytes=size,
                                last_modified=mtime,
                                description="Temporary system directory"
                            ))
                except (PermissionError, FileNotFoundError, OSError):
                    pass
        except (PermissionError, OSError):
            pass

        return results

    @classmethod
    def clean_junk(cls, items: List[JunkItem], dry_run: bool = False) -> Dict[str, Any]:
        """Safely removes discovered junk files and directories."""
        cleaned_count = 0
        reclaimed_bytes = 0
        errors: List[str] = []

        for item in items:
            target = item.path
            if not target.exists():
                continue

            reclaimed_bytes += item.size_bytes
            cleaned_count += 1

            if not dry_run:
                try:
                    if target.is_dir():
                        shutil.rmtree(target, ignore_errors=False)
                    else:
                        target.unlink()
                except Exception as e:
                    errors.append(f"Failed to remove {target.name}: {str(e)}")

        return {
            "success": len(errors) == 0,
            "cleaned_count": cleaned_count,
            "reclaimed_bytes": reclaimed_bytes,
            "dry_run": dry_run,
            "errors": errors,
            "message": f"{'[DRY RUN] Would reclaim' if dry_run else 'Reclaimed'} {reclaimed_bytes / (1024*1024):.1f} MB across {cleaned_count} items."
        }

    # Alias for backward compatibility
    clean_items = clean_junk

    @staticmethod
    def _calculate_folder_size(folder: Path) -> int:
        """Fast calculation of folder size in bytes."""
        total = 0
        try:
            for root, _, files in os.walk(folder):
                for f in files:
                    try:
                        total += (Path(root) / f).stat().st_size
                    except (PermissionError, OSError):
                        pass
        except Exception:
            pass
        return total
