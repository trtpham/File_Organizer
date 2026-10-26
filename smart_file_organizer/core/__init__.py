"""
Core engine modules for Smart File Organizer.
"""

from .models import FileInfo, ActionType, ActionRecord, SessionLog, CollisionStrategy, OrganizeStrategy
from .config import ConfigManager
from .scanner import FileScanner
from .hasher import TieredHasher
from .deduplicator import Deduplicator
from .organizer import Organizer
from .executor import TransactionExecutor
from .undo_manager import UndoManager
from .watcher import DirectoryWatcher
from .reporter import Reporter

__all__ = [
    "FileInfo",
    "ActionType",
    "ActionRecord",
    "SessionLog",
    "CollisionStrategy",
    "OrganizeStrategy",
    "ConfigManager",
    "FileScanner",
    "TieredHasher",
    "Deduplicator",
    "Organizer",
    "TransactionExecutor",
    "UndoManager",
    "DirectoryWatcher",
    "Reporter",
]
