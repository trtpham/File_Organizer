"""
Unit tests for Deduplicator.
"""

from smart_file_organizer.core.scanner import FileScanner
from smart_file_organizer.core.deduplicator import Deduplicator


def test_deduplicator_finds_exact_duplicates(populated_workspace):
    scanner = FileScanner()
    files = scanner.scan(populated_workspace)

    dedup = Deduplicator()
    dup_groups = dedup.find_duplicates(files)

    # Exactly 1 duplicate group with 3 files (original_file, copy_of_file, another_copy)
    assert len(dup_groups) == 1
    group = dup_groups[0]
    assert len(group.files) == 3
    assert group.suggested_keep is not None
    assert group.wasted_space_bytes == 2 * group.file_size


def test_plan_deduplication_actions(populated_workspace):
    scanner = FileScanner()
    files = scanner.scan(populated_workspace)

    dedup = Deduplicator()
    dup_groups = dedup.find_duplicates(files)
    records = dedup.plan_deduplication_actions(dup_groups, populated_workspace, action_mode="move_to_duplicates")

    # Should plan to move 2 files into _Duplicates folder, preserving the 1 original
    assert len(records) == 2
    for r in records:
        assert r.destination_path.parent.name == "_Duplicates"
