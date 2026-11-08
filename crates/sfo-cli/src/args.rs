//! Command-line flags. Names, short forms, defaults and choices match the
//! Python v3.0 `argparse` parser so existing scripts keep working.

use clap::Parser;

const EXAMPLES: &str = r#"Examples:
  # Scan and organize downloads in dry-run mode (safest for preview):
  sfo --source "C:/Users/Raven/Downloads" --dry-run

  # Live organization by file extension:
  sfo --source "C:/Users/Raven/Downloads"

  # Organize files into Year/Month subfolders:
  sfo --source "C:/Photos" --by-date

  # Deep duplicate detection and cleanup:
  sfo --source "C:/Storage" --dedupe

  # Undo the latest file organization session:
  sfo --undo

  # Launch the Desktop Graphical Interface:
  sfo --gui

  # Monitor Downloads folder in background:
  sfo --source "C:/Users/Raven/Downloads" --watch
"#;

#[derive(Debug, Parser)]
#[command(
    name = "sfo",
    version,
    about = "Smart File Organizer (SFO) - Automated Cleanup, Deduplication & Rollback",
    after_help = EXAMPLES
)]
pub struct Args {
    // Core paths
    /// Source directory to scan and organize (default: current directory)
    #[arg(short = 's', long)]
    pub source: Option<String>,
    /// Destination directory for organized folders (default: same as source)
    #[arg(short = 'd', long)]
    pub dest: Option<String>,

    // Organization
    /// Simulate actions without moving, renaming, or deleting any files
    #[arg(short = 'n', long)]
    pub dry_run: bool,
    /// Strategy for grouping files
    #[arg(long, default_value = "extension", value_parser = ["extension", "date", "size", "regex"])]
    pub strategy: String,
    /// Shortcut to organize files by Date (YYYY/MM)
    #[arg(long)]
    pub by_date: bool,
    /// Shortcut to organize files by Size (Tiny, Medium, Large, Huge)
    #[arg(long)]
    pub by_size: bool,
    /// Recursively scan all subdirectories
    #[arg(short = 'r', long)]
    pub recursive: bool,

    // Deduplication
    /// Scan and resolve duplicate files using 3-tiered hashing
    #[arg(long)]
    pub dedupe: bool,
    /// Scan and cluster visually similar images using perceptual hashing (dHash)
    #[arg(long)]
    pub visual_dedupe: bool,
    /// Perceptual similarity threshold between 0.50 and 1.0
    #[arg(long, default_value_t = 0.90)]
    pub similarity: f64,
    /// Action for duplicate files
    #[arg(long, default_value = "move_to_duplicates", value_parser = ["move_to_duplicates", "delete", "report"])]
    pub dedupe_action: String,

    // Renaming
    /// Batch rename template with tokens like {name}, {counter:03d}, {date}, {camera}, {artist}
    #[arg(long)]
    pub rename_template: Option<String>,
    /// Substring or pattern to find for renaming
    #[arg(long, default_value = "")]
    pub rename_find: String,
    /// Replacement text for renaming
    #[arg(long, default_value = "")]
    pub rename_replace: String,
    /// Apply casing transformation during rename
    #[arg(long, value_parser = ["lowercase", "uppercase", "title_case", "snake_case", "kebab_case", "slugify"])]
    pub rename_case: Option<String>,

    /// Analyze text and auto-route invoices/receipts into Finance_<Year> folders
    #[arg(long)]
    pub smart_receipts: bool,

    // Sync & USB
    /// Source directory for folder synchronization / backup
    #[arg(long)]
    pub sync_source: Option<String>,
    /// Target backup or USB destination directory
    #[arg(long)]
    pub sync_dest: Option<String>,
    /// Synchronization mode
    #[arg(long, default_value = "incremental", value_parser = ["incremental", "mirror", "snapshot"])]
    pub sync_mode: String,
    /// Detect and list currently connected removable USB drives
    #[arg(long)]
    pub detect_usb: bool,

    /// Analyze directory disk usage and display top storage-eating files
    #[arg(long)]
    pub analyze_disk: bool,

    // Junk
    /// Scan for inactive node_modules, .venv, and build caches
    #[arg(long)]
    pub scan_junk: bool,
    /// Clean discovered developer junk and reclaim disk space
    #[arg(long)]
    pub clean_junk: bool,

    // Secrets
    /// Scan files for exposed API tokens, passwords, and private keys
    #[arg(long)]
    pub scan_secrets: bool,
    /// Move discovered compromised credential files to _Quarantine/ folder
    #[arg(long)]
    pub quarantine_secrets: bool,

    // Explorer
    /// Register Smart File Organizer in Windows Explorer right-click menu
    #[arg(long)]
    pub install_context_menu: bool,
    /// Remove Smart File Organizer from Windows Explorer right-click menu
    #[arg(long)]
    pub uninstall_context_menu: bool,

    // Shredder & vault
    /// Permanently destroy a file or directory using multi-pass overwriting
    #[arg(long, value_name = "PATH")]
    pub shred: Option<String>,
    /// Data wiping algorithm
    #[arg(long, default_value = "dod_3pass", value_parser = ["dod_3pass", "quick_1pass", "nist_800_88", "gutmann_7pass"])]
    pub shred_method: String,
    /// Encrypt and pack a file or folder into a .sfovault container
    #[arg(long, value_name = "SOURCE_PATH")]
    pub vault_lock: Option<String>,
    /// Decrypt and extract an .sfovault container to the destination directory
    #[arg(long, value_name = "VAULT_FILE")]
    pub vault_unlock: Option<String>,
    /// List files inside an .sfovault container without extracting to disk
    #[arg(long, value_name = "VAULT_FILE")]
    pub vault_inspect: Option<String>,
    /// Re-encrypt a legacy v1 .sfovault container with the current format
    #[arg(long, value_name = "VAULT_FILE")]
    pub vault_upgrade: Option<String>,
    /// Passphrase for vault encryption/decryption (prompted if omitted)
    #[arg(long)]
    pub vault_pass: Option<String>,
    /// Shred the original files after locking them into the vault
    #[arg(long)]
    pub vault_shred_source: bool,

    // Cold storage
    /// Compress cold/inactive files into a ZIP archive
    #[arg(long)]
    pub archive_cold: bool,
    /// Inactivity threshold in days for cold storage
    #[arg(long, default_value_t = 90)]
    pub archive_days: u64,
    /// Destination path for cold storage archive ZIP
    #[arg(long)]
    pub archive_dest: Option<String>,
    /// Delete original files after successful archive compression
    #[arg(long)]
    pub archive_delete_sources: bool,
    /// Restore/extract a previously created SFO archive ZIP
    #[arg(long)]
    pub restore_archive: Option<String>,

    // Cloud
    /// Detect and list installed cloud storage folders (OneDrive, Google Drive, Dropbox, iCloud)
    #[arg(long)]
    pub detect_cloud: bool,
    /// Sync source directory to a detected cloud provider (e.g. 'OneDrive', 'Google Drive', 'Dropbox')
    #[arg(long)]
    pub sync_cloud: Option<String>,

    // LAN Drop
    /// Launch LAN Drop server for wireless file exchange across phones and PCs
    #[arg(long)]
    pub lan_share: bool,
    /// Port number for LAN Drop server
    #[arg(long, default_value_t = 8080)]
    pub lan_port: u16,
    /// Optional access PIN for LAN Drop server
    #[arg(long)]
    pub lan_pin: Option<String>,

    // Diagnostics & system
    /// Run SFO Doctor system health, latency & permission diagnostics
    #[arg(long)]
    pub doctor: bool,
    /// Select language localization (en: English, vi: Vietnamese / Tieng Viet)
    #[arg(long, value_parser = ["en", "vi"])]
    pub lang: Option<String>,
    /// Start Smart File Organizer automatically on Windows login
    #[arg(long)]
    pub enable_autostart: bool,
    /// Remove Smart File Organizer from Windows startup
    #[arg(long)]
    pub disable_autostart: bool,
    /// Verify the SHA-256 audit log chain integrity
    #[arg(long)]
    pub verify_audit: bool,
    /// Schedule a daily background cleanup in Windows Task Scheduler (e.g. '18:00')
    #[arg(long, value_name = "HH:MM")]
    pub schedule_daily: Option<String>,
    /// List all SFO scheduled tasks in Windows Task Scheduler
    #[arg(long)]
    pub list_tasks: bool,
    /// Remove a scheduled task from Windows Task Scheduler
    #[arg(long)]
    pub delete_task: Option<String>,

    // Undo & history
    /// Rollback an organization session (provide session ID or omit for latest)
    #[arg(long, num_args = 0..=1, default_missing_value = "latest", value_name = "SESSION_ID")]
    pub undo: Option<String>,
    /// List all previous organization sessions
    #[arg(long)]
    pub history: bool,

    /// Run continuously in background, auto-organizing new downloads
    #[arg(short = 'w', long)]
    pub watch: bool,

    /// Path to generate standalone HTML dashboard report
    #[arg(long)]
    pub report: Option<String>,
    /// Custom configuration JSON file path
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// Launch the Desktop GUI
    #[arg(short = 'g', long)]
    pub gui: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn python_style_flags_parse() {
        let a = Args::try_parse_from(["sfo", "-s", "x", "-n", "-r", "--undo"]).unwrap();
        assert_eq!(a.source.as_deref(), Some("x"));
        assert!(a.dry_run && a.recursive);
        assert_eq!(a.undo.as_deref(), Some("latest"));
        let a = Args::try_parse_from(["sfo", "--undo", "sfo_20261108_101452_e020d0"]).unwrap();
        assert_eq!(a.undo.as_deref(), Some("sfo_20261108_101452_e020d0"));
        assert!(Args::try_parse_from(["sfo", "--strategy", "nope"]).is_err());
        let a = Args::try_parse_from(["sfo"]).unwrap();
        assert_eq!((a.similarity, a.archive_days, a.lan_port), (0.90, 90, 8080));
    }
}
