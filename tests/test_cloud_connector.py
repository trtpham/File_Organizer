from pathlib import Path
from smart_file_organizer.core.cloud_connector import CloudDetector, CloudProvider, CloudSyncManager
from smart_file_organizer.core.models import SyncMode


def test_cloud_detector_runs_safely():
    providers = CloudDetector.detect_all()
    # Should return a list without raising any exceptions
    assert isinstance(providers, list)
    for p in providers:
        assert isinstance(p, CloudProvider)
        assert p.name in ["OneDrive", "Google Drive", "Dropbox", "iCloud Drive", "Nextcloud", "Box"]
        assert isinstance(p.local_path, Path)


def test_cloud_sync_manager(tmp_path: Path):
    source = tmp_path / "documents"
    source.mkdir()
    (source / "report.pdf").write_text("Company Q3 Financial Report")

    fake_cloud_root = tmp_path / "FakeOneDrive"
    fake_cloud_root.mkdir()

    provider = CloudProvider("OneDrive", fake_cloud_root, is_active=True, provider_type="Microsoft OneDrive")
    res = CloudSyncManager.sync_to_cloud(
        source_dir=source,
        provider=provider,
        subfolder_name="SFO_Backup",
        mode=SyncMode.INCREMENTAL,
        dry_run=False
    )

    assert res["success"] is True
    target_synced = fake_cloud_root / "SFO_Backup" / "report.pdf"
    assert target_synced.exists()
    assert target_synced.read_text() == "Company Q3 Financial Report"

    # Test sync conflict detector
    (fake_cloud_root / "conflicted_doc.sync-conflict-20261028.txt").write_text("conflict")
    conflicts = CloudSyncManager.find_sync_conflicts(fake_cloud_root)
    assert len(conflicts) == 1
