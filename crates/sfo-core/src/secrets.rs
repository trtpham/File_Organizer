//! DLP secret scanner: credential patterns, Shannon entropy, quarantine.

use crate::fsutil;
use crate::paths;
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// `(label, pattern, severity)`; the first matching pattern per line wins.
pub const PATTERNS: [(&str, &str, &str); 11] = [
    ("AWS Access Key", r"\b(AKIA[0-9A-Z]{16})\b", "CRITICAL"),
    (
        "OpenAI API Key",
        r"\b(sk-[a-zA-Z0-9T3BlbkFJ]{20,})\b",
        "CRITICAL",
    ),
    (
        "GitHub Personal Token",
        r"\b(ghp_[a-zA-Z0-9]{36})\b",
        "CRITICAL",
    ),
    (
        "Slack Token",
        r"\b(xox[pboa]-[0-9A-Za-z\-]{20,})\b",
        "CRITICAL",
    ),
    (
        "Stripe Secret Key",
        r"\b(sk_live_[0-9a-zA-Z]{24,})\b",
        "CRITICAL",
    ),
    (
        "JSON Web Token (JWT)",
        r"\b(eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,})\b",
        "HIGH",
    ),
    (
        "Google Cloud API Key",
        r"\b(AIza[0-9A-Za-z\-_]{35})\b",
        "HIGH",
    ),
    (
        "Private Encryption Key",
        r"-----BEGIN (?:RSA|OPENSSH|DSA|EC) PRIVATE KEY-----",
        "CRITICAL",
    ),
    (
        "Database Connection String",
        r"(?:postgres|mysql|mongodb|redis)://[a-zA-Z0-9_\-\.]+:[^@\s]+@",
        "HIGH",
    ),
    (
        "Vietnam Citizen ID (CCCD)",
        r"(?i)(?:cccd|cmnd|căn cước)[\s:]*([0-9]{9}|[0-9]{12})\b",
        "MEDIUM",
    ),
    (
        "Generic Password Assignment",
        r#"(?i)(?:password|passwd|secret_key)\s*[:=]\s*["']([^"']{8,})["']"#,
        "HIGH",
    ),
];

pub const SCANNABLE_EXTS: [&str; 17] = [
    ".env", ".txt", ".json", ".yaml", ".yml", ".py", ".js", ".ts", ".csv", ".sql", ".xml", ".ini",
    ".conf", ".cfg", ".sh", ".pem", ".key",
];

const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

fn compiled() -> &'static Vec<(&'static str, Regex, &'static str)> {
    static C: OnceLock<Vec<(&'static str, Regex, &'static str)>> = OnceLock::new();
    C.get_or_init(|| {
        PATTERNS
            .iter()
            .map(|(l, p, s)| (*l, Regex::new(p).expect("static pattern"), *s))
            .collect()
    })
}

/// One detected secret.
#[derive(Debug, Clone, PartialEq)]
pub struct SecretFinding {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub secret_type: String,
    pub masked_snippet: String,
    pub severity: String,
    pub entropy: f64,
}

/// Bits per symbol over Unicode scalar values.
pub fn shannon_entropy(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let mut counts: HashMap<char, usize> = HashMap::new();
    let mut len = 0usize;
    for c in text.chars() {
        *counts.entry(c).or_default() += 1;
        len += 1;
    }
    counts.values().fold(0.0, |acc, &n| {
        let p = n as f64 / len as f64;
        acc - p * p.log2()
    })
}

/// Keeps the first and last 4 characters.
pub fn mask(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() > 8 {
        let head: String = chars[..4].iter().collect();
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("{head}{}{tail}", "*".repeat(chars.len() - 8))
    } else {
        "***REDACTED***".into()
    }
}

fn is_scannable(path: &Path) -> bool {
    let name = fsutil::name_of(path);
    let special = name.starts_with(".env")
        || name.starts_with("id_")
        || ["credentials", "secrets", "config"].contains(&name.as_str());
    special || SCANNABLE_EXTS.contains(&paths::suffix_lower(&name).as_str())
}

/// Scans one file (skips unsupported types and files over 5 MB).
pub fn scan_file(path: &Path) -> Vec<SecretFinding> {
    if !is_scannable(path)
        || fs::metadata(path)
            .map(|m| m.len() > MAX_FILE_BYTES)
            .unwrap_or(true)
    {
        return Vec::new();
    }
    let Ok(f) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    let mut reader = BufReader::new(f);
    let mut buf = Vec::new();
    let mut line_no = 0;
    while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
        line_no += 1;
        let line = String::from_utf8_lossy(&buf);
        if let Some((label, m, sev)) = compiled()
            .iter()
            .find_map(|(l, re, s)| re.find(&line).map(|m| (*l, m, *s)))
        {
            let raw = m.as_str();
            findings.push(SecretFinding {
                file_path: path.to_path_buf(),
                line_number: line_no,
                secret_type: label.into(),
                masked_snippet: mask(raw),
                severity: sev.into(),
                entropy: shannon_entropy(raw),
            });
        }
        buf.clear();
    }
    findings
}

/// Scans supported files under `root`.
pub fn scan_directory(root: &Path, recursive: bool) -> Vec<SecretFinding> {
    let root = paths::resolve(root);
    let files = if recursive {
        fsutil::walk_files(&root)
    } else {
        fsutil::list_files(&root)
    };
    files.iter().flat_map(|p| scan_file(p)).collect()
}

/// Result of [`quarantine_files`].
#[derive(Debug, Clone, PartialEq)]
pub struct QuarantineResult {
    pub success: bool,
    pub quarantined_count: usize,
    pub quarantine_dir: PathBuf,
    pub errors: Vec<String>,
    pub message: String,
}

/// Moves each affected file to `<root>/_Quarantine/<name>.quarantined`.
pub fn quarantine_files(
    findings: &[SecretFinding],
    quarantine_root: &Path,
) -> std::io::Result<QuarantineResult> {
    let qdir = quarantine_root.join("_Quarantine");
    fs::create_dir_all(&qdir)?;
    let files: BTreeSet<&PathBuf> = findings.iter().map(|f| &f.file_path).collect();
    let mut moved = 0;
    let mut errors = Vec::new();
    for src in files {
        if !src.exists() {
            continue;
        }
        let dest = qdir.join(format!("{}.quarantined", fsutil::name_of(src)));
        match paths::move_file(src, &dest) {
            Ok(()) => moved += 1,
            Err(e) => errors.push(format!("Failed to isolate {}: {e}", fsutil::name_of(src))),
        }
    }
    Ok(QuarantineResult {
        success: errors.is_empty(),
        quarantined_count: moved,
        message: format!(
            "Quarantined {moved} compromised files to '{}'.",
            qdir.display()
        ),
        quarantine_dir: qdir,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_masks() {
        let d = tempfile::tempdir().unwrap();
        let env = d.path().join(".env");
        fs::write(
            &env,
            "AWS_KEY=AKIAABCDEFGHIJKLMNOP\nnothing here\nDB=postgres://admin:hunter2pass@db.local/x\n\
             password = \"SuperSecretPass123\"\nCCCD: 012345678901\n",
        )
        .unwrap();
        fs::write(d.path().join("image.png"), "AKIAABCDEFGHIJKLMNOP").unwrap();

        let found = scan_directory(d.path(), true);
        let types: Vec<(&str, usize)> = found
            .iter()
            .map(|f| (f.secret_type.as_str(), f.line_number))
            .collect();
        assert_eq!(
            types,
            [
                ("AWS Access Key", 1),
                ("Database Connection String", 3),
                ("Generic Password Assignment", 4),
                ("Vietnam Citizen ID (CCCD)", 5)
            ]
        );
        assert_eq!(found[0].masked_snippet, "AKIA************MNOP");
        assert_eq!(found[0].severity, "CRITICAL");
        assert!(found[0].entropy > 3.0);

        let q = quarantine_files(&found, d.path()).unwrap();
        assert_eq!(q.quarantined_count, 1);
        assert!(d.path().join("_Quarantine/.env.quarantined").exists());
    }

    #[test]
    fn entropy_and_mask_edges() {
        assert_eq!(shannon_entropy(""), 0.0);
        assert_eq!(shannon_entropy("aaaa"), 0.0);
        assert_eq!(shannon_entropy("abcd"), 2.0);
        assert_eq!(mask("short"), "***REDACTED***");
    }
}
