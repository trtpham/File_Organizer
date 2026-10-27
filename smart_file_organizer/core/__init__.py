"""
Core engine modules for Smart File Organizer.
"""

from .models import (
    FileInfo, ActionType, ActionRecord, SessionLog,
    CollisionStrategy, OrganizeStrategy, DuplicateGroup,
    VisualDuplicateGroup, SyncMode, SyncProfile, ReceiptMetadata
)
from .config import ConfigManager
from .scanner import FileScanner
from .hasher import TieredHasher
from .deduplicator import Deduplicator
from .organizer import Organizer
from .executor import TransactionExecutor
from .undo_manager import UndoManager
from .watcher import DirectoryWatcher
from .reporter import Reporter
from .renamer import SmartBulkRenamer, EXIFExtractor, ID3Extractor
from .visual_hasher import PerceptualHasher, VisualSimilarityEngine
from .syncer import DriveDetector, FolderSynchronizer
from .smart_filter import ReceiptClassifier

__all__ = [
    "FileInfo",
    "ActionType",
    "ActionRecord",
    "SessionLog",
    "CollisionStrategy",
    "OrganizeStrategy",
    "DuplicateGroup",
    "VisualDuplicateGroup",
    "SyncMode",
    "SyncProfile",
    "ReceiptMetadata",
    "ConfigManager",
    "FileScanner",
    "TieredHasher",
    "Deduplicator",
    "Organizer",
    "TransactionExecutor",
    "UndoManager",
    "DirectoryWatcher",
    "Reporter",
    "SmartBulkRenamer",
    "EXIFExtractor",
    "ID3Extractor",
    "PerceptualHasher",
    "VisualSimilarityEngine",
    "DriveDetector",
    "FolderSynchronizer",
    "ReceiptClassifier",
]
