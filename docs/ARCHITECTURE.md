# System Architecture & Technical Design

## 1. Architectural Principles
- **Decoupled Layered Architecture:** Clear boundary between Presentation (CLI / GUI) and Domain Core.
- **Zero-Dependency Mandate:** The entire application core runs on Python Standard Library (`pathlib`, `shutil`, `hashlib`, `sqlite3`, `json`, `tkinter`, `argparse`, `threading`).
- **Transactional Integrity:** File system mutations are recorded in an append-only journal in SQLite and JSON.

```
+----------------------------------------------------------------------------------+
|                            SMART FILE ORGANIZER v2.0                             |
+----------------------------------------------------------------------------------+
|                                  INTERFACES                                      |
|  smart_file_organizer/gui/                        smart_file_organizer/cli/      |
|  - app.py (Tkinter Dark UI)                       - parser.py (argparse)         |
|  - components.py (StatCard, Treeview)             - formatter.py (ANSI tables)   |
|  - theme.py (Modern Slate Theme)                  - app.py (CLI Router)          |
+----------------------------------------------------------------------------------+
                                        |
                                        v
+----------------------------------------------------------------------------------+
|                                CORE ENGINE                                       |
|  smart_file_organizer/core/                                                      |
|  ├── scanner.py       -> Fast directory traversal with pattern exclusion         |
|  ├── hasher.py        -> 3-tiered chunked hasher (Size -> 64KB -> SHA-256)       |
|  ├── deduplicator.py  -> Duplicate grouping & keep policy selection              |
|  ├── organizer.py     -> Multi-strategy rule engine (Ext, Date, Size, Regex)     |
|  ├── executor.py      -> Safe execution, dry-run simulation & collision solver   |
|  ├── undo_manager.py  -> SQLite & JSON journaling, 100% rollback engine          |
|  ├── watcher.py       -> Debounced directory daemon                              |
|  ├── reporter.py      -> HTML Dashboard, Markdown & Console summaries            |
|  ├── config.py        -> Configuration loader with fallback defaults             |
|  └── models.py        -> Dataclasses: FileInfo, ActionRecord, SessionLog         |
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
