# System Architecture & Technical Design

## 1. Architectural Principles
- **Decoupled Layered Architecture:** Clear boundary between Presentation (CLI / GUI) and Domain Core.
- **Zero-Dependency Mandate:** The entire application core runs on Python Standard Library (`pathlib`, `shutil`, `hashlib`, `sqlite3`, `json`, `tkinter`, `argparse`, `threading`).
- **Transactional Integrity:** File system mutations are recorded in an append-only journal in SQLite and JSON.

```
+----------------------------------------------------------------------------------+
|                 SMART FILE ORGANIZER v3.0 - COMMERCIAL EDITION                   |
+----------------------------------------------------------------------------------+
|                                  INTERFACES                                      |
|  smart_file_organizer/gui/                        smart_file_organizer/cli/      |
|  - app.py (9-Tab Localized Tkinter Dark UI)       - parser.py (argparse CLI)     |
|  - components.py (StatCard, Treeview)             - formatter.py (ANSI tables)   |
|  - theme.py (Modern Slate Theme)                  - app.py (CLI Router)          |
+----------------------------------------------------------------------------------+
                                        |
                                        v
+----------------------------------------------------------------------------------+
|                         ENTERPRISE & PLATFORM SERVICES                           |
|  ├── enterprise_guardrails.py -> Path blacklist, blast radius & SHA-256 audit log|
|  ├── doctor.py                -> System health & disk I/O latency diagnostics    |
|  ├── i18n.py                  -> Multilingual localization (English & Tiếng Việt)|
|  ├── tray_daemon.py           -> Windows Startup (HKCU Run) & Toast Notifications|
|  └── scheduler.py             -> Windows Task Scheduler (schtasks) integration   |
+----------------------------------------------------------------------------------+
                                        |
                                        v
+----------------------------------------------------------------------------------+
|                                CORE ENGINE                                       |
|  smart_file_organizer/core/                                                      |
|  ├── scanner.py          -> Fast traversal with pattern exclusion (.git, etc.)   |
|  ├── hasher.py           -> 3-tiered chunked hasher (Size -> 64KB -> SHA-256)    |
|  ├── visual_hasher.py    -> Perceptual difference hasher (dHash) & similarity   |
|  ├── deduplicator.py     -> Duplicate grouping (Exact & Visual) & resolution    |
|  ├── renamer.py          -> Smart Bulk Renamer (EXIF, ID3, Templates, Casing)   |
|  ├── syncer.py           -> USB detector, incremental sync & snapshot archiver  |
|  ├── archiver.py         -> Cold storage analyzer, compression & restore engine  |
|  ├── cloud_connector.py  -> Auto-detect OneDrive, GDrive, Dropbox & cloud sync  |
|  ├── lan_share.py        -> Embedded HTTP LAN Drop server for wireless transfers |
|  ├── treemap.py          -> Disk usage tree analyzer & 2D treemap layout engine |
|  ├── junk_cleaner.py     -> Inactive dev bloat cleaner (node_modules, caches)   |
|  ├── secret_scanner.py   -> DLP secret detector (AWS, OpenAI, GitHub, SSH)      |
|  ├── shell_integration.py-> Windows Explorer HKCU right-click context menu      |
|  ├── smart_filter.py     -> PDF text extractor & financial invoice classifier   |
|  ├── organizer.py        -> Multi-strategy engine (Ext, Date, Size, Receipts)   |
|  ├── executor.py         -> Safe execution, dry-run simulation & collisions     |
|  ├── undo_manager.py     -> SQLite & JSON journaling, 100% rollback engine       |
|  ├── watcher.py          -> Debounced directory daemon & USB auto-sync          |
|  ├── reporter.py         -> HTML Dashboard, Markdown & Console summaries         |
|  ├── config.py           -> Configuration loader with fallback defaults          |
|  └── models.py           -> Dataclasses: FileInfo, ActionRecord, SessionLog      |
+----------------------------------------------------------------------------------+
```

## 2. Tiered Deduplication Algorithm
Traditional deduplication computes SHA-256 over all candidate files, incurring intense disk I/O.
Smart File Organizer uses a 3-tier filtration pipeline:

1. **Tier 1 (Size Grouping):**
   - Files are grouped by exact byte size `st_size`.
   - Singletons (unique sizes) are discarded from duplicate evaluation immediately with $0$ disk read operations.
2. **Tier 2 (Fast Partial Chunk Hashing):**
   - Candidate pairs read only the first $64\text{ KB}$ (`DEFAULT_PARTIAL_BYTES`).
   - If initial bytes differ, files are discarded.
3. **Tier 3 (Full Cryptographic SHA-256 Verification):**
   - Only surviving candidates are streamed in $1\text{ MB}$ buffers to compute the full SHA-256 digest.

## 3. Transaction Journaling & Rollback Safety
Every session generates a unique ID `sfo_YYYYMMDD_HHMMSS_<hex>` and records each file operation into:
- An SQLite database: `~/.smart_file_organizer/history.db`
- A JSON journal: `~/.smart_file_organizer/journals/<session_id>.json`

During rollback:
- Operations are reversed in LIFO (reverse) order.
- Destination files are moved back to their original recorded source paths.
- Directories that became empty as a result of the rollback are automatically pruned.
