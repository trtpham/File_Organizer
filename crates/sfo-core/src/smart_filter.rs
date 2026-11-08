//! Invoice / receipt classifier (Vietnamese + English keywords, vendors,
//! dates, amounts) driving the organizer's "Smart Financial Sort".

use crate::models::{FileInfo, ReceiptMetadata};
use chrono::{Datelike, NaiveDateTime};
use regex::{Regex, RegexBuilder};
use std::path::Path;
use std::sync::OnceLock;

/// Extensions the organizer inspects when smart receipts are enabled.
pub const RECEIPT_EXTENSIONS: [&str; 6] = [".pdf", ".txt", ".csv", ".jpg", ".png", ".webp"];

pub const RECEIPT_KEYWORDS_VI: [&str; 21] = [
    "hóa đơn",
    "hoa don",
    "phiếu thu",
    "phieu thu",
    "hóa đơn gtgt",
    "hóa đơn điện tử",
    "tổng tiền",
    "tong tien",
    "thanh toán",
    "thanh toan",
    "biên lai",
    "tiền cước",
    "tiền điện",
    "tiền nước",
    "mã tra cứu",
    "thuế suất",
    "tiền thuế",
    "mã số thuế",
    "phiếu chi",
    "đề nghị thanh toán",
    "xác nhận thanh toán",
];

pub const RECEIPT_KEYWORDS_EN: [&str; 17] = [
    "invoice",
    "tax invoice",
    "receipt",
    "bill to",
    "total amount",
    "amount due",
    "subtotal",
    "payment receipt",
    "order confirmation",
    "statement",
    "billing statement",
    "vat number",
    "tax id",
    "order summary",
    "payment successful",
    "invoice date",
    "due date",
];

/// Checked in order; the first match wins.
pub const COMMON_VENDORS: [&str; 31] = [
    "EVN",
    "Viettel",
    "VNPT",
    "FPT",
    "Apple",
    "Google",
    "Microsoft",
    "AWS",
    "Amazon",
    "Grab",
    "Shopee",
    "Tiki",
    "Lazada",
    "Vietcombank",
    "Techcombank",
    "MBBank",
    "VPBank",
    "Ahamove",
    "Be Group",
    "Spotify",
    "Netflix",
    "Momo",
    "ZaloPay",
    "ShopeePay",
    "ViettelPay",
    "JetBrains",
    "GitHub",
    "DigitalOcean",
    "Hetzner",
    "OpenAI",
    "Adobe",
];

const MAX_TEXT_CHARS: usize = 20_000;
const PDF_MAX_PAGES: usize = 5;

macro_rules! re {
    ($name:ident, $pat:expr, $ci:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| {
                RegexBuilder::new($pat)
                    .case_insensitive($ci)
                    .build()
                    .expect("static regex")
            })
        }
    };
}
re!(re_dmy, r"\b(\d{1,2})[/-](\d{1,2})[/-](20\d{2})\b", false);
re!(re_ymd, r"\b(20\d{2})[/-](\d{1,2})[/-](\d{1,2})\b", false);
re!(re_vnd, r"(\d{1,3}(?:[.,]\d{3})+)\s*(?:VND|VNĐ|đ)", true);
re!(re_usd, r"[$€£]\s*(\d{1,3}(?:,\d{3})*(?:\.\d{2})?)", false);
re!(
    re_intl,
    r"(\d{1,3}(?:,\d{3})*(?:\.\d{2})?)\s*(?:USD|EUR|GBP)",
    true
);

fn vendor_regexes() -> &'static Vec<(&'static str, Regex)> {
    static R: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    R.get_or_init(|| {
        COMMON_VENDORS
            .iter()
            .map(|v| {
                let re = RegexBuilder::new(&format!(r"\b{}\b", regex::escape(v)))
                    .case_insensitive(true)
                    .build();
                (*v, re.expect("vendor regex"))
            })
            .collect()
    })
}

/// Inspects a file for financial metadata.
pub fn analyze_file(file: &FileInfo) -> ReceiptMetadata {
    let ext = file.extension.to_lowercase();
    let text = match ext.as_str() {
        ".pdf" => extract_text_from_pdf(&file.path),
        ".txt" | ".csv" => std::fs::read(&file.path)
            .map(|b| {
                String::from_utf8_lossy(&b)
                    .chars()
                    .take(MAX_TEXT_CHARS)
                    .collect()
            })
            .unwrap_or_default(),
        _ => file.filename.clone(),
    };
    analyze_text(&text, &file.modified_time)
}

/// Classifies already-extracted text; `fallback` supplies year/month.
pub fn analyze_text(text: &str, fallback: &NaiveDateTime) -> ReceiptMetadata {
    if text.is_empty() {
        return ReceiptMetadata::default();
    }
    let lower = text.to_lowercase();
    let matches = RECEIPT_KEYWORDS_VI
        .iter()
        .chain(RECEIPT_KEYWORDS_EN.iter())
        .filter(|k| lower.contains(*k))
        .count();
    if matches == 0 {
        return ReceiptMetadata::default();
    }
    let confidence = (0.4 + matches as f64 * 0.2).min(0.99);
    let vendor = vendor_regexes()
        .iter()
        .find(|(_, re)| re.is_match(text))
        .map(|(v, _)| v.to_string());
    let (year, month, date_str) = extract_date(text, fallback);
    let amount = extract_amount(text);
    let summary = format!(
        "Invoice ({}, {year}-{month:02})",
        vendor.as_deref().unwrap_or("Unknown Vendor")
    );
    ReceiptMetadata {
        is_receipt: true,
        vendor,
        date_str: Some(date_str),
        year: Some(year),
        month: Some(month),
        amount,
        currency: None,
        confidence,
        summary,
    }
}

/// `(year, month, "YYYY-MM-DD")` from `DD/MM/20YY`, then `20YY-MM-DD`, then `fallback`.
pub fn extract_date(text: &str, fallback: &NaiveDateTime) -> (i32, u32, String) {
    let parse = |re: &Regex, order: [usize; 3]| -> Option<(i32, u32, u32)> {
        let c = re.captures(text)?;
        let g = |i: usize| c.get(i)?.as_str().parse::<u32>().ok();
        let (d, m, y) = (g(order[0])?, g(order[1])?, g(order[2])?);
        ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y as i32, m, d))
    };
    // Python only checks the first match of each pattern.
    for (re, order) in [(re_dmy(), [1, 2, 3]), (re_ymd(), [3, 2, 1])] {
        if let Some((y, m, d)) = parse(re, order) {
            return (y, m, format!("{y}-{m:02}-{d:02}"));
        }
    }
    (
        fallback.year(),
        fallback.month(),
        fallback.format("%Y-%m-%d").to_string(),
    )
}

/// `"1.250.000 VND"`, `"$149.50"` or `"12.00 USD"`.
pub fn extract_amount(text: &str) -> Option<String> {
    if let Some(c) = re_vnd().captures(text) {
        return Some(format!("{} VND", &c[1]));
    }
    if let Some(c) = re_usd().captures(text) {
        return Some(format!("${}", &c[1]));
    }
    re_intl().captures(text).map(|c| format!("{} USD", &c[1]))
}

/// Text of the first pages via `lopdf`, falling back to raw `( ... )` string
/// literals in the first 64 KB (the Python stdlib fallback).
pub fn extract_text_from_pdf(path: &Path) -> String {
    let parsed = std::panic::catch_unwind(|| -> Option<String> {
        let doc = lopdf::Document::load(path).ok()?;
        let pages: Vec<u32> = doc
            .get_pages()
            .keys()
            .copied()
            .take(PDF_MAX_PAGES)
            .collect();
        doc.extract_text(&pages).ok()
    });
    if let Ok(Some(text)) = parsed
        && !text.trim().is_empty()
    {
        return text;
    }
    let Ok(raw) = std::fs::read(path) else {
        return String::new();
    };
    let raw = &raw[..raw.len().min(65_536)];
    static RAW: OnceLock<regex::bytes::Regex> = OnceLock::new();
    let re = RAW.get_or_init(|| {
        regex::bytes::Regex::new(r"(?-u)\(([A-Za-z0-9\s.,:\-/_]{4,})\)").expect("static regex")
    });
    re.captures_iter(raw)
        .map(|c| c[1].iter().map(|&b| char::from(b)).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::models::OrganizeStrategy;
    use crate::organizer::{Organizer, PlanOptions};

    fn file(dir: &Path, name: &str, text: &str) -> FileInfo {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        FileInfo::from_path(&p).unwrap()
    }

    #[test]
    fn vietnamese_invoice() {
        let d = tempfile::tempdir().unwrap();
        let f = file(
            d.path(),
            "electric_bill.txt",
            "TẬP ĐOÀN ĐIỆN LỰC VIỆT NAM (EVN)\nHÓA ĐƠN GIÁ TRỊ GIA TĂNG (TIỀN ĐIỆN)\nNgày: 15/10/2026\n\
             Tổng tiền thanh toán: 1.250.000 VNĐ\nMã tra cứu: EVN987654321\n",
        );
        let m = analyze_file(&f);
        assert!(m.is_receipt);
        assert_eq!(m.vendor.as_deref(), Some("EVN"));
        assert_eq!((m.year, m.month), (Some(2026), Some(10)));
        assert!(m.amount.unwrap().contains("1.250.000 VND"));
        assert_eq!(m.summary, "Invoice (EVN, 2026-10)");
    }

    #[test]
    fn english_invoice() {
        let d = tempfile::tempdir().unwrap();
        let f = file(
            d.path(),
            "aws_cloud_statement.txt",
            "Amazon Web Services (AWS)\nTax Invoice / Billing Statement\nInvoice Date: 2026-08-01\n\
             Total Amount Due: $149.50\nPayment Receipt Confirmed\n",
        );
        let m = analyze_file(&f);
        assert!(m.is_receipt);
        assert_eq!(m.vendor.as_deref(), Some("AWS"));
        assert_eq!((m.year, m.month), (Some(2026), Some(8)));
        assert!(m.amount.unwrap().contains("$149.50"));
        assert_eq!(m.confidence, 0.99);
    }

    #[test]
    fn smart_financial_sort_routing() {
        let d = tempfile::tempdir().unwrap();
        let bill = file(
            d.path(),
            "bill.txt",
            "Hóa đơn thanh toán tiền điện EVN 2026-05-10 tổng tiền 500.000 VNĐ",
        );
        let essay = file(
            d.path(),
            "essay.txt",
            "This is an essay about history without any financial keywords.",
        );
        let cfg = Config::defaults();
        let opts = PlanOptions {
            smart_receipts: true,
            ..Default::default()
        };
        let recs =
            Organizer::new(&cfg).plan(&[bill, essay], d.path(), OrganizeStrategy::Extension, opts);
        let bill_dst = recs[0]
            .destination_path
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            bill_dst.contains("Finance_2026") && bill_dst.contains("EVN"),
            "{bill_dst}"
        );
        assert!(
            recs[0]
                .message
                .starts_with("Smart Sort (Invoice (EVN, 2026-05))")
        );
        assert!(
            recs[1]
                .destination_path
                .as_ref()
                .unwrap()
                .to_string_lossy()
                .contains("Documents")
        );
    }

    #[test]
    fn dates_amounts_and_pdf_fallback() {
        let fb = crate::paths::py_fromisoformat("2024-02-03T00:00:00").unwrap();
        assert_eq!(
            extract_date("on 40/13/2026", &fb),
            (2024, 2, "2024-02-03".into())
        );
        assert_eq!(extract_amount("Total 12.00 eur"), Some("12.00 USD".into()));
        assert_eq!(extract_amount("nothing"), None);
        assert!(!analyze_text("hello", &fb).is_receipt);

        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.pdf");
        std::fs::write(&p, b"%PDF-1.4 garbage (Invoice Date 2026-01-02) (ab) more").unwrap();
        assert_eq!(extract_text_from_pdf(&p), "Invoice Date 2026-01-02");
    }
}
