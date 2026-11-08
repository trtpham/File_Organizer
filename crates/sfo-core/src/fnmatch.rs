//! Python `fnmatch.fnmatch` semantics.
//!
//! On Windows CPython applies `os.path.normcase` to both name and pattern:
//! lowercase, and `/` becomes `\`. Elsewhere matching is exact.

use regex::Regex;

/// A compiled shell-style pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    re: Option<Regex>,
}

impl Pattern {
    pub fn new(pattern: &str) -> Self {
        let re = Regex::new(&translate(&normcase(pattern))).ok();
        Self { re }
    }

    pub fn matches(&self, name: &str) -> bool {
        self.re
            .as_ref()
            .is_some_and(|re| re.is_match(&normcase(name)))
    }
}

/// One-shot `fnmatch.fnmatch(name, pattern)`.
pub fn fnmatch(name: &str, pattern: &str) -> bool {
    Pattern::new(pattern).matches(name)
}

fn normcase(s: &str) -> String {
    if cfg!(windows) {
        s.to_lowercase().replace('/', "\\")
    } else {
        s.to_string()
    }
}

/// Port of `fnmatch.translate` targeting the `regex` crate.
fn translate(pat: &str) -> String {
    let chars: Vec<char> = pat.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut out = String::from("^(?s:");
    while i < n {
        let c = chars[i];
        i += 1;
        match c {
            '*' => {
                while i < n && chars[i] == '*' {
                    i += 1;
                }
                out.push_str(".*");
            }
            '?' => out.push('.'),
            '[' => {
                let mut j = i;
                if j < n && chars[j] == '!' {
                    j += 1;
                }
                if j < n && chars[j] == ']' {
                    j += 1;
                }
                while j < n && chars[j] != ']' {
                    j += 1;
                }
                if j >= n {
                    out.push_str("\\[");
                } else {
                    let mut stuff: String = chars[i..j].iter().collect();
                    i = j + 1;
                    let negate = stuff.starts_with('!');
                    if negate {
                        stuff.remove(0);
                    }
                    if stuff.is_empty() {
                        // Empty set never matches (`[!]` matches any char).
                        out.push_str(if negate { "." } else { r"\b\B" });
                        continue;
                    }
                    let mut class = String::new();
                    for (k, ch) in stuff.chars().enumerate() {
                        match ch {
                            '\\' | '[' | ']' | '&' | '~' => {
                                class.push('\\');
                                class.push(ch);
                            }
                            '^' if k == 0 => class.push_str("\\^"),
                            _ => class.push(ch),
                        }
                    }
                    out.push('[');
                    if negate {
                        out.push('^');
                    }
                    out.push_str(&class);
                    out.push(']');
                }
            }
            _ => out.push_str(&regex::escape(&c.to_string())),
        }
    }
    out.push_str(")$");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_globs() {
        assert!(fnmatch("temp.tmp", "*.tmp"));
        assert!(fnmatch("video.mp4.crdownload", "*.crdownload"));
        assert!(!fnmatch("file.part2", "*.part"));
        assert!(fnmatch("a.txt", "?.txt"));
        assert!(fnmatch("b1", "[abc]1"));
        assert!(!fnmatch("d1", "[abc]1"));
        assert!(fnmatch("d1", "[!abc]1"));
        assert!(fnmatch("[x", "[x"));
        assert!(fnmatch("a/.git/b", "*/.git/*"));
        assert!(fnmatch("$RECYCLE.BIN", "$RECYCLE.BIN"));
    }

    #[test]
    fn case_follows_platform() {
        assert_eq!(fnmatch("Thumbs.DB", "thumbs.db"), cfg!(windows));
    }
}
