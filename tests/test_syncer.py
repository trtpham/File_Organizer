"""
Unit tests for DriveDetector and FolderSynchronizer.
"""

from pathlib import Path
from smart_file_organizer.core.models import SyncProfile, SyncMode
from smart_file_organizer.core.syncer import DriveDetector, FolderSynchronizer


def test_drive_detector_runs_without_error():
    # Drive detection should execute cleanly across platforms
    drives = DriveDetector.get_removable_drives()
    assert isinstance(drives, list)


def test_incremental_sync(temp_workspace):
    src = temp_workspace / "source_folder"
    src.mkdir()
    (src / "file1.txt").write_text("Hello Sync")
    (src / "sub").mkdir()
    (src / "sub" / "file2.txt").write_text("Nested File")

    dst = temp_workspace / "backup_folder"

    profile = SyncProfile(
        name="Test Sync",
        source_dir=src,
        dest_dir=dst,
        mode=SyncMode.INCREMENTAL
    )

    # 1. First sync -> copies all files
    res1 = FolderSynchronizer.sync(profile, dry_run=False)
    assert res1["success"] is True
    assert res1["copied_count"] == 2
    assert (dst / "file1.txt").exists()
    assert (dst / "sub" / "file2.txt").exists()

    # 2. Second sync with no changes -> skips all files
    res2 = FolderSynchronizer.sync(profile, dry_run=False)
    assert res2["copied_count"] == 0
    assert res2["skipped_count"] == 2


def test_snapshot_sync(temp_workspace):
    src = temp_workspace / "savegame_folder"
    src.mkdir()
    (src / "save.dat").write_bytes(b"LEVEL_99_PLAYER_DATA")

    dst = temp_workspace / "snapshots"

    profile = SyncProfile(
        name="RPG_Save",
        source_dir=src,
        dest_dir=dst,
        mode=SyncMode.SNAPSHOT
    )

    res = FolderSynchronizer.sync(profile, dry_run=False)
    assert res["success"] is True
    assert res["copied_count"] == 1
    assert "RPG_Save" in res["archive_path"]
    assert Path(res["archive_path"]).exists()
