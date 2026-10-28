"""
Directory scanner with filtering and metadata extraction.
Enhanced with symlink loop protection, depth limiting, and optimized stat gathering.
"""

import os
from pathlib import Path
from typing import List, Optional, Callable, Set, Tuple
from .models import FileInfo
from .config import ConfigManager


class FileScanner:
    """Scans directories, filters unwanted files, and gathers file metadata."""

    def __init__(self, config: Optional[ConfigManager] = None):
        self.config = config or ConfigManager()

    def scan(
        self,
        source_dir: Path,
        recursive: bool = False,
        exclude_dirs: Optional[List[Path]] = None,
        progress_callback: Optional[Callable[[int, str], None]] = None,
        max_depth: Optional[int] = None
    ) -> List[FileInfo]:
        """
        Scans source directory for files, applying ignore rules.

        Args:
            source_dir: The root directory to scan.
            recursive: If True, scans subdirectories; otherwise only immediate files.
            exclude_dirs: List of directories to completely skip.
            progress_callback: Optional callback receiving (scanned_count, current_filename).
            max_depth: Optional maximum depth limit for recursive scans.
        """
        source_path = Path(source_dir).resolve()
        if not source_path.exists():
            raise FileNotFoundError(f"Source directory does not exist: {source_path}")
        if not source_path.is_dir():
            raise NotADirectoryError(f"Source path is not a directory: {source_path}")

        resolved_excludes = [p.resolve() for p in (exclude_dirs or [])]
        discovered_files: List[FileInfo] = []
        count = 0
        visited_dirs: Set[Tuple[int, int]] = set()

        if recursive:
            source_parts_len = len(source_path.parts)
            for root, dirs, files in os.walk(source_path, followlinks=False):
                current_dir = Path(root).resolve()

                # Symlink loop protection
                try:
                    dir_stat = current_dir.stat()
                    dir_key = (dir_stat.st_dev, dir_stat.st_ino)
                    if dir_key in visited_dirs and dir_stat.st_ino != 0:
                        dirs.clear()
                        continue
                    visited_dirs.add(dir_key)
                except OSError:
                    pass

                # Depth limiting
                if max_depth is not None:
                    depth = len(current_dir.parts) - source_parts_len
                    if depth >= max_depth:
                        dirs.clear()

                # Filter out excluded directories and ignored patterns in-place
                dirs[:] = [
                    d for d in dirs
                    if not self.config.is_ignored(d)
                    and (current_dir / d).resolve() not in resolved_excludes
                ]

                for fname in files:
                    if self.config.is_ignored(fname):
                        continue
                    file_path = current_dir / fname
                    try:
                        file_info = FileInfo.from_path(file_path)
                        discovered_files.append(file_info)
                        count += 1
                        if progress_callback:
                            progress_callback(count, fname)
                    except (PermissionError, FileNotFoundError, OSError):
                        continue
        else:
            try:
                for entry in os.scandir(source_path):
                    try:
                        if entry.is_file(follow_symlinks=False):
                            if self.config.is_ignored(entry.name):
                                continue
                            file_info = FileInfo.from_path(Path(entry.path))
                            discovered_files.append(file_info)
                            count += 1
                            if progress_callback:
                                progress_callback(count, entry.name)
                    except (PermissionError, FileNotFoundError, OSError):
                        continue
            except (PermissionError, OSError):
                pass

        return discovered_files
