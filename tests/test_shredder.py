import os
import pytest
from pathlib import Path

from smart_file_organizer.core.shredder import FileShredder, ShredMethod, ShredResult, ShredBatchResult
from smart_file_organizer.core.enterprise_guardrails import AuditLogChain


@pytest.fixture
def temp_shred_workspace(tmp_path):
    workspace = tmp_path / "shred_test_area"
    workspace.mkdir()
    return workspace


def test_shred_single_file_dod_3pass(temp_shred_workspace):
    test_file = temp_shred_workspace / "secret_document.txt"
    test_file.write_text("TOP SECRET CLASSIFIED CONTENT " * 100, encoding="utf-8")
    original_size = test_file.stat().st_size
    assert original_size > 0

    res = FileShredder.shred_file(test_file, method=ShredMethod.DOD_3PASS)

    assert res.success is True
    assert res.passes_completed == 3
    assert res.original_size == original_size
    assert not test_file.exists()


def test_shred_single_file_quick_1pass(temp_shred_workspace):
    test_file = temp_shred_workspace / "quick_wipe.bin"
    test_file.write_bytes(b"\xaa\xbb\xcc" * 500)

    res = FileShredder.shred_file(test_file, method=ShredMethod.QUICK_1PASS)

    assert res.success is True
    assert res.passes_completed == 1
    assert not test_file.exists()


def test_shred_empty_file(temp_shred_workspace):
    empty_file = temp_shred_workspace / "empty.txt"
    empty_file.touch()
    assert empty_file.stat().st_size == 0

    res = FileShredder.shred_file(empty_file, method=ShredMethod.DOD_3PASS)

    assert res.success is True
    assert not empty_file.exists()


def test_shred_directory_recursive(temp_shred_workspace):
    sub_dir = temp_shred_workspace / "confidential_project"
    sub_dir.mkdir()
    (sub_dir / "code.py").write_text("api_key = '12345'", encoding="utf-8")
    nested = sub_dir / "sub"
    nested.mkdir()
    (nested / "database.sqlite").write_bytes(b"SQLITE_DUMMY_HEADER" * 50)

    audit_chain = AuditLogChain(temp_shred_workspace / "audit.jsonl")
    batch_res = FileShredder.shred_directory(sub_dir, method=ShredMethod.DOD_3PASS, audit_chain=audit_chain)

    assert batch_res.total_files == 2
    assert len(batch_res.shredded_files) == 2
    assert len(batch_res.failed_files) == 0
    assert batch_res.success_rate == 100.0
    assert not sub_dir.exists()


def test_shred_guardrails_blocks_protected_path():
    # C:\Windows or C:\
    root_path = Path("C:/") if os.name == "nt" else Path("/")
    with pytest.raises(PermissionError) as exc_info:
        FileShredder.shred_file(root_path)
    assert "Enterprise Guardrails" in str(exc_info.value) or "regular file" in str(exc_info.value)


def test_shred_nonexistent_file_raises():
    fake_path = Path("D:/nonexistent_file_random_99882.txt")
    with pytest.raises(FileNotFoundError):
        FileShredder.shred_file(fake_path)
