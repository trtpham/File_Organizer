# Smart File Organizer CLI Reference Guide

## Syntax Overview
```bash
python main.py [OPTIONS]
```

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
| | `--rename-case CASE` | Casing transform: `lowercase`, `uppercase`, `title_case`, `snake_case`, `kebab_case` | `None` |
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
| | `--undo [ID]` | Rollback session (`latest` if ID is omitted) | `None` |
| | `--history` | List all previous organization sessions | `False` |
| `-w` | `--watch` | Run background watcher daemon with debounce | `False` |
| | `--report FILE` | Generate standalone HTML dashboard report | `None` |
| `-c` | `--config FILE` | Path to custom `config.json` | Default config |
| `-g` | `--gui` | Launch Desktop Graphical User Interface | `False` |

---

## Practical Examples

### 1. Smart Bulk Renaming with Metadata & Counter
```bash
# Rename photos with camera model, date taken, and counter:
python main.py --source "D:/Photos" --rename-template "{date}_{camera}_{counter:03d}"

# Snake-case filenames with find and replace:
python main.py --source "D:/Project" --rename-template "{name}" --rename-case snake_case --rename-find "draft" --rename-replace "release"
```

### 2. Perceptual Visual Duplicate Detection (pHash)
```bash
# Find visually similar or resized images:
python main.py --source "D:/Photos" --visual-dedupe --similarity 0.90 --dedupe-action move_to_duplicates
```

### 3. Smart Receipt & Invoice Organization
```bash
# Automatically route electricity, water, and online invoices into Finance_2026/
python main.py --source "C:/Users/Raven/Downloads" --smart-receipts
```

### 4. USB Drive Detection & Incremental Backup
```bash
# Check connected USB drives:
python main.py --detect-usb

# Incrementally sync documents or game saves:
python main.py --sync-source "C:/Users/Raven/Documents" --sync-dest "E:/USB_Backup" --sync-mode incremental

# Create timestamped zip snapshot:
python main.py --sync-source "D:/SaveGames" --sync-dest "E:/Backups" --sync-mode snapshot
```

### 5. Safe Dry-Run Preview
```bash
python main.py --source "C:/Users/Raven/Downloads" --dry-run
```

### 6. Storage Analysis & Largest Files
```bash
# Analyze disk usage distribution and find top storage-eating files:
python main.py --source "C:/Users/Raven" --analyze-disk
```

### 7. Developer Junk Cleaner
```bash
# Scan for inactive node_modules, .venv, and build caches:
python main.py --source "D:/Workspaces" --scan-junk

# Permanently delete discovered junk directories:
python main.py --source "D:/Workspaces" --clean-junk
```

### 8. Secret Scanner (DLP) & File Quarantine
```bash
# Scan directory for exposed API keys, private keys, and connection strings:
python main.py --source "D:/Projects" --scan-secrets

# Quarantine compromised files into _Quarantine/ folder:
python main.py --source "D:/Projects" --quarantine-secrets
```

### 9. Windows Explorer Shell Integration
```bash
# Register right-click context menu under HKCU (no Admin required):
python main.py --install-context-menu

# Unregister right-click context menu:
python main.py --uninstall-context-menu
```

### 10. Undo Previous Operation (100% Rollback)
```bash
python main.py --undo
```
