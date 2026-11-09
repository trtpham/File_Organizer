# Smart File Organizer CLI Reference Guide

## Syntax Overview
```bash
sfo [OPTIONS]
```

On Windows the program is `sfo.exe`. All examples work the same on Linux and macOS with POSIX paths.

## Available Command-Line Arguments

| Flag | Long Option | Description | Default |
| :--- | :--- | :--- | :--- |
| `-s` | `--source PATH` | Directory to scan and organize | Current Directory |
| `-d` | `--dest PATH` | Target destination directory | Same as `--source` |
| `-n` | `--dry-run` | Safe preview mode; no files are moved | `False` |
| | `--strategy STRAT` | Strategy: `extension`, `date`, `size`, `regex` | `extension` |
| | `--by-date` | Shortcut for organizing files into `YYYY/MM` | `False` |
| | `--by-size` | Shortcut for organizing files by size ranges | `False` |
| `-r` | `--recursive` | Recursively scan nested subdirectories | `False` |
| | `--smart-receipts` | Smart financial sort for invoices and receipts | `False` |
| | `--dedupe` | Exact duplicate file detection (SHA-256) | `False` |
| | `--visual-dedupe` | Perceptual visual duplicate image detection (pHash) | `False` |
| | `--similarity NUM` | Similarity threshold for pHash (0.50 - 1.0) | `0.90` |
| | `--dedupe-action ACT`| Action for duplicates: `move_to_duplicates`, `delete`, `report` | `move_to_duplicates` |
| | `--rename-template TPL`| Batch renaming template with metadata tokens | `None` |
| | `--rename-find STR` | Substring or pattern to find for renaming | `""` |
| | `--rename-replace STR` | Replacement string for renaming | `""` |
| | `--rename-case CASE` | Casing transform: `lowercase`, `uppercase`, `title_case`, `snake_case`, `kebab_case`, `slugify` | `None` |
| | `--detect-usb` | Detect and list currently attached removable USB drives | `False` |
| | `--sync-source PATH` | Source directory for backup synchronization | `None` |
| | `--sync-dest PATH` | Destination directory or USB path for backup | `None` |
| | `--sync-mode MODE` | Synchronization mode: `incremental`, `mirror`, `snapshot` | `incremental` |
| | `--analyze-disk` | Analyze storage usage and top space-eating files | `False` |
| | `--scan-junk` | Scan for disposable developer bloat (`node_modules`, caches) | `False` |
| | `--clean-junk` | Delete discovered developer junk directories | `False` |
| | `--scan-secrets` | Scan files for exposed API credentials & private keys | `False` |
| | `--quarantine-secrets` | Move compromised credential files to `_Quarantine/` | `False` |
| | `--install-context-menu` | Register right-click context menu in Windows Explorer | `False` |
| | `--uninstall-context-menu` | Remove right-click context menu from Windows Explorer | `False` |
| | `--shred PATH` | Overwrite and delete a file or a whole folder (irreversible) | `None` |
| | `--shred-method M` | `dod_3pass`, `quick_1pass`, `nist_800_88`, `gutmann_7pass` | `dod_3pass` |
| | `--vault-lock PATH` | Pack and encrypt a file or folder into a `.sfovault` (`--dest` sets the output) | `None` |
| | `--vault-unlock FILE` | Decrypt a vault into `--dest` (default: current folder) | `None` |
| | `--vault-inspect FILE` | List a vault's contents without extracting | `None` |
| | `--vault-upgrade FILE` | Re-encrypt a v3.0 (v1) vault with the v4 format | `None` |
| | `--vault-pass PASS` | Vault passphrase (prompted, hidden, when omitted) | prompt |
| | `--vault-shred-source` | Shred the source after locking it into the vault | `False` |
| | `--archive-cold` | Compress cold/inactive files into a ZIP archive | `False` |
| | `--archive-days DAYS` | Inactivity age threshold in days for cold storage | `90` |
| | `--archive-dest PATH` | Custom destination path for archive ZIP | `None` |
| | `--archive-delete-sources` | Delete original files after successful archive creation | `False` |
| | `--restore-archive FILE`| Restore/extract a previously created SFO archive ZIP | `None` |
| | `--detect-cloud` | Detect and list installed cloud folders (OneDrive, GDrive, Dropbox, iCloud) | `False` |
| | `--sync-cloud NAME` | Sync source directory to detected cloud provider | `None` |
| | `--lan-share` | Launch local Wi-Fi wireless file exchange server | `False` |
| | `--lan-port PORT` | Port for LAN Drop server | `8080` |
| | `--lan-pin PIN` | Optional access PIN for LAN Drop server | `None` |
| | `--doctor` | Run comprehensive SFO Doctor health & latency diagnostics | `False` |
| | `--lang CODE` | Language localization: `en` (English) or `vi` (Tiếng Việt) | `en` |
| | `--enable-autostart` | Enable Windows startup on login | `False` |
| | `--disable-autostart` | Remove application from Windows startup | `False` |
| | `--verify-audit` | Verify cryptographic SHA-256 audit log integrity | `False` |
| | `--schedule-daily HH:MM`| Schedule automated daily background cleanup via schtasks | `None` |
| | `--list-tasks` | List all active SFO scheduled tasks in Windows Task Scheduler | `False` |
| | `--delete-task NAME` | Remove a scheduled task from Windows Task Scheduler | `None` |
| | `--undo [ID]` | Rollback session (`latest` if ID is omitted) | `None` |
| | `--history` | List all previous organization sessions | `False` |
| `-w` | `--watch` | Run background watcher daemon with debounce | `False` |
| | `--report FILE` | Generate standalone HTML dashboard report | `None` |
| `-c` | `--config FILE` | Path to custom `config.json` | Default config |
| | `--force` | Run a live operation even when it exceeds the blast-radius limit | `False` |
| `-g` | `--gui` | Launch the desktop app (`sfo-gui`, must sit next to `sfo`) | `False` |
| `-h` | `--help` | Show all options | |
| `-V` | `--version` | Show the version | |

Live operations (organize, dedupe, rename, `--clean-junk`, `--archive-delete-sources`, shredding a folder) stop with exit code 1 and change nothing when they would touch more than `blast_radius.max_files` files (default 500) or `blast_radius.max_bytes` bytes (default 10 GB); preview with `--dry-run`, then add `--force`. Set the limits in your `config.json`.

Running `sfo` with no arguments opens the desktop app. Invalid options exit
with code 2; failed operations exit with code 1.

### Environment variables

| Variable | Effect |
| :--- | :--- |
| `SFO_DATA_DIR` | Folder for history, journals, audit log and user config (default `~/.smart_file_organizer`) |
| `NO_COLOR` | Disable coloured output (also automatic when output is piped) |

---

## Practical Examples

### 1. Smart Bulk Renaming with Metadata & Counter
```bash
# Rename photos with camera model, date taken, and counter:
sfo --source "D:/Photos" --rename-template "{date}_{camera}_{counter:03d}"

# Snake-case filenames with find and replace:
sfo --source "D:/Project" --rename-template "{name}" --rename-case snake_case --rename-find "draft" --rename-replace "release"
```

### 2. Perceptual Visual Duplicate Detection (pHash)
```bash
# Find visually similar or resized images:
sfo --source "D:/Photos" --visual-dedupe --similarity 0.90 --dedupe-action move_to_duplicates
```

### 3. Smart Receipt & Invoice Organization
```bash
# Automatically route electricity, water, and online invoices into Finance_2026/
sfo --source "C:/Users/Raven/Downloads" --smart-receipts
```

### 4. USB Drive Detection & Incremental Backup
```bash
# Check connected USB drives:
sfo --detect-usb

# Incrementally sync documents or game saves:
sfo --sync-source "C:/Users/Raven/Documents" --sync-dest "E:/USB_Backup" --sync-mode incremental

# Create timestamped zip snapshot:
sfo --sync-source "D:/SaveGames" --sync-dest "E:/Backups" --sync-mode snapshot
```

### 5. Safe Dry-Run Preview
```bash
sfo --source "C:/Users/Raven/Downloads" --dry-run
```

### 6. Storage Analysis & Largest Files
```bash
# Analyze disk usage distribution and find top storage-eating files:
sfo --source "C:/Users/Raven" --analyze-disk
```

### 7. Developer Junk Cleaner
```bash
# Scan for inactive node_modules, .venv, and build caches:
sfo --source "D:/Workspaces" --scan-junk

# Permanently delete discovered junk directories:
sfo --source "D:/Workspaces" --clean-junk
```

### 8. Secret Scanner (DLP) & File Quarantine
```bash
# Scan directory for exposed API keys, private keys, and connection strings:
sfo --source "D:/Projects" --scan-secrets

# Quarantine compromised files into _Quarantine/ folder:
sfo --source "D:/Projects" --quarantine-secrets
```

### 9. Windows Explorer Shell Integration
```bash
# Register right-click context menu under HKCU (no Admin required):
sfo --install-context-menu

# Unregister right-click context menu:
sfo --uninstall-context-menu
```

### 10. Cold Storage Archiving & Tiered Compression
```bash
# Automatically compress files inactive for >= 90 days into an archive:
sfo --source "D:/Archives" --archive-cold --archive-days 90

# Compress cold files and delete originals to reclaim disk space:
sfo --source "D:/Archives" --archive-cold --archive-days 180 --archive-delete-sources

# Restore/extract a previously created SFO archive:
sfo --restore-archive "D:/Archives/Archive_20261027.zip" --dest "D:/Restored"
```

### 11. Cloud Storage Sync (OneDrive, Google Drive, Dropbox)
```bash
# Detect and list active cloud storage directories:
sfo --detect-cloud

# Sync important documents or finance files directly to OneDrive:
sfo --source "C:/Users/Raven/Documents" --sync-cloud onedrive
```

### 12. Wireless Cross-Device LAN Drop
```bash
# Start local Wi-Fi file exchange server on default port 8080:
sfo --source "C:/Users/Raven/Downloads" --lan-share

# Start LAN server with custom port and 4-digit PIN:
sfo --source "D:/Share" --lan-share --lan-port 9000 --lan-pin 4821
```

### 13. SFO Doctor System Diagnostics
```bash
# Run comprehensive workstation disk latency, permission, and health checks:
sfo --doctor
```

### 14. Cryptographic Audit Chain Verification
```bash
# Mathematically verify that the tamper-evident SHA-256 audit log has not been altered:
sfo --verify-audit
```

### 15. Native Windows Task Scheduler Automation
```bash
# Schedule daily automated organization at 18:00 (runs silently even when GUI is closed):
sfo --source "C:/Users/Raven/Downloads" --schedule-daily 18:00

# View all registered SFO automated tasks:
sfo --list-tasks

# Delete scheduled task:
sfo --delete-task DailySweep
```

### 16. Multi-Language CLI Output (Vietnamese)
```bash
# Run commands with Vietnamese localization:
sfo --source "C:/Users/Raven/Downloads" --dry-run --lang vi
```

### 17. Undo Previous Operation (100% Rollback)
```bash
sfo --undo
```

### 18. Encrypted Vault and Secure Shredding
```bash
# Lock a folder (passphrase prompted), then shred the originals:
sfo --vault-lock "D:/Tax/2026" --dest "D:/Tax_2026.sfovault" --vault-shred-source

# Look inside, then unlock elsewhere:
sfo --vault-inspect "D:/Tax_2026.sfovault"
sfo --vault-unlock "D:/Tax_2026.sfovault" --dest "D:/Restored"

# Upgrade a vault created by v3.0:
sfo --vault-upgrade "D:/Old.sfovault"

# Irreversibly wipe a file:
sfo --shred "D:/old_contract.pdf" --shred-method dod_3pass
```
