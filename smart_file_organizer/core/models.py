"""
Data models and enumerations for Smart File Organizer.
"""

from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import List, Optional, Dict, Any
from datetime import datetime


class ActionType(str, Enum):
    MOVE = "MOVE"
    COPY = "COPY"
    RENAME = "RENAME"
    DELETE = "DELETE"
    SKIP = "SKIP"
    DEDUPLICATE = "DEDUPLICATE"


class CollisionStrategy(str, Enum):
    AUTO_RENAME = "auto_rename"
    SKIP = "skip"
    COMPARE_HASH = "compare_hash"
    OVERWRITE = "overwrite"


class OrganizeStrategy(str, Enum):
    EXTENSION = "extension"
    DATE = "date"
    SIZE = "size"
    REGEX = "regex"


@dataclass
class FileInfo:
    """Represents a discovered file with cached metadata."""
    path: Path
    filename: str
    extension: str
    size_bytes: int
    created_time: datetime
    modified_time: datetime
    partial_hash: Optional[str] = None
    full_hash: Optional[str] = None

    @classmethod
    def from_path(cls, path: Path) -> "FileInfo":
        stat = path.stat()
        created = datetime.fromtimestamp(stat.st_ctime)
        modified = datetime.fromtimestamp(stat.st_mtime)
        return cls(
            path=path.resolve(),
            filename=path.name,
            extension=path.suffix.lower(),
            size_bytes=stat.st_size,
            created_time=created,
            modified_time=modified,
        )

    def to_dict(self) -> Dict[str, Any]:
        return {
            "path": str(self.path),
            "filename": self.filename,
            "extension": self.extension,
            "size_bytes": self.size_bytes,
            "created_time": self.created_time.isoformat(),
            "modified_time": self.modified_time.isoformat(),
            "partial_hash": self.partial_hash,
            "full_hash": self.full_hash,
        }


@dataclass
class ActionRecord:
    """Represents a single planned or executed file operation."""
    action_type: ActionType
    source_path: Path
    destination_path: Optional[Path]
    file_size: int = 0
    file_hash: Optional[str] = None
    status: str = "PENDING"  # PENDING, SUCCESS, SKIPPED, FAILED
    message: str = ""
    timestamp: datetime = field(default_factory=datetime.now)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "action_type": self.action_type.value,
            "source_path": str(self.source_path),
            "destination_path": str(self.destination_path) if self.destination_path else None,
            "file_size": self.file_size,
            "file_hash": self.file_hash,
            "status": self.status,
            "message": self.message,
            "timestamp": self.timestamp.isoformat(),
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ActionRecord":
        return cls(
            action_type=ActionType(data["action_type"]),
            source_path=Path(data["source_path"]),
            destination_path=Path(data["destination_path"]) if data.get("destination_path") else None,
            file_size=data.get("file_size", 0),
            file_hash=data.get("file_hash"),
            status=data.get("status", "SUCCESS"),
            message=data.get("message", ""),
            timestamp=datetime.fromisoformat(data["timestamp"]) if "timestamp" in data else datetime.now(),
        )


@dataclass
class DuplicateGroup:
    """Represents a group of duplicate files sharing identical content hash."""
    file_hash: str
    file_size: int
    files: List[FileInfo]
    suggested_keep: Optional[FileInfo] = None

    @property
    def wasted_space_bytes(self) -> int:
        """Space wasted by duplicates (excluding 1 kept copy)."""
        count = len(self.files)
        if count <= 1:
            return 0
        return (count - 1) * self.file_size


@dataclass
class SessionLog:
    """Complete record of an organization or deduplication session."""
    session_id: str
    start_time: datetime
    end_time: Optional[datetime] = None
    source_dir: str = ""
    destination_dir: str = ""
    strategy: str = "extension"
    dry_run: bool = False
    records: List[ActionRecord] = field(default_factory=list)
    total_files: int = 0
    total_size_bytes: int = 0
    moved_count: int = 0
    skipped_count: int = 0
    duplicate_count: int = 0
    space_saved_bytes: int = 0

    def to_dict(self) -> Dict[str, Any]:
        return {
            "session_id": self.session_id,
            "start_time": self.start_time.isoformat(),
            "end_time": self.end_time.isoformat() if self.end_time else None,
            "source_dir": self.source_dir,
            "destination_dir": self.destination_dir,
            "strategy": self.strategy,
            "dry_run": self.dry_run,
            "total_files": self.total_files,
            "total_size_bytes": self.total_size_bytes,
            "moved_count": self.moved_count,
            "skipped_count": self.skipped_count,
            "duplicate_count": self.duplicate_count,
            "space_saved_bytes": self.space_saved_bytes,
            "records": [r.to_dict() for r in self.records],
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SessionLog":
        records = [ActionRecord.from_dict(r) for r in data.get("records", [])]
        return cls(
            session_id=data["session_id"],
            start_time=datetime.fromisoformat(data["start_time"]),
            end_time=datetime.fromisoformat(data["end_time"]) if data.get("end_time") else None,
            source_dir=data.get("source_dir", ""),
            destination_dir=data.get("destination_dir", ""),
            strategy=data.get("strategy", "extension"),
            dry_run=data.get("dry_run", False),
            records=records,
            total_files=data.get("total_files", 0),
            total_size_bytes=data.get("total_size_bytes", 0),
            moved_count=data.get("moved_count", 0),
            skipped_count=data.get("skipped_count", 0),
            duplicate_count=data.get("duplicate_count", 0),
            space_saved_bytes=data.get("space_saved_bytes", 0),
        )
