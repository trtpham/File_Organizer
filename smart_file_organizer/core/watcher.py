"""
Real-time directory watcher with download stabilization debounce.
"""

import time
import threading
from pathlib import Path
from typing import Dict, Optional, Callable, Set
from .models import FileInfo, OrganizeStrategy, SyncProfile
from .scanner import FileScanner
from .organizer import Organizer
from .executor import TransactionExecutor
from .undo_manager import UndoManager
from .config import ConfigManager
from .syncer import DriveDetector, FolderSynchronizer


class DirectoryWatcher:
    """Monitors a folder in background, debounces downloads, and auto-organizes files."""

    def __init__(
        self,
        source_dir: Path,
        destination_dir: Optional[Path] = None,
        strategy: OrganizeStrategy = OrganizeStrategy.EXTENSION,
        config: Optional[ConfigManager] = None,
        undo_manager: Optional[UndoManager] = None,
        smart_receipts: bool = False,
        sync_profiles: Optional[List[SyncProfile]] = None
    ):
        self.source_dir = Path(source_dir).resolve()
        self.destination_dir = Path(destination_dir or source_dir).resolve()
        self.strategy = strategy
        self.config = config or ConfigManager()
        self.undo_manager = undo_manager or UndoManager()
        self.smart_receipts = smart_receipts
        self.sync_profiles = sync_profiles or []

        self.scanner = FileScanner(self.config)
        self.organizer = Organizer(self.config)
        self.executor = TransactionExecutor(self.config)

        self._stop_event = threading.Event()
        self._thread: Optional[threading.Thread] = None

        # Track file stability: path -> (last_size, last_stable_time)
        self._file_states: Dict[Path, Dict[str, float]] = {}
        self._last_usb_drives: Set[str] = set()

        # Debounce settings
        settings = self.config.watcher_settings
        self.debounce_seconds = float(settings.get("debounce_seconds", 3.0))
        self.check_interval = float(settings.get("check_interval_seconds", 2.0))

    def start(
        self,
        blocking: bool = False,
        on_organized: Optional[Callable[[str, str], None]] = None
    ) -> None:
        """Starts monitoring the source directory."""
        self._stop_event.clear()

        def _run_loop():
            while not self._stop_event.is_set():
                try:
                    self._check_and_organize(on_organized)
                except Exception:
                    pass
                time.sleep(self.check_interval)

        if blocking:
            _run_loop()
        else:
            self._thread = threading.Thread(target=_run_loop, daemon=True)
            self._thread.start()

    def stop(self) -> None:
        """Signals the watcher thread to terminate."""
        self._stop_event.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=3.0)

    @property
    def is_running(self) -> bool:
        return self._thread is not None and self._thread.is_alive() and not self._stop_event.is_set()

    def _check_and_organize(self, on_organized: Optional[Callable[[str, str], None]]) -> None:
        """Inspects source directory and triggers organization for stabilized files."""
        # Exclude destination folder if inside source
        exclude = [self.destination_dir] if self.destination_dir != self.source_dir else []
        candidates = self.scanner.scan(self.source_dir, recursive=False, exclude_dirs=exclude)

        now = time.time()
        current_paths: Set[Path] = set()

        for f in candidates:
            path = f.path
            current_paths.add(path)

            # Skip incomplete downloads
            if path.suffix.lower() in [".crdownload", ".tmp", ".part", ".downloading"]:
                continue

            current_size = f.size_bytes
            state = self._file_states.get(path)

            if state is None:
                # First time seeing this file, record state
                self._file_states[path] = {"size": current_size, "time": now}
            else:
                if state["size"] != current_size:
                    # Size changed, download is still active
                    self._file_states[path] = {"size": current_size, "time": now}
                else:
                    # Size stable, check if debounce duration elapsed
                    if now - state["time"] >= self.debounce_seconds:
                        self._organize_single_file(f, on_organized)
                        # Remove from tracking
                        self._file_states.pop(path, None)

        # Cleanup removed files from states
        stale_paths = [p for p in self._file_states if p not in current_paths]
        for p in stale_paths:
            self._file_states.pop(p, None)

        # Check for USB Drive insertions and auto-sync
        if self.sync_profiles:
            self._check_usb_and_sync()

    def _check_usb_and_sync(self) -> None:
        """Inspects connected USB drives and runs matching sync profiles."""
        drives = DriveDetector.get_removable_drives()
        current_drive_paths = {d["path"] for d in drives}

        # Find newly connected drives
        new_drives = current_drive_paths - self._last_usb_drives
        if new_drives:
            for profile in self.sync_profiles:
                if profile.enabled and profile.trigger_on_usb:
                    FolderSynchronizer.sync(profile)

        self._last_usb_drives = current_drive_paths

    def _organize_single_file(
        self,
        file_info: FileInfo,
        on_organized: Optional[Callable[[str, str], None]]
    ) -> None:
        """Organizes a single stabilized file."""
        records = self.organizer.plan_organization(
            [file_info],
            self.destination_dir,
            strategy=self.strategy,
            smart_receipts=self.smart_receipts
        )
        if not records or records[0].action_type.value == "SKIP":
            return

        session = self.executor.execute_session(
            records,
            self.source_dir,
            self.destination_dir,
            strategy=self.strategy.value,
            dry_run=False
        )
        self.undo_manager.save_session(session)

        if on_organized and session.records and session.records[0].status == "SUCCESS":
            rec = session.records[0]
            on_organized(rec.source_path.name, str(rec.destination_path))
