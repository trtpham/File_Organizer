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

    # Slugify with accents removal
    f2 = temp_workspace / "Hóa Đơn Tiền Điện Tháng 10.pdf"
    f2.write_text("sample")
    info2 = FileInfo.from_path(f2)
    records_slug = renamer.plan_rename([info2], template="{name}", case_transform="slugify")
    assert records_slug[0].destination_path.name == "hoa-don-tien-dien-thang-10.pdf"


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
    mp3 = temp_workspace / "test.mp3"
    # Construct synthetic ID3v1 trailer (128 bytes)
    header = b"TAG"
    title = b"Test Song".ljust(30, b"\x00")
    artist = b"Test Artist".ljust(30, b"\x00")
    album = b"Test Album".ljust(30, b"\x00")
    year = b"2026".ljust(4, b"\x00")
    comment = b"Comment".ljust(30, b"\x00")
    genre = b"\x0c"

    v1_payload = header + title + artist + album + year + comment + genre
    mp3.write_bytes(b"FAKE_AUDIO_DATA" * 10 + v1_payload)

    tags = ID3Extractor.extract(mp3)
    assert tags["title"] == "Test Song"
    assert tags["artist"] == "Test Artist"
    assert tags["album"] == "Test Album"
    assert tags["year"] == "2026"
