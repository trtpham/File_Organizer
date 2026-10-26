"""
Unit tests for TransactionExecutor.
"""

from smart_file_organizer.core.scanner import FileScanner
from smart_file_organizer.core.organizer import Organizer
from smart_file_organizer.core.executor import TransactionExecutor
from smart_file_organizer.core.models import OrganizeStrategy


def test_dry_run_leaves_files_untouched(populated_workspace):
    scanner = FileScanner()
    files_before = set(p.name for p in populated_workspace.iterdir())

    organizer = Organizer()
    records = organizer.plan_organization(scanner.scan(populated_workspace), populated_workspace)

    executor = TransactionExecutor()
    session = executor.execute_session(
        records,
        populated_workspace,
        populated_workspace,
        strategy="extension",
        dry_run=True
    )

    files_after = set(p.name for p in populated_workspace.iterdir())
    assert files_before == files_after
    assert session.dry_run is True


def test_live_execution_moves_files(populated_workspace):
    scanner = FileScanner()
    organizer = Organizer()
    records = organizer.plan_organization(scanner.scan(populated_workspace), populated_workspace)

    executor = TransactionExecutor()
    session = executor.execute_session(
        records,
        populated_workspace,
        populated_workspace,
        strategy="extension",
        dry_run=False
    )

    assert session.moved_count > 0
    assert (populated_workspace / "Images" / "photo1.jpg").exists()
    assert (populated_workspace / "Documents" / "report.pdf").exists()


def test_collision_auto_rename(temp_workspace):
    dest_dir = temp_workspace / "Images"
    dest_dir.mkdir()
    (dest_dir / "pic.png").write_text("DESTINATION_FILE")

    src_file = temp_workspace / "pic.png"
    src_file.write_text("SOURCE_FILE")

    scanner = FileScanner()
    organizer = Organizer()
    records = organizer.plan_organization([scanner.scan(temp_workspace, recursive=False)[0]], temp_workspace)

    executor = TransactionExecutor()
    executor.execute_session(records, temp_workspace, temp_workspace, strategy="extension", dry_run=False)

    # Should have auto-renamed to pic (1).png
    assert (dest_dir / "pic.png").exists()
    assert (dest_dir / "pic (1).png").exists()
