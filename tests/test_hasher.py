"""
Unit tests for TieredHasher.
"""

from smart_file_organizer.core.hasher import TieredHasher
from smart_file_organizer.core.models import FileInfo


def test_partial_and_full_hash(temp_workspace):
    file_path = temp_workspace / "test.dat"
    content = b"A" * 100000
    file_path.write_bytes(content)

    info = FileInfo.from_path(file_path)
    TieredHasher.populate_hashes(info, full=True)

    assert info.partial_hash is not None
    assert info.full_hash is not None
    assert len(info.full_hash) == 64  # SHA-256 length


def test_identical_files_produce_identical_hashes(temp_workspace):
    f1 = temp_workspace / "f1.bin"
    f2 = temp_workspace / "f2.bin"
    data = b"RANDOM_PAYLOAD_TEST_9999"
    f1.write_bytes(data)
    f2.write_bytes(data)

    hash1 = TieredHasher.compute_full_hash(f1)
    hash2 = TieredHasher.compute_full_hash(f2)

    assert hash1 == hash2
