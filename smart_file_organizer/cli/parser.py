"""
Command-line argument parser for Smart File Organizer.
"""

import argparse
from pathlib import Path


def create_parser() -> argparse.ArgumentParser:
    """Builds and returns the complete argument parser."""
    parser = argparse.ArgumentParser(
        prog="sfo",
        description="Smart File Organizer (SFO) - Automated Cleanup, Deduplication & Rollback",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Scan and organize downloads in dry-run mode (safest for preview):
  python main.py --source "C:/Users/Raven/Downloads" --dry-run

  # Live organization by file extension:
  python main.py --source "C:/Users/Raven/Downloads"

  # Organize files into Year/Month subfolders:
  python main.py --source "C:/Photos" --by-date

  # Deep duplicate detection and cleanup:
  python main.py --source "C:/Storage" --dedupe

  # Undo the latest file organization session:
  python main.py --undo

  # Launch the Desktop Graphical Interface:
  python main.py --gui

  # Monitor Downloads folder in background:
  python main.py --source "C:/Users/Raven/Downloads" --watch
        """
    )

    # Core path arguments
    parser.add_argument(
        "-s", "--source",
        type=str,
        default=None,
        help="Source directory to scan and organize (default: current directory)"
    )
    parser.add_argument(
        "-d", "--dest",
        type=str,
        default=None,
        help="Destination directory for organized folders (default: same as source)"
    )

    # Organization mode and strategies
    parser.add_argument(
        "-n", "--dry-run",
        action="store_true",
        help="Simulate actions without moving, renaming, or deleting any files"
    )
    parser.add_argument(
        "--strategy",
        choices=["extension", "date", "size", "regex"],
        default="extension",
        help="Strategy for grouping files (default: extension)"
    )
    parser.add_argument(
        "--by-date",
        action="store_true",
        help="Shortcut to organize files by Date (YYYY/MM)"
    )
    parser.add_argument(
        "--by-size",
        action="store_true",
        help="Shortcut to organize files by Size (Tiny, Medium, Large, Huge)"
    )
    parser.add_argument(
        "-r", "--recursive",
        action="store_true",
        help="Recursively scan all subdirectories"
    )

    # Deduplication arguments
    parser.add_argument(
        "--dedupe",
        action="store_true",
        help="Scan and resolve duplicate files using 3-tiered hashing"
    )
    parser.add_argument(
        "--visual-dedupe",
        action="store_true",
        help="Scan and cluster visually similar images using perceptual hashing (dHash)"
    )
    parser.add_argument(
        "--similarity",
        type=float,
        default=0.90,
        help="Perceptual similarity threshold between 0.50 and 1.0 (default: 0.90)"
    )
    parser.add_argument(
        "--dedupe-action",
        choices=["move_to_duplicates", "delete", "report"],
        default="move_to_duplicates",
        help="Action for duplicate files (default: move_to_duplicates)"
    )

    # Smart Bulk Renaming
    parser.add_argument(
        "--rename-template",
        type=str,
        default=None,
        help="Batch rename template with tokens like {name}, {counter:03d}, {date}, {camera}, {artist}"
    )
    parser.add_argument(
        "--rename-find",
        type=str,
        default="",
        help="Substring or pattern to find for renaming"
    )
    parser.add_argument(
        "--rename-replace",
        type=str,
        default="",
        help="Replacement text for renaming"
    )
    parser.add_argument(
        "--rename-case",
        choices=["lowercase", "uppercase", "title_case", "snake_case", "kebab_case"],
        default=None,
        help="Apply casing transformation during rename"
    )

    # Smart Receipt & Financial Filtering
    parser.add_argument(
        "--smart-receipts",
        action="store_true",
        help="Analyze text and auto-route invoices/receipts into Finance_<Year> folders"
    )

    # Auto-Sync & USB Backup
    parser.add_argument(
        "--sync-source",
        type=str,
        default=None,
        help="Source directory for folder synchronization / backup"
    )
    parser.add_argument(
        "--sync-dest",
        type=str,
        default=None,
        help="Target backup or USB destination directory"
    )
    parser.add_argument(
        "--sync-mode",
        choices=["incremental", "mirror", "snapshot"],
        default="incremental",
        help="Synchronization mode (default: incremental)"
    )
    parser.add_argument(
        "--detect-usb",
        action="store_true",
        help="Detect and list currently connected removable USB drives"
    )

    # Undo and history
    parser.add_argument(
        "--undo",
        nargs="?",
        const="latest",
        default=None,
        help="Rollback an organization session (provide session ID or omit for latest)"
    )
    parser.add_argument(
        "--history",
        action="store_true",
        help="List all previous organization sessions"
    )

    # Watcher & Daemon
    parser.add_argument(
        "-w", "--watch",
        action="store_true",
        help="Run continuously in background, auto-organizing new downloads"
    )

    # Reporting and Configuration
    parser.add_argument(
        "--report",
        type=str,
        default=None,
        help="Path to generate standalone HTML dashboard report"
    )
    parser.add_argument(
        "-c", "--config",
        type=str,
        default=None,
        help="Custom configuration JSON file path"
    )

    # GUI
    parser.add_argument(
        "-g", "--gui",
        action="store_true",
        help="Launch the Modern Desktop GUI"
    )

    return parser
