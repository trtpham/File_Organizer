"""
Smart Content Filter for automated invoice, receipt, and financial bill sorting.
"""

import re
from pathlib import Path
from typing import Optional, List
from datetime import datetime
from .models import FileInfo, ReceiptMetadata


class ReceiptClassifier:
    """Extracts text from documents and identifies invoices, bills, and tax receipts."""

    RECEIPT_KEYWORDS_VI = [
        "hóa đơn", "hoa don", "phiếu thu", "phieu thu", "hóa đơn gtgt", "hóa đơn điện tử",
        "tổng tiền", "tong tien", "thanh toán", "thanh toan", "biên lai", "tiền cước",
        "tiền điện", "tiền nước", "mã tra cứu", "thuế suất", "tiền thuế"
    ]

    RECEIPT_KEYWORDS_EN = [
        "invoice", "tax invoice", "receipt", "bill to", "total amount",
        "amount due", "subtotal", "payment receipt", "order confirmation",
        "statement", "billing statement", "vat number"
    ]

    COMMON_VENDORS = [
        "EVN", "Viettel", "VNPT", "FPT", "Apple", "Google", "Microsoft", "AWS",
        "Amazon", "Grab", "Shopee", "Tiki", "Lazada", "Vietcombank", "Techcombank",
        "MBBank", "VPBank", "Ahamove", "Be Group", "Spotify", "Netflix"
    ]

    @classmethod
    def analyze_file(cls, file_info: FileInfo) -> ReceiptMetadata:
        """Inspects file contents and extracts financial / invoice metadata."""
        ext = file_info.extension.lower()
        text = ""

        # 1. Text extraction
        if ext == ".pdf":
            text = cls._extract_text_from_pdf(file_info.path)
        elif ext in (".txt", ".csv"):
            try:
                text = file_info.path.read_text(encoding="utf-8", errors="ignore")[:10000]
            except Exception:
                pass
        else:
            # For images or other files, inspect the filename
            text = file_info.filename

        if not text:
            return ReceiptMetadata(is_receipt=False)

        text_lower = text.lower()

        # 2. Score keyword matches
        matches_vi = sum(1 for kw in cls.RECEIPT_KEYWORDS_VI if kw in text_lower)
        matches_en = sum(1 for kw in cls.RECEIPT_KEYWORDS_EN if kw in text_lower)
        total_matches = matches_vi + matches_en

        if total_matches == 0:
            return ReceiptMetadata(is_receipt=False)

        # Confidence metric (1 match -> 0.6, 2 matches -> 0.8, 3+ matches -> 0.95)
        confidence = min(0.99, 0.4 + (total_matches * 0.2))

        # 3. Extract Vendor
        detected_vendor = None
        for vendor in cls.COMMON_VENDORS:
            if re.search(rf"\b{re.escape(vendor)}\b", text, re.IGNORECASE):
                detected_vendor = vendor
                break

        # 4. Extract Transaction Date and Year
        year, month, date_str = cls._extract_date(text, file_info.modified_time)

        # 5. Extract Total Amount
        amount = cls._extract_amount(text)

        summary = f"Invoice ({detected_vendor or 'Unknown Vendor'}, {year}-{month:02d})"
        return ReceiptMetadata(
            is_receipt=True,
            vendor=detected_vendor,
            date_str=date_str,
            year=year,
            month=month,
            amount=amount,
            confidence=confidence,
            summary=summary
        )

    @classmethod
    def _extract_text_from_pdf(cls, path: Path) -> str:
        """Extracts text from PDF using PyMuPDF (fitz) or PyPDF2 with regex stream fallback."""
        # 1. Try PyMuPDF (fitz) - fastest and most accurate
        try:
            import fitz
            doc = fitz.open(path)
            extracted = []
            for i in range(min(3, len(doc))):
                extracted.append(doc[i].get_text())
            doc.close()
            return "\n".join(extracted)
        except Exception:
            pass

        # 2. Try PyPDF2
        try:
            import PyPDF2
            with open(path, "rb") as f:
                reader = PyPDF2.PdfReader(f)
                extracted = []
                for i in range(min(3, len(reader.pages))):
                    page_text = reader.pages[i].extract_text()
                    if page_text:
                        extracted.append(page_text)
                return "\n".join(extracted)
        except Exception:
            pass

        # 3. Pure standard library fallback: inspect raw ASCII streams in binary PDF
        try:
            with open(path, "rb") as f:
                raw = f.read(50000)
                # Look for printable text blocks in PDF
                strings = re.findall(rb"\(([A-Za-z0-9\s.,:\-/_]{4,})\)", raw)
                return " ".join(s.decode("latin-1", errors="ignore") for s in strings)
        except Exception:
            return ""

    @staticmethod
    def _extract_date(text: str, fallback_dt: datetime) -> (int, int, str):
        """Extracts invoice date, returning (year, month, formatted_string)."""
        # Match dates like 25/12/2026 or 2026-12-25
        match = re.search(r"\b(\d{1,2})[/-](\d{1,2})[/-](20\d{2})\b", text)
        if match:
            day, month, year = int(match.group(1)), int(match.group(2)), int(match.group(3))
            if 1 <= month <= 12 and 1 <= day <= 31:
                return year, month, f"{year}-{month:02d}-{day:02d}"

        # Match ISO format YYYY-MM-DD
        iso_match = re.search(r"\b(20\d{2})[/-](\d{1,2})[/-](\d{1,2})\b", text)
        if iso_match:
            year, month, day = int(iso_match.group(1)), int(iso_match.group(2)), int(iso_match.group(3))
            if 1 <= month <= 12 and 1 <= day <= 31:
                return year, month, f"{year}-{month:02d}-{day:02d}"

        # Fallback to file timestamp
        return fallback_dt.year, fallback_dt.month, fallback_dt.strftime("%Y-%m-%d")

    @staticmethod
    def _extract_amount(text: str) -> Optional[str]:
        """Detects total currency amount."""
        # Currency matches: 1,234,567 VND or $123.45
        vnd_match = re.search(r"(\d{1,3}(?:[.,]\d{3})+)\s*(?:VND|VNĐ|đ)", text, re.IGNORECASE)
        if vnd_match:
            return vnd_match.group(1) + " VND"

        usd_match = re.search(r"[$€£]\s*(\d{1,3}(?:,\d{3})*(?:\.\d{2})?)", text)
        if usd_match:
            return "$" + usd_match.group(1)

        return None
