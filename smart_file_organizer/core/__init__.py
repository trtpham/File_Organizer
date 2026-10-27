"""
Core engine modules for Smart File Organizer (Enterprise Edition v2.2).
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
from .shell_integration import WindowsShellIntegration
from .treemap import DiskAnalyzer, DiskNode
from .junk_cleaner import DevJunkCleaner, JunkItem, JunkCategory
from .secret_scanner import SecretScanner, SecretFinding
from .archiver import ColdStorageAnalyzer, ColdStorageArchiver
from .cloud_connector import CloudProvider, CloudDetector, CloudSyncManager
from .lan_share import LANShareServer, LANShareHandler, get_local_ip

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
    "WindowsShellIntegration",
    "DiskAnalyzer",
    "DiskNode",
    "DevJunkCleaner",
    "JunkItem",
    "JunkCategory",
    "SecretScanner",
    "SecretFinding",
    "ColdStorageAnalyzer",
    "ColdStorageArchiver",
    "CloudProvider",
    "CloudDetector",
    "CloudSyncManager",
    "LANShareServer",
    "LANShareHandler",
    "get_local_ip",
]
