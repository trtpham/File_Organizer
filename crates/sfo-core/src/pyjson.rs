//! Byte-exact reimplementation of Python's `json.dumps()` defaults.
//!
//! The audit chain hashes `json.dumps(details, sort_keys=True)`, so the Rust
//! build must emit exactly what CPython emits: `", "` / `": "` separators,
//! `ensure_ascii=True` escaping (UTF-16 surrogate pairs for astral chars), and
//! `repr(float)` number formatting.

use serde_json::Value;

/// `json.dumps(value, sort_keys=sort_keys)` with every other option default.
pub fn dumps(value: &Value, sort_keys: bool) -> String {
    let mut out = String::new();
    write_value(&mut out, value, sort_keys);
    out
}

fn write_value(out: &mut String, value: &Value, sort_keys: bool) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                out.push_str(&py_float_repr(n.as_f64().unwrap_or(f64::NAN)));
            }
        }
        Value::String(s) => write_str(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(out, item, sort_keys);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            if sort_keys {
                // Python sorts str keys by code point == UTF-8 byte order.
                entries.sort_by(|a, b| a.0.cmp(b.0));
            }
            for (i, (k, v)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_str(out, k);
                out.push_str(": ");
                write_value(out, v, sort_keys);
            }
            out.push('}');
        }
    }
}

/// `ensure_ascii=True` string encoding.
fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ' '..='~' => out.push(c),
            _ => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
}

/// CPython `float.__repr__`: shortest round-trip digits, scientific notation
/// when the decimal exponent is `< -4` or `>= 16`.
pub fn py_float_repr(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if f == 0.0 {
        return if f.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    // Rust `{:e}` yields the shortest round-trip digits, e.g. "1.5e16".
    let sci = format!("{f:e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let neg = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    let sign = if neg { "-" } else { "" };

    if !(-4..16).contains(&exp) {
        let (head, rest) = digits.split_at(1);
        let mant = if rest.is_empty() {
            head.to_string()
        } else {
            format!("{head}.{rest}")
        };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{mant}e{esign}{:02}", exp.abs());
    }
    if exp >= 0 {
        let int_len = (exp + 1) as usize;
        let (int_part, frac) = if digits.len() > int_len {
            (digits[..int_len].to_string(), digits[int_len..].to_string())
        } else {
            (format!("{digits:0<int_len$}"), "0".to_string())
        };
        format!("{sign}{int_part}.{frac}")
    } else {
        let zeros = "0".repeat((-exp - 1) as usize);
        format!("{sign}0.{zeros}{digits}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn float_repr_matches_cpython() {
        let cases = [
            (0.25, "0.25"),
            (1e-05, "1e-05"),
            (1.5e16, "1.5e+16"),
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (2.0, "2.0"),
            (0.0001, "0.0001"),
            (-3.5, "-3.5"),
            (123456.789, "123456.789"),
            (0.1 + 0.2, "0.30000000000000004"),
        ];
        for (f, want) in cases {
            assert_eq!(py_float_repr(f), want, "repr({f})");
        }
    }

    #[test]
    fn dumps_matches_cpython() {
        let v = json!({"name": "tài liệu 📄", "nested": {"b": [1, 2.0, null, true], "a": "x"}});
        assert_eq!(
            dumps(&v, true),
            r#"{"name": "t\u00e0i li\u1ec7u \ud83d\udcc4", "nested": {"a": "x", "b": [1, 2.0, null, true]}}"#
        );
        assert_eq!(
            dumps(&json!({"p": "C:\\x\n"}), false),
            r#"{"p": "C:\\x\n"}"#
        );
        assert_eq!(dumps(&json!({}), true), "{}");
    }
}
