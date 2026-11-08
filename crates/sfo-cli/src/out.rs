//! Terminal output: coloured status lines and ASCII tables (Python
//! `cli/formatter.py`). `anstream` strips colours when output is piped or
//! `NO_COLOR` is set, and enables ANSI on older Windows consoles.

use anstream::println;

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[36m";

pub fn banner() {
    println!(
        "\n{CYAN}{BOLD}======================================================================\n  \
         SMART FILE ORGANIZER (SFO) v{} - Native Rust Edition\n  \
         Automated Cleanup, Deep Deduplication & Safe 100% Rollback\n\
         ======================================================================{RESET}\n",
        sfo_core::VERSION
    );
}

pub fn success(msg: &str) {
    println!("{GREEN}[+] {msg}{RESET}");
}

pub fn info(msg: &str) {
    println!("{CYAN}[*] {msg}{RESET}");
}

pub fn warning(msg: &str) {
    println!("{YELLOW}[!] {msg}{RESET}");
}

pub fn error(msg: &str) {
    println!("{RED}[x] {msg}{RESET}");
}

pub fn plain(msg: &str) {
    println!("{msg}");
}

/// Visible width, ignoring ANSI escape sequences.
fn visible_len(s: &str) -> usize {
    let mut n = 0;
    let mut in_esc = false;
    for c in s.chars() {
        match (in_esc, c) {
            (false, '\x1b') => in_esc = true,
            (true, 'm') => in_esc = false,
            (true, _) => {}
            (false, _) => n += 1,
        }
    }
    n
}

fn pad(s: &str, width: usize) -> String {
    format!("{s}{}", " ".repeat(width.saturating_sub(visible_len(s))))
}

/// Header row, `-+-` separator, then rows; columns sized to content.
pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }
    let mut widths: Vec<usize> = headers.iter().map(|h| visible_len(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(widths.len()) {
            widths[i] = widths[i].max(visible_len(cell));
        }
    }
    let head: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| pad(h, widths[i]))
        .collect();
    println!("{BOLD}{}{RESET}", head.join(" | "));
    println!(
        "{}",
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("-+-")
    );
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, c)| pad(c, widths.get(i).copied().unwrap_or(0)))
            .collect();
        println!("{}", cells.join(" | "));
    }
}

/// Python `f"{s:<width}"`.
pub fn left(s: &str, width: usize) -> String {
    pad(s, width)
}

/// Python `f"{s:>width}"`.
pub fn right(s: &str, width: usize) -> String {
    format!("{}{s}", " ".repeat(width.saturating_sub(visible_len(s))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_ignore_ansi() {
        assert_eq!(visible_len(&format!("{GREEN}PASS{RESET}")), 4);
        assert_eq!(left("ab", 4), "ab  ");
        assert_eq!(right("ab", 4), "  ab");
        assert_eq!(left("abcdef", 2), "abcdef");
    }
}
