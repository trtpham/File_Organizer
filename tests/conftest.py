"""
Pytest fixtures providing realistic filesystem environments for tests.
"""

import pytest
import tempfile
import shutil
from pathlib import Path


@pytest.fixture
def temp_workspace():
    """Creates an isolated temporary directory and tears it down afterwards."""
    temp_dir = Path(tempfile.mkdtemp(prefix="sfo_test_"))
    yield temp_dir
    shutil.rmtree(temp_dir, ignore_errors=True)


@pytest.fixture
def populated_workspace(temp_workspace):
    """
    Sets up a realistic messy folder with:
    - Images (.png, .jpg)
    - Documents (.pdf, .txt, .docx)
    - Code (.py, .json)
    - Archives (.zip)
    - Duplicates (same content, different names)
    - Ignored files (.tmp, .crdownload, desktop.ini)
    """
    ws = temp_workspace

    # Unique files
    (ws / "photo1.jpg").write_bytes(b"JPEG_IMAGE_DATA_12345")
    (ws / "photo2.png").write_bytes(b"PNG_IMAGE_DATA_67890")
    (ws / "report.pdf").write_bytes(b"PDF_DOCUMENT_CONTENT_ABCDE")
    (ws / "notes.txt").write_bytes(b"PLAIN_TEXT_NOTES")
    (ws / "archive.zip").write_bytes(b"PK_ZIP_ARCHIVE_DATA")
    (ws / "script.py").write_bytes(b"print('Hello world!')")

    # Duplicate files (Exact same bytes)
    dup_data = b"DUPLICATE_FILE_CONTENT_IDENTICAL_1234567890" * 200
    (ws / "original_file.pdf").write_bytes(dup_data)
    (ws / "copy_of_file.pdf").write_bytes(dup_data)
    (ws / "another_copy.pdf").write_bytes(dup_data)

    # Ignored files
    (ws / "download.crdownload").write_bytes(b"INCOMPLETE_DOWNLOAD")
    (ws / "temp_file.tmp").write_bytes(b"TEMPORARY_DATA")
    (ws / "desktop.ini").write_text("[.ShellClassInfo]\nIconResource=...")

    return ws
