"""
Terminal formatting utilities (ANSI colors, ASCII progress bars, tables).
Zero external dependencies.
"""

import sys
import os

# Enable virtual terminal processing on Windows if possible
if os.name == "nt":
    try:
        import ctypes
        kernel32 = ctypes.windll.kernel32
        kernel32.SetConsoleMode(kernel32.GetStdHandle(-11), 7)
    except Exception:
        pass


class Color:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"


def print_banner() -> None:
    banner = f"""
{Color.CYAN}{Color.BOLD}======================================================================
  SMART FILE ORGANIZER (SFO) v2.2 - Enterprise Edition
  Automated Cleanup, Deep Deduplication & Safe 100% Rollback
======================================================================{Color.RESET}
"""
    print(banner)


def print_success(msg: str) -> None:
    print(f"{Color.GREEN}[+] {msg}{Color.RESET}")


def print_info(msg: str) -> None:
    print(f"{Color.CYAN}[*] {msg}{Color.RESET}")


def print_warning(msg: str) -> None:
    print(f"{Color.YELLOW}[!] {msg}{Color.RESET}")


def print_error(msg: str) -> None:
    print(f"{Color.RED}[x] {msg}{Color.RESET}")


def print_progress(current: int, total: int, prefix: str = "", length: int = 30) -> None:
    """Renders an inline ASCII progress bar."""
    if total <= 0:
        return
    percent = float(current) / float(total)
    filled_len = int(length * percent)
    bar = "=" * filled_len + "-" * (length - filled_len)
    sys.stdout.write(f"\r{prefix} [{Color.CYAN}{bar}{Color.RESET}] {current}/{total} ({percent * 100:.1f}%)")
    sys.stdout.flush()
    if current >= total:
        sys.stdout.write("\n")
        sys.stdout.flush()


def print_table(headers: list, rows: list) -> None:
    """Renders a formatted ASCII table."""
    if not rows:
        return

    # Calculate column widths
    col_widths = [len(str(h)) for h in headers]
    for row in rows:
        for idx, cell in enumerate(row):
            if idx < len(col_widths):
                col_widths[idx] = max(col_widths[idx], len(str(cell)))

    # Format header
    header_str = " | ".join(f"{str(h):<{col_widths[i]}}" for i, h in enumerate(headers))
    sep_str = "-+-".join("-" * col_widths[i] for i in range(len(headers)))

    print(f"{Color.BOLD}{header_str}{Color.RESET}")
    print(sep_str)

    for row in rows:
        row_str = " | ".join(f"{str(cell):<{col_widths[i]}}" for i, cell in enumerate(row))
        print(row_str)
