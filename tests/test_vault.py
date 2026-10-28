import os
import pytest
from pathlib import Path

from smart_file_organizer.core.vault import (
    VaultEngine, VaultManager, VaultMetadata,
    VaultError, InvalidPasswordError, CorruptedVaultError, VaultProtectionError
)
from smart_file_organizer.core.enterprise_guardrails import AuditLogChain


@pytest.fixture
def temp_vault_workspace(tmp_path):
    workspace = tmp_path / "vault_test_area"
    workspace.mkdir()
    return workspace


def test_vault_lock_and_unlock_single_file(temp_vault_workspace):
    src_file = temp_vault_workspace / "contract.pdf"
    content = b"%PDF-1.4 Classified Contract Data For Testing\x00\xFF" * 100
    src_file.write_bytes(content)

    vault_file = temp_vault_workspace / "secure.sfovault"
    password = "MasterKey#2026_Secure!"

    # 1. Lock file into vault
    locked_path = VaultManager.lock_path(src_file, vault_file, password)
    assert locked_path.exists()
    assert locked_path.stat().st_size > 0

    # 2. Unlock file into extract directory
    extract_dir = temp_vault_workspace / "extracted"
    extracted_files = VaultManager.unlock_vault(locked_path, extract_dir, password)

    assert len(extracted_files) == 1
    extracted_file = extracted_files[0]
    assert extracted_file.name == "contract.pdf"
    assert extracted_file.read_bytes() == content


def test_vault_lock_and_unlock_directory_tree(temp_vault_workspace):
    folder = temp_vault_workspace / "tax_records"
    folder.mkdir()
    (folder / "summary.txt").write_text("Tax Q1: $50,000", encoding="utf-8")
    sub = folder / "receipts"
    sub.mkdir()
    (sub / "receipt_001.csv").write_text("Item,Price\nLaptop,1500", encoding="utf-8")

    vault_file = temp_vault_workspace / "taxes.sfovault"
    password = "Taxes#Password2026"

    VaultManager.lock_path(folder, vault_file, password)

    extract_dir = temp_vault_workspace / "unlocked_taxes"
    extracted_files = VaultManager.unlock_vault(vault_file, extract_dir, password)

    assert len(extracted_files) == 2
    rel_names = {f.name for f in extracted_files}
    assert "summary.txt" in rel_names
    assert "receipt_001.csv" in rel_names
    assert (extract_dir / "receipts" / "receipt_001.csv").exists()


def test_vault_wrong_password_raises(temp_vault_workspace):
    src_file = temp_vault_workspace / "data.txt"
    src_file.write_text("Sensitive Data")

    vault_file = temp_vault_workspace / "data.sfovault"
    VaultManager.lock_path(src_file, vault_file, "CorrectPassword123")

    extract_dir = temp_vault_workspace / "out"
    with pytest.raises(InvalidPasswordError):
        VaultManager.unlock_vault(vault_file, extract_dir, "WrongPassword999")


def test_vault_tampered_ciphertext_detection(temp_vault_workspace):
    src_file = temp_vault_workspace / "secret.txt"
    src_file.write_text("Integrity Check Data")

    vault_file = temp_vault_workspace / "tamper_test.sfovault"
    VaultManager.lock_path(src_file, vault_file, "Password#123")

    # Read bytes and alter one byte in the ciphertext payload
    data = bytearray(vault_file.read_bytes())
    data[-5] ^= 0xFF  # Flip bits in ciphertext
    vault_file.write_bytes(bytes(data))

    extract_dir = temp_vault_workspace / "out"
    with pytest.raises((InvalidPasswordError, CorruptedVaultError)):
        VaultManager.unlock_vault(vault_file, extract_dir, "Password#123")


def test_vault_inspect_contents_without_extracting(temp_vault_workspace):
    folder = temp_vault_workspace / "docs"
    folder.mkdir()
    (folder / "file_a.txt").write_text("AAA")
    (folder / "file_b.txt").write_text("BBB")

    vault_file = temp_vault_workspace / "docs.sfovault"
    VaultManager.lock_path(folder, vault_file, "InspectorSecret")

    meta = VaultManager.inspect_vault(vault_file, "InspectorSecret")
    assert meta.file_count == 2
    filenames = [f["filename"] for f in meta.files]
    assert any("file_a.txt" in name for name in filenames)
    assert any("file_b.txt" in name for name in filenames)

    # Test verify_password helper
    assert VaultManager.verify_password(vault_file, "InspectorSecret") is True
    assert VaultManager.verify_password(vault_file, "WrongSecret") is False


def test_vault_shred_source_option(temp_vault_workspace):
    src_file = temp_vault_workspace / "to_be_shredded.key"
    src_file.write_text("ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC...")
    assert src_file.exists()

    vault_file = temp_vault_workspace / "keys.sfovault"
    VaultManager.lock_path(src_file, vault_file, "KeyVaultPassword", shred_source=True)

    # Vault container exists
    assert vault_file.exists()
    # Source file was shredded and unlinked
    assert not src_file.exists()
