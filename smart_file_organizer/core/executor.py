"""
Transaction execution engine handling safe file moves, collisions, and dry-runs.
"""

import os
import shutil
import uuid
from datetime import datetime
from pathlib import Path
from typing import List, Optional, Callable
from .models import ActionRecord, ActionType, CollisionStrategy, SessionLog
from .hasher import TieredHasher
from .config import ConfigManager


class TransactionExecutor:
    """Executes planned actions safely with collision resolution and dry-run support."""

    def __init__(self, config: Optional[ConfigManager] = None):
        self.config = config or ConfigManager()

    def execute_session(
        self,
        records: List[ActionRecord],
        source_dir: Path,
        destination_dir: Path,
        strategy: str,
        dry_run: bool = False,
        progress_callback: Optional[Callable[[int, int, ActionRecord], None]] = None
    ) -> SessionLog:
        """
        Executes a sequence of ActionRecords, producing a recorded SessionLog.
        """
        session_id = f"sfo_{datetime.now().strftime('%Y%m%d_%H%M%S')}_{uuid.uuid4().hex[:6]}"
        start_time = datetime.now()

        session_log = SessionLog(
            session_id=session_id,
            start_time=start_time,
            source_dir=str(source_dir),
            destination_dir=str(destination_dir),
            strategy=strategy,
            dry_run=dry_run,
            total_files=len(records)
        )

        collision_policy = self.config.collision_strategy
        total = len(records)

        for idx, record in enumerate(records, start=1):
            if record.action_type == ActionType.SKIP:
                session_log.skipped_count += 1
                session_log.records.append(record)
                if progress_callback:
                    progress_callback(idx, total, record)
                continue

            # Process Move / Deduplicate / Delete
            self._process_single_record(record, collision_policy, dry_run)

            if record.status == "SUCCESS":
                if record.action_type in (ActionType.MOVE, ActionType.DEDUPLICATE):
                    session_log.moved_count += 1
                    session_log.total_size_bytes += record.file_size
                    if record.action_type == ActionType.DEDUPLICATE:
                        session_log.duplicate_count += 1
                        session_log.space_saved_bytes += record.file_size
                elif record.action_type == ActionType.DELETE:
                    session_log.duplicate_count += 1
                    session_log.space_saved_bytes += record.file_size
            elif record.status == "SKIPPED":
                session_log.skipped_count += 1

            session_log.records.append(record)
            if progress_callback:
                progress_callback(idx, total, record)

        if not dry_run and self.config.data.get("clean_empty_folders", False):
            self.clean_empty_directories(source_dir, exclude_root=True)

        session_log.end_time = datetime.now()
        return session_log

    def _process_single_record(
        self,
        record: ActionRecord,
        collision_policy: CollisionStrategy,
        dry_run: bool
    ) -> None:
        """Processes an individual record, handling collision and filesystem action."""
        if not record.source_path.exists():
            record.status = "FAILED"
            record.message = f"Source file no longer exists: {record.source_path}"
            return

        if record.action_type == ActionType.DELETE:
            if dry_run:
                record.status = "SUCCESS"
                record.message = f"[DRY-RUN] Would delete {record.source_path.name}"
            else:
                try:
                    record.source_path.unlink()
                    record.status = "SUCCESS"
                    record.message = f"Deleted duplicate: {record.source_path.name}"
                except OSError as e:
                    record.status = "FAILED"
                    record.message = f"Delete failed: {str(e)}"
            return

        # Handle MOVE / DEDUPLICATE with destination path
        if not record.destination_path:
            record.status = "FAILED"
            record.message = "Destination path is missing"
            return

        target_dest = record.destination_path

        # Handle destination collision if target file already exists
        if target_dest.exists() and target_dest.resolve() != record.source_path.resolve():
            target_dest, collision_handled = self._resolve_collision(
                record.source_path, target_dest, collision_policy
            )
            if not collision_handled:
                record.status = "SKIPPED"
                record.message = f"Skipped due to collision policy at: {target_dest.name}"
                return
            record.destination_path = target_dest

        # Execute physical or dry-run move
        if dry_run:
            record.status = "SUCCESS"
            record.message = f"[DRY-RUN] Planned move to {target_dest}"
        else:
            try:
                target_dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(str(record.source_path), str(target_dest))
                record.status = "SUCCESS"
                record.message = f"Moved to {target_dest}"
            except OSError as e:
                record.status = "FAILED"
                record.message = f"Move failed: {str(e)}"

    def _resolve_collision(
        self,
        source: Path,
        dest: Path,
        policy: CollisionStrategy
    ) -> (Path, bool):
        """Resolves target collision according to selected CollisionStrategy."""
        if policy == CollisionStrategy.SKIP:
            return dest, False

        elif policy == CollisionStrategy.OVERWRITE:
            return dest, True

        elif policy == CollisionStrategy.COMPARE_HASH:
            src_hash = TieredHasher.compute_full_hash(source)
            dst_hash = TieredHasher.compute_full_hash(dest)
            if src_hash and dst_hash and src_hash == dst_hash:
                # Content is identical, skip moving
                return dest, False
            # Different content: auto rename to avoid losing data
            return self._generate_unique_name(dest), True

        else:  # AUTO_RENAME (default)
            return self._generate_unique_name(dest), True

    def _generate_unique_name(self, target_path: Path) -> Path:
        """Finds an unused filename by appending (1), (2), etc."""
        parent = target_path.parent
        stem = target_path.stem
        suffix = target_path.suffix
        counter = 1

        while True:
            candidate = parent / f"{stem} ({counter}){suffix}"
            if not candidate.exists():
                return candidate
            counter += 1

    @staticmethod
    def clean_empty_directories(root_dir: Path, exclude_root: bool = True) -> int:
        """Recursively removes empty directories within root_dir."""
        removed_count = 0
        root_path = Path(root_dir).resolve()
        for current, dirs, files in os.walk(root_path, topdown=False):
            curr_path = Path(current).resolve()
            if exclude_root and curr_path == root_path:
                continue
            if not os.listdir(curr_path):
                try:
                    curr_path.rmdir()
                    removed_count += 1
                except OSError:
                    pass
        return removed_count
