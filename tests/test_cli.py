"""
Unit tests for CLI App routing.
"""

from smart_file_organizer.cli.app import run_cli


def test_cli_help(capsys):
    try:
        run_cli(["--help"])
    except SystemExit as e:
        assert e.code == 0

    captured = capsys.readouterr()
    assert "Smart File Organizer" in captured.out


def test_cli_dry_run(populated_workspace, capsys):
    code = run_cli(["--source", str(populated_workspace), "--dry-run"])
    assert code == 0

    captured = capsys.readouterr()
    assert "DRY RUN" in captured.out or "DRY-RUN" in captured.out
    assert "photo1.jpg" in captured.out


def test_cli_history_empty(temp_workspace, capsys):
    code = run_cli(["--history"])
    assert code == 0


def test_cli_detect_cloud(capsys):
    code = run_cli(["--detect-cloud"])
    assert code == 0
    captured = capsys.readouterr()
    assert "cloud storage provider" in captured.out.lower()


def test_cli_archive_cold_empty(temp_workspace, capsys):
    code = run_cli(["--source", str(temp_workspace), "--archive-cold", "--archive-days", "365"])
    assert code == 0
    captured = capsys.readouterr()
    assert "cold files" in captured.out.lower()


def test_cli_shred(temp_workspace, capsys):
    test_file = temp_workspace / "to_shred.bin"
    test_file.write_bytes(b"\x12\x34\x56" * 50)
    assert test_file.exists()

    code = run_cli(["--shred", str(test_file), "--shred-method", "quick_1pass"])
    assert code == 0
    assert not test_file.exists()
    captured = capsys.readouterr()
    assert "successfully shredded" in captured.out.lower()


def test_cli_vault_lifecycle(temp_workspace, capsys):
    secret_file = temp_workspace / "confidential.doc"
    secret_file.write_text("Company Secret Roadmap 2026", encoding="utf-8")

    vault_file = temp_workspace / "my_secrets.sfovault"
    lock_code = run_cli([
        "--vault-lock", str(secret_file),
        "--dest", str(vault_file),
        "--vault-pass", "TestPassword#123",
        "--vault-shred-source"
    ])
    assert lock_code == 0
    assert vault_file.exists()
    assert not secret_file.exists()

    # Inspect
    inspect_code = run_cli([
        "--vault-inspect", str(vault_file),
        "--vault-pass", "TestPassword#123"
    ])
    assert inspect_code == 0

    # Unlock
    extract_folder = temp_workspace / "unlocked_data"
    unlock_code = run_cli([
        "--vault-unlock", str(vault_file),
        "--dest", str(extract_folder),
        "--vault-pass", "TestPassword#123"
    ])
    assert unlock_code == 0
    assert (extract_folder / "confidential.doc").exists()
    assert (extract_folder / "confidential.doc").read_text(encoding="utf-8") == "Company Secret Roadmap 2026"


