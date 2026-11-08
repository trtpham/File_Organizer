//! English / Vietnamese string tables.
//!
//! `data/translations.json` is exported verbatim from the Python v3.0
//! `TRANSLATIONS` dict by `scripts/gen_compat_fixtures.py`.

use serde_json::{Map, Value};
use std::sync::{OnceLock, RwLock};

const TRANSLATIONS_JSON: &str = include_str!("../data/translations.json");

fn tables() -> &'static Map<String, Value> {
    static T: OnceLock<Map<String, Value>> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(TRANSLATIONS_JSON).expect("embedded translations.json is valid")
    })
}

fn current() -> &'static RwLock<String> {
    static LANG: OnceLock<RwLock<String>> = OnceLock::new();
    LANG.get_or_init(|| RwLock::new("en".into()))
}

/// Sets the active language; unknown codes fall back to `"en"`.
pub fn set_language(code: &str) {
    let code = code.to_lowercase();
    let lang = if tables().contains_key(&code) {
        code
    } else {
        "en".into()
    };
    *current().write().unwrap_or_else(|e| e.into_inner()) = lang;
}

/// Active language code.
pub fn language() -> String {
    current().read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Supported language codes in table order.
pub fn languages() -> Vec<String> {
    tables().keys().cloned().collect()
}

/// Looks up `key` in the active language, then English, then returns `key`.
pub fn t(key: &str) -> String {
    let lang = language();
    let lookup = |l: &str| {
        tables()
            .get(l)
            .and_then(|m| m.get(key))
            .and_then(Value::as_str)
    };
    lookup(&lang)
        .or_else(|| lookup("en"))
        .unwrap_or(key)
        .to_string()
}

/// `t(key)` with Python `str.format(**kwargs)` substitution. Any unknown
/// placeholder leaves the template unformatted, like the Python fallback.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let template = t(key);
    format_named(&template, args).unwrap_or(template)
}

fn format_named(template: &str, args: &[(&str, &str)]) -> Option<String> {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next()? {
                        '}' => break,
                        ch => name.push(ch),
                    }
                }
                let (_, value) = args.iter().find(|(k, _)| *k == name)?;
                out.push_str(value);
            }
            '}' => return None,
            _ => out.push(c),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test owns the global language to avoid races between tests.
    #[test]
    fn translation_and_fallback() {
        set_language("en");
        assert_eq!(language(), "en");
        assert!(t("app_title").contains("Smart File Organizer"));
        assert_eq!(t("execute_now"), "⚡ Execute Now");

        set_language("vi");
        assert_eq!(language(), "vi");
        assert!(t("app_title").contains("Doanh nghiệp"));
        assert_eq!(t("execute_now"), "⚡ Thực hiện Ngay");

        set_language("fr");
        assert_eq!(language(), "en");
        assert_eq!(t("no_such_key"), "no_such_key");

        let msg = tf(
            "blast_radius_warning",
            &[("count", "500"), ("bytes", "10 GB")],
        );
        assert!(msg.contains("500") && msg.contains("10 GB"));
        assert!(tf("blast_radius_warning", &[("count", "1")]).contains("{count}"));
        set_language("en");
    }
}
