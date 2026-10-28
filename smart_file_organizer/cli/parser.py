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

    # Storage Analyzer & Treemap
    parser.add_argument(
        "--analyze-disk",
        action="store_true",
        help="Analyze directory disk usage and display top storage-eating files"
    )

    # Developer Junk Sweeper
    parser.add_argument(
        "--scan-junk",
        action="store_true",
        help="Scan for inactive node_modules, .venv, and build caches"
    )
    parser.add_argument(
        "--clean-junk",
        action="store_true",
        help="Clean discovered developer junk and reclaim disk space"
    )

    # Sensitive Data & Secret Scanner (DLP)
    parser.add_argument(
        "--scan-secrets",
        action="store_true",
        help="Scan files for exposed API tokens, passwords, and private keys"
    )
    parser.add_argument(
        "--quarantine-secrets",
        action="store_true",
        help="Move discovered compromised credential files to _Quarantine/ folder"
    )

    # Windows Explorer Integration
    parser.add_argument(
        "--install-context-menu",
        action="store_true",
        help="Register Smart File Organizer in Windows Explorer right-click menu"
    )
    parser.add_argument(
        "--uninstall-context-menu",
        action="store_true",
        help="Remove Smart File Organizer from Windows Explorer right-click menu"
    )

    # Military-Grade Secure Shredder & Encrypted Vault
    parser.add_argument(
        "--shred",
        type=str,
        default=None,
        metavar="PATH",
        help="Permanently destroy a file or directory using DoD 5220.22-M multi-pass overwriting"
    )
    parser.add_argument(
        "--shred-method",
        choices=["dod_3pass", "quick_1pass", "nist_800_88", "gutmann_7pass"],
        default="dod_3pass",
        help="Data wiping algorithm (default: dod_3pass)"
    )
    parser.add_argument(
        "--vault-lock",
        type=str,
        default=None,
        metavar="SOURCE_PATH",
        help="Encrypt and pack file or folder into an AES-256-GCM .sfovault container"
    )
    parser.add_argument(
        "--vault-unlock",
        type=str,
        default=None,
        metavar="VAULT_FILE",
        help="Decrypt and extract an .sfovault container to destination directory"
    )
    parser.add_argument(
        "--vault-inspect",
        type=str,
        default=None,
        metavar="VAULT_FILE",
        help="List files inside an .sfovault container without extracting to disk"
    )
    parser.add_argument(
        "--vault-pass",
        type=str,
        default=None,
        help="Passphrase for vault encryption/decryption (prompted if omitted)"
    )
    parser.add_argument(
        "--vault-shred-source",
        action="store_true",
        help="Execute military-grade DoD shredding of original files after locking into vault"
    )

    # Cold Storage & Auto-Archive
    parser.add_argument(
        "--archive-cold",
        action="store_true",
        help="Compress cold/inactive files into a ZIP archive"
    )
    parser.add_argument(
        "--archive-days",
        type=int,
        default=90,
        help="Inactivity threshold in days for cold storage (default: 90)"
    )
    parser.add_argument(
        "--archive-dest",
        type=str,
        default=None,
        help="Destination path for cold storage archive ZIP"
    )
    parser.add_argument(
        "--archive-delete-sources",
        action="store_true",
        help="Delete original files after successful archive compression"
    )
    parser.add_argument(
        "--restore-archive",
        type=str,
        default=None,
        help="Restore/extract a previously created SFO archive ZIP"
    )

    # Cloud Storage Connector
    parser.add_argument(
        "--detect-cloud",
        action="store_true",
        help="Detect and list installed cloud storage folders (OneDrive, Google Drive, Dropbox, iCloud)"
    )
    parser.add_argument(
        "--sync-cloud",
        type=str,
        default=None,
        help="Sync source directory to a detected cloud provider (e.g. 'OneDrive', 'Google Drive', 'Dropbox')"
    )

    # Cross-Device LAN Share (LAN Drop)
    parser.add_argument(
        "--lan-share",
        action="store_true",
        help="Launch background LAN Drop server for wireless file exchange across phones and PCs"
    )
    parser.add_argument(
        "--lan-port",
        type=int,
        default=8080,
        help="Port number for LAN Drop server (default: 8080)"
    )
    parser.add_argument(
        "--lan-pin",
        type=str,
        default=None,
        help="Optional access PIN for LAN Drop server"
    )

    # Enterprise & System Diagnostics
    parser.add_argument(
        "--doctor",
        action="store_true",
        help="Run comprehensive SFO Doctor system health, latency & permission diagnostics"
    )
    parser.add_argument(
        "--lang",
        choices=["en", "vi"],
        default=None,
        help="Select language localization (en: English, vi: Vietnamese / Tieng Viet)"
    )
    parser.add_argument(
        "--enable-autostart",
        action="store_true",
        help="Configure Smart File Organizer to start automatically on Windows boot"
    )
    parser.add_argument(
        "--disable-autostart",
        action="store_true",
        help="Remove Smart File Organizer from Windows boot startup"
    )
    parser.add_argument(
        "--verify-audit",
        action="store_true",
        help="Mathematically verify cryptographic SHA-256 audit log chain integrity"
    )
    parser.add_argument(
        "--schedule-daily",
        type=str,
        default=None,
        metavar="HH:MM",
        help="Schedule daily automated background cleanup in Windows Task Scheduler (e.g. '18:00')"
    )
    parser.add_argument(
        "--list-tasks",
        action="store_true",
        help="List all active SFO scheduled tasks in Windows Task Scheduler"
    )
    parser.add_argument(
        "--delete-task",
        type=str,
        default=None,
        help="Remove a scheduled task from Windows Task Scheduler"
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
