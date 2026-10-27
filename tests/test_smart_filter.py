"""
Unit tests for ReceiptClassifier and Smart Financial Sort.
"""

from pathlib import Path
from smart_file_organizer.core.models import FileInfo, OrganizeStrategy
from smart_file_organizer.core.smart_filter import ReceiptClassifier
from smart_file_organizer.core.organizer import Organizer


def test_vietnamese_invoice_detection(temp_workspace):
    inv_file = temp_workspace / "electric_bill.txt"
    inv_file.write_text(
        "TẬP ĐOÀN ĐIỆN LỰC VIỆT NAM (EVN)\n"
        "HÓA ĐƠN GIÁ TRỊ GIA TĂNG (TIỀN ĐIỆN)\n"
        "Ngày: 15/10/2026\n"
        "Tổng tiền thanh toán: 1.250.000 VNĐ\n"
        "Mã tra cứu: EVN987654321\n",
        encoding="utf-8"
    )

    info = FileInfo.from_path(inv_file)
    meta = ReceiptClassifier.analyze_file(info)

    assert meta.is_receipt is True
    assert meta.vendor == "EVN"
    assert meta.year == 2026
    assert meta.month == 10
    assert "1.250.000 VND" in meta.amount


def test_english_invoice_detection(temp_workspace):
    inv_file = temp_workspace / "aws_cloud_statement.txt"
    inv_file.write_text(
        "Amazon Web Services (AWS)\n"
        "Tax Invoice / Billing Statement\n"
        "Invoice Date: 2026-08-01\n"
        "Total Amount Due: $149.50\n"
        "Payment Receipt Confirmed\n",
        encoding="utf-8"
    )

    info = FileInfo.from_path(inv_file)
    meta = ReceiptClassifier.analyze_file(info)

    assert meta.is_receipt is True
    assert meta.vendor == "AWS"
    assert meta.year == 2026
    assert meta.month == 8
    assert "$149.50" in meta.amount


def test_smart_financial_sort_routing(temp_workspace):
    inv_file = temp_workspace / "bill.txt"
    inv_file.write_text("Hóa đơn thanh toán tiền điện EVN 2026-05-10 tổng tiền 500.000 VNĐ", encoding="utf-8")

    normal_doc = temp_workspace / "essay.txt"
    normal_doc.write_text("This is an essay about history without any financial keywords.")

    files = [FileInfo.from_path(inv_file), FileInfo.from_path(normal_doc)]
    organizer = Organizer()

    records = organizer.plan_organization(
        files,
        destination_dir=temp_workspace,
        strategy=OrganizeStrategy.EXTENSION,
        smart_receipts=True
    )

    dest_paths = {r.source_path.name: str(r.destination_path) for r in records}

    # Invoice should route to Finance_2026/EVN
    assert "Finance_2026" in dest_paths["bill.txt"]
    assert "EVN" in dest_paths["bill.txt"]

    # Normal document routes to Documents
    assert "Documents" in dest_paths["essay.txt"]
