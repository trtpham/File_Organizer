"""
Cloud Storage Connector & Synchronization Router.
Auto-detects OneDrive, Google Drive, Dropbox, iCloud Drive, Nextcloud, and Box across Windows, macOS, and Linux.
Enables direct routing of organized files into cloud-synced folders and detects sync conflicts.
Zero external dependencies.
"""

import os
import json
from pathlib import Path
from typing import List, Dict, Any, Optional

from .syncer import FolderSynchronizer
from .models import SyncProfile, SyncMode


class CloudProvider:
    """Represents an active local cloud storage folder."""
    def __init__(self, name: str, local_path: Path, is_active: bool, provider_type: str):
        self.name = name
        self.local_path = local_path
        self.is_active = is_active
        self.provider_type = provider_type

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "path": str(self.local_path),
            "is_active": self.is_active,
            "provider_type": self.provider_type
        }


class CloudDetector:
    """Discovers installed and active cloud storage directories across platforms."""

    @classmethod
    def detect_all(cls) -> List[CloudProvider]:
        """Scans system for active cloud storage providers."""
        providers: List[CloudProvider] = []

        # 1. OneDrive
        onedrive = cls.detect_onedrive()
        if onedrive:
            providers.append(onedrive)

        # 2. Google Drive
        gdrive = cls.detect_google_drive()
        if gdrive:
            providers.append(gdrive)

        # 3. Dropbox
        dropbox = cls.detect_dropbox()
        if dropbox:
            providers.append(dropbox)

        # 4. iCloud Drive
        icloud = cls.detect_icloud()
        if icloud:
            providers.append(icloud)

        # 5. Nextcloud / ownCloud
        nextcloud = cls.detect_nextcloud()
        if nextcloud:
            providers.append(nextcloud)

        # 6. Box
        box = cls.detect_box()
        if box:
            providers.append(box)

        return providers

    @classmethod
    def detect_onedrive(cls) -> Optional[CloudProvider]:
        """Detects Microsoft OneDrive directory."""
        candidates = []

        # Environment variables
        for var in ["OneDriveConsumer", "OneDrive", "OneDriveCommercial"]:
            env_val = os.environ.get(var)
            if env_val and Path(env_val).is_dir():
                candidates.append(Path(env_val))

        # Windows registry lookup
        if os.name == "nt":
            try:
                import winreg
                reg_paths = [
                    (winreg.HKEY_CURRENT_USER, r"Software\Microsoft\OneDrive\Accounts\Personal", "UserFolder"),
                    (winreg.HKEY_CURRENT_USER, r"Software\Microsoft\OneDrive", "UserFolder"),
                ]
                for root, key_path, val_name in reg_paths:
                    try:
                        with winreg.OpenKey(root, key_path) as k:
                            val, _ = winreg.QueryValueEx(k, val_name)
                            if val and Path(val).is_dir():
                                candidates.append(Path(val))
                    except Exception:
                        pass
            except ImportError:
                pass

        # Standard fallback path
        fallback = Path.home() / "OneDrive"
        if fallback.is_dir():
            candidates.append(fallback)

        if candidates:
            return CloudProvider("OneDrive", candidates[0], True, "Microsoft OneDrive")
        return None

    @classmethod
    def detect_google_drive(cls) -> Optional[CloudProvider]:
        """Detects Google Drive directory."""
        candidates = []

        # Windows drive letters (Google Drive Desktop mounts as virtual drive or folder)
        if os.name == "nt":
            for letter in ["G", "H", "I", "J", "F"]:
                drive_root = Path(f"{letter}:/")
                my_drive = drive_root / "My Drive"
                if my_drive.is_dir():
                    candidates.append(my_drive)
                elif drive_root.is_dir() and letter not in ("C", "D"):
                    candidates.append(drive_root)

        # Fallback paths
        fallbacks = [
            Path.home() / "Google Drive" / "My Drive",
            Path.home() / "Google Drive",
            Path.home() / "My Drive"
        ]
        for fb in fallbacks:
            if fb.is_dir():
                candidates.append(fb)

        if candidates:
            return CloudProvider("Google Drive", candidates[0], True, "Google Drive")
        return None

    @classmethod
    def detect_dropbox(cls) -> Optional[CloudProvider]:
        """Detects Dropbox directory."""
        candidates = []

        info_json_paths = []
        if os.name == "nt":
            local_appdata = os.environ.get("LOCALAPPDATA")
            appdata = os.environ.get("APPDATA")
            if local_appdata:
                info_json_paths.append(Path(local_appdata) / "Dropbox" / "info.json")
            if appdata:
                info_json_paths.append(Path(appdata) / "Dropbox" / "info.json")
        else:
            info_json_paths.append(Path.home() / ".dropbox" / "info.json")

        for p in info_json_paths:
            if p.is_file():
                try:
                    with open(p, "r", encoding="utf-8") as f:
                        data = json.load(f)
                        for key in ["personal", "business"]:
                            path_str = data.get(key, {}).get("path")
                            if path_str and Path(path_str).is_dir():
                                candidates.append(Path(path_str))
                except Exception:
                    pass

        fallback = Path.home() / "Dropbox"
        if fallback.is_dir():
            candidates.append(fallback)

        if candidates:
            return CloudProvider("Dropbox", candidates[0], True, "Dropbox")
        return None

    @classmethod
    def detect_icloud(cls) -> Optional[CloudProvider]:
        """Detects Apple iCloud Drive directory."""
        candidates = []

        # Windows iCloud path
        user_profile = os.environ.get("USERPROFILE")
        if user_profile:
            win_path = Path(user_profile) / "iCloudDrive"
            if win_path.is_dir():
                candidates.append(win_path)

        # macOS iCloud path
        mac_path = Path.home() / "Library" / "Mobile Documents" / "com~apple~CloudDocs"
        if mac_path.is_dir():
            candidates.append(mac_path)

        if candidates:
            return CloudProvider("iCloud Drive", candidates[0], True, "Apple iCloud")
        return None

    @classmethod
    def detect_nextcloud(cls) -> Optional[CloudProvider]:
        """Detects Nextcloud / ownCloud sync folders."""
        candidates = [
            Path.home() / "Nextcloud",
            Path.home() / "ownCloud"
        ]
        for c in candidates:
            if c.is_dir():
                return CloudProvider("Nextcloud", c, True, "Nextcloud")
        return None

    @classmethod
    def detect_box(cls) -> Optional[CloudProvider]:
        """Detects Box Sync directory."""
        candidates = [
            Path.home() / "Box",
            Path.home() / "Box Sync"
        ]
        for c in candidates:
            if c.is_dir():
                return CloudProvider("Box", c, True, "Box")
        return None


class CloudSyncManager:
    """Coordinates backup and routing to detected cloud storage providers."""

    @classmethod
    def sync_to_cloud(
        cls,
        source_dir: Path,
        provider: CloudProvider,
        subfolder_name: str = "SFO_Cloud_Sync",
        mode: SyncMode = SyncMode.INCREMENTAL,
        dry_run: bool = False
    ) -> Dict[str, Any]:
        """Synchronizes source directory into target cloud provider's directory."""
        source_dir = Path(source_dir).resolve()
        if not source_dir.exists():
            return {"success": False, "message": f"Source directory does not exist: {source_dir}"}

        target_dir = provider.local_path / subfolder_name
        target_dir.mkdir(parents=True, exist_ok=True)

        profile = SyncProfile(
            name=f"Cloud Sync ({provider.name})",
            source_dir=source_dir,
            dest_dir=target_dir,
            mode=mode
        )

        return FolderSynchronizer.sync(profile, dry_run=dry_run)

    @staticmethod
    def find_sync_conflicts(cloud_folder: Path) -> List[Path]:
        """Identifies files with sync collision artifacts (e.g. conflicted copy, .sync-conflict)."""
        conflicts = []
        try:
            for root, _, files in os.walk(cloud_folder):
                for f in files:
                    if any(term in f.lower() for term in [".sync-conflict", "conflicted copy", ".lock"]):
                        conflicts.append(Path(root) / f)
        except OSError:
            pass
        return conflicts
