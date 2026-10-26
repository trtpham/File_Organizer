"""
Unit tests for UndoManager.
"""

from smart_file_organizer.core.scanner import FileScanner
from smart_file_organizer.core.organizer import Organizer
from smart_file_organizer.core.executor import TransactionExecutor
from smart_file_organizer.core.undo_manager import UndoManager


def test_undo_restores_all_files(populated_workspace, temp_workspace):
    # Snapshot original files
    original_files = {p.name: p.read_bytes() for p in populated_workspace.iterdir() if p.is_file()}

    scanner = FileScanner()
    organizer = Organizer()
    records = organizer.plan_organization(scanner.scan(populated_workspace), populated_workspace)

    executor = TransactionExecutor()
    session = executor.execute_session(records, populated_workspace, populated_workspace, strategy="extension", dry_run=False)

    undo_mgr = UndoManager(data_dir=temp_workspace / "sfo_data")
    undo_mgr.save_session(session)

    # Verify files moved
    assert not (populated_workspace / "photo1.jpg").exists()
    assert (populated_workspace / "Images" / "photo1.jpg").exists()

    # Perform Rollback
    result = undo_mgr.rollback_session(session.session_id)
    assert result["success"] is True

    # Verify all original files are restored with exact content
    for name, content in original_files.items():
        restored = populated_workspace / name
        assert restored.exists(), f"File {name} was not restored"
        assert restored.read_bytes() == content

    # Verify empty category directories were cleaned up
    assert not (populated_workspace / "Images").exists()
    assert not (populated_workspace / "Documents").exists()
