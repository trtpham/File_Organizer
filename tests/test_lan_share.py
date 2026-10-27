import urllib.request
from pathlib import Path
from smart_file_organizer.core.lan_share import LANShareServer, get_local_ip


def test_get_local_ip():
    ip = get_local_ip()
    assert isinstance(ip, str)
    assert len(ip.split(".")) == 4


def test_lan_share_server_lifecycle(tmp_path: Path):
    shared_folder = tmp_path / "share"
    shared_folder.mkdir()
    sample_file = shared_folder / "test_file.txt"
    sample_file.write_text("LAN Share Data Payload")

    port = 18991
    server = LANShareServer(shared_dir=shared_folder, port=port, pin=None)
    start_res = server.start()

    assert start_res["success"] is True
    assert server.is_running() is True

    try:
        # Test HTTP GET index
        url = f"http://127.0.0.1:{port}/"
        with urllib.request.urlopen(url, timeout=3) as resp:
            content = resp.read().decode("utf-8")
            assert resp.status == 200
            assert "Smart File Organizer - LAN Drop" in content
            assert "test_file.txt" in content

        # Test download
        dl_url = f"http://127.0.0.1:{port}/download/test_file.txt"
        with urllib.request.urlopen(dl_url, timeout=3) as resp:
            data = resp.read().decode("utf-8")
            assert resp.status == 200
            assert data == "LAN Share Data Payload"

    finally:
        stop_res = server.stop()
        assert stop_res["success"] is True
        assert server.is_running() is False
