"""
Configuration manager for Smart File Organizer.
"""

import json
import fnmatch
from pathlib import Path
from typing import Dict, Any, List, Optional
from .models import CollisionStrategy, OrganizeStrategy


class ConfigManager:
    """Handles loading, updating, and querying organization rules and settings."""

    DEFAULT_CONFIG_PATH = Path(__file__).resolve().parent.parent.parent / "config" / "default_config.json"
    USER_CONFIG_DIR = Path.home() / ".smart_file_organizer"
    USER_CONFIG_PATH = USER_CONFIG_DIR / "config.json"

    def __init__(self, custom_path: Optional[Path] = None):
        self.custom_path = custom_path
        self.data: Dict[str, Any] = {}
        self.reload()

    def reload(self) -> None:
        """Loads configuration in order of priority: Custom -> User Local -> Default."""
        # 1. Load default config
        if self.DEFAULT_CONFIG_PATH.exists():
            with open(self.DEFAULT_CONFIG_PATH, "r", encoding="utf-8") as f:
                self.data = json.load(f)
        else:
            self.data = self._generate_fallback_defaults()

        # 2. Merge user config if exists
        if self.USER_CONFIG_PATH.exists() and not self.custom_path:
            try:
                with open(self.USER_CONFIG_PATH, "r", encoding="utf-8") as f:
                    user_data = json.load(f)
                    self._deep_merge(self.data, user_data)
            except Exception:
                pass

        # 3. Load custom path if explicitly provided
        if self.custom_path and Path(self.custom_path).exists():
            with open(self.custom_path, "r", encoding="utf-8") as f:
                custom_data = json.load(f)
                self._deep_merge(self.data, custom_data)

        # Build reverse lookup index for O(1) extension category lookup
        self._extension_map: Dict[str, str] = {}
        for category, extensions in self.categories.items():
            for ext in extensions:
                self._extension_map[ext.lower()] = category

    def _deep_merge(self, base: Dict[str, Any], override: Dict[str, Any]) -> None:
        """Recursively merges override dictionary into base dictionary."""
        for k, v in override.items():
            if isinstance(v, dict) and k in base and isinstance(base[k], dict):
                self._deep_merge(base[k], v)
            else:
                base[k] = v

    def _generate_fallback_defaults(self) -> Dict[str, Any]:
        """Provides internal defaults if default_config.json file is missing."""
        return {
            "version": "2.0.0",
            "default_organize_strategy": "extension",
            "date_format": "YYYY/MM",
            "date_attribute": "modified",
            "collision_strategy": "auto_rename",
            "clean_empty_folders": False,
            "recursive": False,
            "ignore_patterns": [".git", ".DS_Store", "desktop.ini", "thumbs.db", "*.tmp", "*.crdownload", "*.part"],
            "size_thresholds": {
                "Tiny (< 1MB)": [0, 1048576],
                "Small (1MB - 10MB)": [1048576, 10485760],
                "Medium (10MB - 100MB)": [10485760, 104857600],
                "Large (100MB - 1GB)": [104857600, 1073741824],
                "Huge (> 1GB)": [1073741824, -1]
            },
            "deduplication": {
                "hash_algorithm": "sha256",
                "partial_chunk_bytes": 65536,
                "default_action": "move_to_duplicates",
                "duplicates_folder_name": "_Duplicates",
                "keep_policy": "oldest"
            },
            "watcher": {
                "debounce_seconds": 3.0,
                "check_interval_seconds": 2.0
            },
            "categories": {
                "Images": [".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg"],
                "Documents": [".pdf", ".docx", ".doc", ".txt", ".xlsx", ".pptx"],
                "Archives": [".zip", ".rar", ".7z", ".tar", ".gz"],
                "Audio": [".mp3", ".wav", ".flac", ".m4a"],
                "Videos": [".mp4", ".mkv", ".avi", ".mov"],
                "Executables": [".exe", ".msi", ".bat", ".cmd"],
                "Code": [".py", ".js", ".html", ".css", ".json", ".sql"]
            },
            "custom_regex_rules": []
        }

    @property
    def categories(self) -> Dict[str, List[str]]:
        return self.data.get("categories", {})

    @property
    def ignore_patterns(self) -> List[str]:
        return self.data.get("ignore_patterns", [])

    @property
    def date_format(self) -> str:
        return self.data.get("date_format", "YYYY/MM")

    @property
    def date_attribute(self) -> str:
        return self.data.get("date_attribute", "modified")

    @property
    def collision_strategy(self) -> CollisionStrategy:
        val = self.data.get("collision_strategy", "auto_rename")
        try:
            return CollisionStrategy(val)
        except ValueError:
            return CollisionStrategy.AUTO_RENAME

    @property
    def deduplication_settings(self) -> Dict[str, Any]:
        return self.data.get("deduplication", {})

    @property
    def watcher_settings(self) -> Dict[str, Any]:
        return self.data.get("watcher", {"debounce_seconds": 3.0, "check_interval_seconds": 2.0})

    def get_category_for_extension(self, extension: str) -> str:
        """Finds corresponding category name for a file extension, or 'Others'."""
        ext = extension.lower().strip()
        if not ext.startswith(".") and ext:
            ext = "." + ext
        return self._extension_map.get(ext, "Others")

    def is_ignored(self, filename_or_path: str) -> bool:
        """Checks if a file or directory matches any ignore pattern (fnmatch)."""
        name = Path(filename_or_path).name
        full_str = str(filename_or_path).replace("\\", "/")
        for pattern in self.ignore_patterns:
            if fnmatch.fnmatch(name, pattern) or fnmatch.fnmatch(full_str, f"*/{pattern}/*") or fnmatch.fnmatch(full_str, f"*/{pattern}"):
                return True
        return False

    def get_size_folder(self, size_bytes: int) -> str:
        """Determines size bucket category for a given file size in bytes."""
        thresholds = self.data.get("size_thresholds", {})
        for label, (min_b, max_b) in thresholds.items():
            if max_b == -1:
                if size_bytes >= min_b:
                    return label
            elif min_b <= size_bytes < max_b:
                return label
        return "Others"

    def save_user_config(self) -> Path:
        """Saves current configuration to user local directory."""
        self.USER_CONFIG_DIR.mkdir(parents=True, exist_ok=True)
        with open(self.USER_CONFIG_PATH, "w", encoding="utf-8") as f:
            json.dump(self.data, f, indent=2, ensure_ascii=False)
        return self.USER_CONFIG_PATH
