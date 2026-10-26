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
| | `--dedupe` | Run 3-tiered duplicate file detection | `False` |
| | `--dedupe-action` | Action for duplicates: `move_to_duplicates`, `delete`, `report` | `move_to_duplicates` |
| | `--undo [ID]` | Rollback session (`latest` if ID is omitted) | `None` |
| | `--history` | List all previous organization sessions | `False` |
| `-w` | `--watch` | Run background watcher daemon with debounce | `False` |
| | `--report FILE` | Generate standalone HTML dashboard report | `None` |
| `-c` | `--config FILE` | Path to custom `config.json` | Default config |
| `-g` | `--gui` | Launch Desktop Graphical User Interface | `False` |

---

## Practical Examples

### 1. Dry-Run Organization (Preview Only)
```bash
python main.py --source "C:/Users/Raven/Downloads" --dry-run
```

### 2. Live File Organization by Type
```bash
python main.py --source "C:/Users/Raven/Downloads"
```

### 3. Organize Photos by Year and Month
```bash
python main.py --source "D:/RawPhotos" --dest "D:/OrganizedPhotos" --by-date
```

### 4. Deep Deduplication with Duplicates Quarantine
```bash
python main.py --source "D:/MessyDrive" --dedupe --dedupe-action move_to_duplicates
```

### 5. Undo the Latest Operation
```bash
python main.py --undo
```

### 6. Background Watcher for Automated Downloads Cleanup
```bash
python main.py --source "C:/Users/Raven/Downloads" --watch
```
