import os
import time
from pathlib import Path
from smart_file_organizer.core.archiver import ColdStorageAnalyzer, ColdStorageArchiver
from smart_file_organizer.core.scanner import FileScanner
from smart_file_organizer.core.config import ConfigManager


def test_cold_storage_scanning_and_archiving(tmp_path: Path):
    source = tmp_path / "source"
    source.mkdir()

    # Create old file (mtime set back 100 days)
    old_file = source / "archive_me.txt"
    old_file.write_text("Hello World Cold Storage" * 20)
    old_timestamp = time.time() - (100 * 86400)
    os.utime(old_file, (old_timestamp, old_timestamp))

    # Create fresh file
    fresh_file = source / "fresh.txt"
    fresh_file.write_text("Fresh Content")

    # Scan for cold files older than 50 days
    cold_files = ColdStorageAnalyzer.scan_cold_files(source, inactive_days=50)
    assert len(cold_files) == 1
    assert cold_files[0].path.name == "archive_me.txt"

    # Create archive
    zip_dest = tmp_path / "archives" / "cold_backup.zip"
    res = ColdStorageArchiver.create_cold_archive(
        cold_files,
        zip_dest,
        base_dir=source,
        delete_sources=True
    )
    assert res["success"] is True
    assert res["archived_count"] == 1
    assert zip_dest.exists()
    assert not old_file.exists()  # deleted after successful compression
    assert fresh_file.exists()    # preserved untouched

    # Restore archive
    restore_target = tmp_path / "restored"
    restore_res = ColdStorageArchiver.restore_archive(zip_dest, target_dir=restore_target)
    assert restore_res["success"] is True
    assert (restore_target / "archive_me.txt").exists()
    assert (restore_target / "archive_me.txt").read_text() == "Hello World Cold Storage" * 20
