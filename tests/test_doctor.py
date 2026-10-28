from pathlib import Path
from smart_file_organizer.core.doctor import SFODoctor


def test_doctor_full_diagnosis(tmp_path: Path):
    report = SFODoctor.run_full_diagnosis(target_path=tmp_path)

    assert "overall_status" in report
    assert report["overall_status"] in ["HEALTHY", "UNHEALTHY"]
    assert report["pass_count"] >= 3
    assert isinstance(report["checks"], list)

    check_names = [c["name"] for c in report["checks"]]
    assert "Python Runtime" in check_names
    assert "Operating System" in check_names
    assert "Disk Headroom" in check_names
