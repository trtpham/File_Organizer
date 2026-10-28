from pathlib import Path
from smart_file_organizer.core.enterprise_guardrails import EnterpriseGuardrails, AuditLogChain


def test_protected_path_detection():
    # Drive root should be protected
    is_prot, reason = EnterpriseGuardrails.is_protected_path(Path("C:/"))
    assert is_prot is True

    # System directory should be protected
    is_prot_win, _ = EnterpriseGuardrails.is_protected_path(Path("C:/Windows/System32"))
    assert is_prot_win is True

    # User document folder should be safe
    safe_dir = Path.home() / "Documents" / "MyProject"
    is_prot_safe, _ = EnterpriseGuardrails.is_protected_path(safe_dir)
    assert is_prot_safe is False


def test_blast_radius_threshold():
    # Under limit
    exceeded, _ = EnterpriseGuardrails.check_blast_radius(100, 1024 * 1024)
    assert exceeded is False

    # Exceed file count limit
    exceeded_count, msg = EnterpriseGuardrails.check_blast_radius(600, 1024)
    assert exceeded_count is True
    assert "600 files" in msg

    # Exceed byte size limit
    exceeded_size, msg_size = EnterpriseGuardrails.check_blast_radius(5, 12 * 1024 * 1024 * 1024)
    assert exceeded_size is True
    assert "12.0 GB" in msg_size


def test_audit_log_hash_chain(tmp_path: Path):
    audit_file = tmp_path / "test_audit.jsonl"
    chain = AuditLogChain(log_path=audit_file)

    # Initial state
    assert chain.get_latest_hash() == AuditLogChain.GENESIS_HASH

    # Append first event
    entry1 = chain.log_event("ORGANIZE", {"moved_files": 10}, user="test_admin")
    assert entry1["previous_hash"] == AuditLogChain.GENESIS_HASH
    assert len(entry1["current_hash"]) == 64

    # Append second event
    entry2 = chain.log_event("CLEAN_JUNK", {"freed_bytes": 500000}, user="test_admin")
    assert entry2["previous_hash"] == entry1["current_hash"]

    # Verify chain integrity
    is_valid, count, _ = chain.verify_integrity()
    assert is_valid is True
    assert count == 2

    # Tamper with the log file to test security detection
    with open(audit_file, "r+", encoding="utf-8") as f:
        content = f.read()
        f.seek(0)
        f.write(content.replace("10", "9999"))  # Alter payload without updating hash

    is_valid_after_tamper, _, msg_tamper = chain.verify_integrity()
    assert is_valid_after_tamper is False
    assert "tampered" in msg_tamper or "mismatch" in msg_tamper
