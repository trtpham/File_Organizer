"""
Unit tests for Organizer.
"""

from smart_file_organizer.core.scanner import FileScanner
from smart_file_organizer.core.organizer import Organizer
from smart_file_organizer.core.models import OrganizeStrategy, ActionType


def test_organize_by_extension(populated_workspace):
    scanner = FileScanner()
    files = scanner.scan(populated_workspace)

    organizer = Organizer()
    records = organizer.plan_organization(files, populated_workspace, strategy=OrganizeStrategy.EXTENSION)

    dest_folders = {r.source_path.name: r.destination_path.parent.name for r in records if r.destination_path}

    assert dest_folders["photo1.jpg"] == "Images"
    assert dest_folders["photo2.png"] == "Images"
    assert dest_folders["report.pdf"] == "Documents"
    assert dest_folders["archive.zip"] == "Archives"
    assert dest_folders["script.py"] == "Code"


def test_organize_by_size(populated_workspace):
    scanner = FileScanner()
    files = scanner.scan(populated_workspace)

    organizer = Organizer()
    records = organizer.plan_organization(files, populated_workspace, strategy=OrganizeStrategy.SIZE)

    for r in records:
        assert r.destination_path.parent.name.startswith("Tiny")
