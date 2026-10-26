"""
High-performance tiered file hasher (Partial -> Full Cryptographic).
"""

import hashlib
from pathlib import Path
from typing import Optional, Callable
from .models import FileInfo


class TieredHasher:
    """Computes fast partial and complete file hashes for duplicate detection."""

    DEFAULT_PARTIAL_BYTES = 65536  # 64 KB
    BUFFER_SIZE = 1048576           # 1 MB chunk for full hashing

    @classmethod
    def compute_partial_hash(cls, path: Path, chunk_bytes: int = DEFAULT_PARTIAL_BYTES) -> Optional[str]:
        """Reads only first N bytes of a file and returns MD5/SHA-256 hex digest."""
        hasher = hashlib.sha256()
        try:
            with open(path, "rb") as f:
                data = f.read(chunk_bytes)
                if not data:
                    return "empty_file"
                hasher.update(data)
            return hasher.hexdigest()
        except (PermissionError, FileNotFoundError, OSError):
            return None

    @classmethod
    def compute_full_hash(
        cls,
        path: Path,
        algorithm: str = "sha256",
        progress_callback: Optional[Callable[[int], None]] = None
    ) -> Optional[str]:
        """Reads complete file in chunks and computes full cryptographic digest."""
        if algorithm == "md5":
            hasher = hashlib.md5()
        else:
            hasher = hashlib.sha256()

        try:
            with open(path, "rb") as f:
                bytes_read = 0
                while True:
                    chunk = f.read(cls.BUFFER_SIZE)
                    if not chunk:
                        break
                    hasher.update(chunk)
                    bytes_read += len(chunk)
                    if progress_callback:
                        progress_callback(bytes_read)
            return hasher.hexdigest()
        except (PermissionError, FileNotFoundError, OSError):
            return None

    @classmethod
    def populate_hashes(cls, file_info: FileInfo, full: bool = False) -> None:
        """Populates partial and optionally full hash directly onto FileInfo."""
        if not file_info.partial_hash:
            file_info.partial_hash = cls.compute_partial_hash(file_info.path)
        if full and not file_info.full_hash:
            file_info.full_hash = cls.compute_full_hash(file_info.path)
