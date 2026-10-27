"""
Unit tests for DevJunkCleaner.
"""

from pathlib import Path
from smart_file_organizer.core.junk_cleaner import DevJunkCleaner, JunkCategory


def test_scan_and_clean_junk_directory(temp_workspace):
    # Simulate a project with node_modules and __pycache__
    project_dir = temp_workspace / "old_project"
    project_dir.mkdir()

    nm = project_dir / "node_modules"
    nm.mkdir()
    (nm / "package.json").write_text("{}")
    (nm / "bloat.js").write_bytes(b"var x = 1;" * 500)

    pycache = project_dir / "__pycache__"
    pycache.mkdir()
    (pycache / "module.cpython-314.pyc").write_bytes(b"BYTECODE" * 100)

    # Scan with inactive_days=0 so all are detected
    junk_items = DevJunkCleaner.scan_directory(temp_workspace, inactive_days=0)

    assert len(junk_items) >= 2
    names = {item.name for item in junk_items}
    assert "node_modules" in names
    assert "__pycache__" in names

    # Clean junk
    res = DevJunkCleaner.clean_junk(junk_items, dry_run=False)
    assert res["success"] is True
    assert res["cleaned_count"] >= 2
    assert not nm.exists()
    assert not pycache.exists()
