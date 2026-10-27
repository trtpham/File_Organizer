"""
Unit tests for SmartBulkRenamer, EXIF, and ID3 extraction.
"""

from pathlib import Path
from datetime import datetime
from smart_file_organizer.core.models import FileInfo
from smart_file_organizer.core.renamer import SmartBulkRenamer, ID3Extractor, EXIFExtractor


def test_basic_template_renaming(temp_workspace):
    f1 = temp_workspace / "old_document.txt"
    f1.write_text("sample content")

    info = FileInfo.from_path(f1)
    renamer = SmartBulkRenamer()
    records = renamer.plan_rename(
        [info],
        template="{counter}_{name}",
        start_counter=1,
        counter_padding=3
    )

    assert len(records) == 1
    assert records[0].destination_path.name == "001_old_document.txt"


def test_case_transformations(temp_workspace):
    f1 = temp_workspace / "My Cool Project_V1.txt"
    f1.write_text("sample")

    info = FileInfo.from_path(f1)
    renamer = SmartBulkRenamer()

    # Snake case
    records = renamer.plan_rename([info], template="{name}", case_transform="snake_case")
    assert records[0].destination_path.name == "my_cool_project_v1.txt"

    # Uppercase
    records_up = renamer.plan_rename([info], template="{name}", case_transform="uppercase")
    assert records_up[0].destination_path.name == "MY COOL PROJECT_V1.txt"


def test_find_and_replace(temp_workspace):
    f1 = temp_workspace / "IMG_2026_RAW.jpg"
    f1.write_bytes(b"dummy")

    info = FileInfo.from_path(f1)
    renamer = SmartBulkRenamer()
    records = renamer.plan_rename(
        [info],
        template="{name}",
        find_text="IMG_",
        replace_text="PHOTO_"
    )

    assert records[0].destination_path.name == "PHOTO_2026_RAW.jpg"


def test_id3_v1_extraction(temp_workspace):
    mp3_file = temp_workspace / "song.mp3"
    # Construct standard ID3v1 trailer (128 bytes)
    header = b"\x00" * 1000
    v1_tag = b"TAG" + b"Awesome Title".ljust(30, b"\x00") + b"Top Artist".ljust(30, b"\x00") + b"Great Album".ljust(30, b"\x00") + b"2026" + b"\x00" * 31
    mp3_file.write_bytes(header + v1_tag)

    tags = ID3Extractor.extract(mp3_file)
    assert tags["title"] == "Awesome Title"
    assert tags["artist"] == "Top Artist"
    assert tags["album"] == "Great Album"
