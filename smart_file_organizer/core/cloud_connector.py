"""
Cloud Storage Sync Connector.
Detects local synchronization folders for major cloud storage providers:
OneDrive, Google Drive, Dropbox, iCloud Drive.
Allows routing organized categories or backups directly to cloud storage.
Zero external dependencies.
"""

import os
import json
from pathlib import Path
from typing import List, Dict, Any, Optional

from .syncer import FolderSynchronizer
from .models import SyncProfile, SyncMode


class CloudProvider:
    """Represents a detected cloud storage service."""
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
            # Pick first valid candidate
            chosen = candidates[0]
            return CloudProvider("OneDrive", chosen, is_active=True, provider_type="Microsoft OneDrive")
        return None

    @classmethod
    def detect_google_drive(cls) -> Optional[CloudProvider]:
        """Detects Google Drive directory or virtual drive."""
        # 1. Check virtual drive letters (Windows DriveFS mounts as G:, H:, etc.)
        if os.name == "nt":
            import string
            for letter in ["G", "H", "I", "J", "D", "E"]:
                drive_root = Path(f"{letter}:/")
                my_drive = drive_root / "My Drive"
                if my_drive.is_dir():
                    return CloudProvider("Google Drive", my_drive, is_active=True, provider_type="Google Drive Virtual")
                if (drive_root / "Shared drives").is_dir():
                    return CloudProvider("Google Drive", drive_root, is_active=True, provider_type="Google Drive Virtual")

        # 2. Check local sync folder
        home_gdrive = Path.home() / "Google Drive"
        if home_gdrive.is_dir():
            return CloudProvider("Google Drive", home_gdrive, is_active=True, provider_type="Google Drive Client")

        return None

    @classmethod
    def detect_dropbox(cls) -> Optional[CloudProvider]:
        """Detects Dropbox sync directory via info.json or user home."""
        # Dropbox stores host info in info.json
        possible_info_paths = []
        if os.name == "nt":
            local_appdata = os.environ.get("LOCALAPPDATA")
            appdata = os.environ.get("APPDATA")
            if local_appdata:
                possible_info_paths.append(Path(local_appdata) / "Dropbox" / "info.json")
            if appdata:
                possible_info_paths.append(Path(appdata) / "Dropbox" / "info.json")
        else:
            possible_info_paths.append(Path.home() / ".dropbox" / "info.json")

        for info_file in possible_info_paths:
            if info_file.is_file():
                try:
                    with open(info_file, "r", encoding="utf-8") as f:
                        data = json.load(f)
                        for account_type in ["personal", "business"]:
                            if account_type in data and "path" in data[account_type]:
                                p = Path(data[account_type]["path"])
                                if p.is_dir():
                                    return CloudProvider("Dropbox", p, is_active=True, provider_type="Dropbox Client")
                except Exception:
                    pass

        # Fallback default
        fallback = Path.home() / "Dropbox"
        if fallback.is_dir():
            return CloudProvider("Dropbox", fallback, is_active=True, provider_type="Dropbox Client")

        return None

    @classmethod
    def detect_icloud(cls) -> Optional[CloudProvider]:
        """Detects Apple iCloud Drive directory."""
        if os.name == "nt":
            user_profile = os.environ.get("USERPROFILE")
            if user_profile:
                icloud_path = Path(user_profile) / "iCloudDrive"
                if icloud_path.is_dir():
                    return CloudProvider("iCloud Drive", icloud_path, is_active=True, provider_type="Apple iCloud")
        else:
            mac_icloud = Path.home() / "Library" / "Mobile Documents" / "com~apple~CloudDocs"
            if mac_icloud.is_dir():
                return CloudProvider("iCloud Drive", mac_icloud, is_active=True, provider_type="Apple iCloud")

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
