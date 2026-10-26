"""
High-performance duplicate file detection using 3-tiered hashing.
"""

from collections import defaultdict
from pathlib import Path
from typing import List, Dict, Optional, Callable
from .models import FileInfo, DuplicateGroup, ActionRecord, ActionType
from .hasher import TieredHasher
from .config import ConfigManager


class Deduplicator:
    """Finds and resolves duplicate files efficiently across three filtering tiers."""

    def __init__(self, config: Optional[ConfigManager] = None):
        self.config = config or ConfigManager()

    def find_duplicates(
        self,
        files: List[FileInfo],
        progress_callback: Optional[Callable[[str, int, int], None]] = None
    ) -> List[DuplicateGroup]:
        """
        Executes 3-tiered duplicate identification:
          Tier 1: Group by file size in bytes
          Tier 2: Group by 64KB partial hash
          Tier 3: Group by full SHA-256 hash
        """
        # Tier 1: Group by exact size (exclude 0-byte files from hashing)
        size_groups: Dict[int, List[FileInfo]] = defaultdict(list)
        for f in files:
            if f.size_bytes > 0:
                size_groups[f.size_bytes].append(f)

        candidates_tier1: List[List[FileInfo]] = [
            group for group in size_groups.values() if len(group) > 1
        ]

        if progress_callback:
            progress_callback("tier1_complete", len(candidates_tier1), len(files))

        # Tier 2: Partial Hash on surviving candidates
        partial_groups: Dict[str, List[FileInfo]] = defaultdict(list)
        for group in candidates_tier1:
            for file_info in group:
                TieredHasher.populate_hashes(file_info, full=False)
                if file_info.partial_hash:
                    # Key combined with size to prevent accidental cross-size collisions
                    key = f"{file_info.size_bytes}_{file_info.partial_hash}"
                    partial_groups[key].append(file_info)

        candidates_tier2: List[List[FileInfo]] = [
            group for group in partial_groups.values() if len(group) > 1
        ]

        if progress_callback:
            progress_callback("tier2_complete", len(candidates_tier2), len(candidates_tier1))

        # Tier 3: Full SHA-256 Cryptographic Hash
        full_groups: Dict[str, List[FileInfo]] = defaultdict(list)
        for group in candidates_tier2:
            for file_info in group:
                TieredHasher.populate_hashes(file_info, full=True)
                if file_info.full_hash:
                    key = f"{file_info.size_bytes}_{file_info.full_hash}"
                    full_groups[key].append(file_info)

        # Build DuplicateGroup results
        keep_policy = self.config.deduplication_settings.get("keep_policy", "oldest")
        duplicate_groups: List[DuplicateGroup] = []

        for key, duplicate_files in full_groups.items():
            if len(duplicate_files) > 1:
                suggested_keep = self._select_keep_file(duplicate_files, keep_policy)
                group = DuplicateGroup(
                    file_hash=duplicate_files[0].full_hash or "",
                    file_size=duplicate_files[0].size_bytes,
                    files=duplicate_files,
                    suggested_keep=suggested_keep
                )
                duplicate_groups.append(group)

        if progress_callback:
            progress_callback("tier3_complete", len(duplicate_groups), len(candidates_tier2))

        return duplicate_groups

    def _select_keep_file(self, files: List[FileInfo], policy: str) -> FileInfo:
        """Determines which file in the duplicate group should be preserved as the original."""
        if policy == "newest":
            return max(files, key=lambda f: f.modified_time)
        elif policy == "shortest_path":
            return min(files, key=lambda f: len(str(f.path)))
        else:  # default: oldest
            return min(files, key=lambda f: f.created_time)

    def plan_deduplication_actions(
        self,
        groups: List[DuplicateGroup],
        destination_dir: Path,
        action_mode: str = "move_to_duplicates"
    ) -> List[ActionRecord]:
        """
        Creates actionable ActionRecords for resolving duplicates.
        action_mode options:
          - 'move_to_duplicates': moves non-original duplicates to _Duplicates folder
          - 'delete': permanently marks duplicate for deletion
        """
        records: List[ActionRecord] = []
        dup_folder_name = self.config.deduplication_settings.get("duplicates_folder_name", "_Duplicates")
        duplicates_root = destination_dir / dup_folder_name

        for group in groups:
            keep_file = group.suggested_keep
            for file_info in group.files:
                if keep_file and file_info.path == keep_file.path:
                    # Original file to keep - no moving
                    continue

                if action_mode == "delete":
                    records.append(ActionRecord(
                        action_type=ActionType.DELETE,
                        source_path=file_info.path,
                        destination_path=None,
                        file_size=file_info.size_bytes,
                        file_hash=file_info.full_hash,
                        message=f"Duplicate of {keep_file.path.name if keep_file else 'unknown'}"
                    ))
                else:  # move_to_duplicates
                    target_dest = duplicates_root / file_info.filename
                    records.append(ActionRecord(
                        action_type=ActionType.DEDUPLICATE,
                        source_path=file_info.path,
                        destination_path=target_dest,
                        file_size=file_info.size_bytes,
                        file_hash=file_info.full_hash,
                        message=f"Duplicate of {keep_file.path.name if keep_file else 'unknown'}"
                    ))

        return records
