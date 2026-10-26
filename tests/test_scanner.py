"""
Unit tests for FileScanner.
"""

from smart_file_organizer.core.scanner import FileScanner


def test_scanner_filters_ignored_files(populated_workspace):
    scanner = FileScanner()
    files = scanner.scan(populated_workspace, recursive=False)
    filenames = {f.filename for f in files}

    # Should include valid files
    assert "photo1.jpg" in filenames
    assert "report.pdf" in filenames
    assert "script.py" in filenames

    # Should filter out ignored files
    assert "download.crdownload" not in filenames
    assert "temp_file.tmp" not in filenames
    assert "desktop.ini" not in filenames


def test_scanner_extracts_correct_metadata(populated_workspace):
    scanner = FileScanner()
    files = scanner.scan(populated_workspace)
    report_file = next(f for f in files if f.filename == "report.pdf")

    assert report_file.extension == ".pdf"
    assert report_file.size_bytes > 0
    assert report_file.path.exists()
