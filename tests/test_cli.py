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

