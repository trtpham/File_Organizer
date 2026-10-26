# Smart File Organizer 📂 (Enterprise Edition v2.0)

[![Python Version](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Tests Status](https://img.shields.io/badge/tests-19%2F19%20passed-success.svg)](tests/)
[![Architecture](https://img.shields.io/badge/architecture-Clean%20%2F%20Zero--Dependency-orange.svg)](docs/ARCHITECTURE.md)

**Smart File Organizer (SFO)** is a robust, production-grade desktop and command-line utility designed to automatically declutter, categorize, and optimize your file system safely and efficiently.

Featuring both an intuitive **Modern Dark Theme Desktop GUI** and a feature-complete **Interactive CLI**, SFO operates out-of-the-box with **zero external dependencies** — running entirely on the Python Standard Library.

---

## 🌟 Highlights & Key Capabilities

- 🧹 **Multi-Strategy Organization:**
  - Automatically sorts **over 150+ file formats** into organized category directories (`Images`, `Documents`, `Spreadsheets`, `Presentations`, `Audio`, `Videos`, `Archives`, `Executables`, `Code`, `Fonts`, `3D_and_CAD`, `Torrents`, `Others`).
  - Organizes files chronologically by **Date** (`YYYY/MM` or `YYYY-MM-DD`).
  - Organizes files by **Size** buckets (`Tiny <1MB`, `Small`, `Medium`, `Large`, `Huge >1GB`).
  - Supports custom rule matching using **Regular Expressions (Regex)**.

- 🔍 **High-Performance 3-Tiered Deduplication:**
  - Eliminates disk bottlenecks and freezing caused by traditional full-file hashing:
    - **Tier 1 (Size Clustering):** Groups files by exact byte size, bypassing >90% of unique files without reading disk contents.
    - **Tier 2 (Fast Partial Chunk Hash):** Hashes only the first 64 KB of size-matched candidate files.
    - **Tier 3 (Full Cryptographic SHA-256):** Verifies surviving candidates with full SHA-256 to guarantee 100% bit-for-bit identity.
  - Automatically quarantines duplicate files into a safe `_Duplicates/` folder or performs safe deletion, preserving the best original file according to your selected policy (`oldest`, `newest`, or `shortest_path`).

- 🛡️ **Zero-Risk Safety & 100% Rollback (Transactional Engine):**
  - **Dry-Run Mode:** Simulates and displays planned actions in a detailed table without writing or moving a single byte on disk.
  - **1-Click Undo / Rollback:** Every session is atomically recorded in a transactional journal (SQLite & JSON). A single click or `--undo` command restores all files to their exact original locations and automatically removes newly created empty directories.
  - **Collision Resolution:** Automatically avoids overwriting files by renaming with incremented counters: `file (1).ext`.

- ⚡ **Real-Time Directory Watcher (Daemon Mode):**
  - Runs in the background to monitor designated folders (such as `Downloads`).
  - Features **Download Stabilization Debounce**: waits for file size to stabilize and ignores incomplete downloads (`.crdownload`, `.tmp`, `.part`) before triggering organization.

- 🖥️ **Dual Interface (Desktop GUI & Rich CLI):**
  - **Modern Desktop GUI:** Built with Tkinter/ttk in a sleek slate dark palette, featuring metric cards, an interactive dry-run table, duplicate manager, and an undo history viewer.
  - **Rich CLI:** Standard POSIX/GNU flags, colored ANSI formatting, live progress indicators, and standalone HTML dashboard report generation.

---

## 📁 Repository Directory Structure

```
File_Organizer/
├── .github/
│   └── workflows/
│       └── ci.yml                 # Automated CI test pipeline (Windows, Ubuntu, macOS)
├── config/
│   └── default_config.json        # 150+ extension mappings and default configuration
├── docs/
│   ├── PRD.md                     # Product Requirements Document
│   ├── ARCHITECTURE.md            # Technical Architecture & System Design
│   ├── CLI_USAGE.md               # Detailed Command-Line Reference Guide
│   └── GUI_GUIDE.md               # Desktop Graphical User Interface Guide
├── smart_file_organizer/          # Main application package (Clean Architecture)
│   ├── core/                      # Domain core engine (Zero external UI dependencies)
│   │   ├── models.py              # FileInfo, ActionRecord, SessionLog, Enums
│   │   ├── config.py              # ConfigManager & user override handling
│   │   ├── scanner.py             # Recursive directory scanner with pattern exclusion
│   │   ├── hasher.py              # 3-tiered chunked hasher (Size -> 64KB -> SHA-256)
│   │   ├── deduplicator.py        # Duplicate grouping & keep policy resolution
│   │   ├── organizer.py           # Organization algorithms (Ext, Date, Size, Regex)
│   │   ├── executor.py            # Transaction execution & collision handling
│   │   ├── undo_manager.py        # SQLite/JSON journaling & 100% rollback engine
│   │   ├── watcher.py             # Background watcher daemon with debounce
│   │   └── reporter.py            # Console, Markdown, and HTML Dashboard reports
│   ├── cli/                       # Command-line interface package
│   │   ├── parser.py              # Argument parsing and validation
│   │   ├── formatter.py           # ANSI color styles and ASCII table formatters
│   │   └── app.py                 # CLI controller and command dispatcher
│   └── gui/                       # Desktop graphical interface package
│       ├── theme.py               # Modern Slate Dark Theme for Tkinter/ttk
│       ├── components.py          # Reusable StatCard, FolderPicker, Treeview
│       └── app.py                 # Multi-tab Desktop GUI main window
├── tests/                         # Comprehensive automated test suite (19/19 Tests Pass)
├── main.py                        # Unified entrypoint (auto-launches GUI when run directly)
├── pyproject.toml                 # Packaging configuration (PEP 621)
├── setup.py                       # Support for pip install -e .
├── LICENSE                        # MIT License
└── README.md                      # Project documentation
```

---

## 🚀 Installation & Quick Start

### 1. Prerequisites
- **Operating System:** Windows 10/11, macOS, or Linux.
- **Python:** Version 3.10 or later.
- **External Dependencies:** **None** (operates entirely with Python's standard library).

### 2. Clone the Repository
```bash
git clone https://github.com/trtpham/File_Organizer.git
cd File_Organizer
```

---

## 🖥️ Desktop GUI Usage

Launch the desktop interface simply by running:
```bash
python main.py
```
*(or explicitly: `python main.py --gui`)*

The GUI provides four dedicated tabs:
1. **🧹 Organize & Clean:** Select source and destination folders, pick an organization strategy, click **Preview Actions** to inspect the diff table, and click **Execute Now**.
2. **🔍 Duplicate Center:** Scan for duplicate files, view wasted storage space, and quarantine redundant copies into `_Duplicates/`.
3. **⏪ Undo & History:** View past organization sessions and click **Undo Selected Session** to instantly restore files to their previous paths.
4. **⚙️ Rules & Settings:** Inspect and customize supported file extensions and categories.

---

## ⌨️ Command-Line Interface (CLI) Usage

### Common Commands:

- **Safe Dry-Run Preview (Recommended on first run):**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --dry-run
  ```

- **Live File Organization by Extension Category:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads"
  ```

- **Organize Photos or Documents by Date (`YYYY/MM`):**
  ```bash
  python main.py --source "D:/Photos" --by-date
  ```

- **Group Files by Size Ranges:**
  ```bash
  python main.py --source "D:/Downloads" --by-size
  ```

- **Detect and Quarantine Duplicate Files:**
  ```bash
  python main.py --source "D:/Storage" --dedupe
  ```

- **Rollback the Most Recent Organization Session:**
  ```bash
  python main.py --undo
  ```

- **View Past Session History:**
  ```bash
  python main.py --history
  ```

- **Start Background Watcher Daemon (Auto-organize Downloads):**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --watch
  ```

- **Generate Standalone HTML Dashboard Report:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --report "report.html"
  ```

For a complete reference of flags and options, see [CLI Reference Guide](docs/CLI_USAGE.md) or run `python main.py --help`.

---

## 🧪 Automated Testing

Smart File Organizer includes a comprehensive test suite covering scanner filtering, tiered hashing, multi-strategy organization, collision resolution, dry-run simulation, and transaction rollbacks:

```bash
python -m pytest -v tests/
```

### Test Results:
```text
tests/test_cli.py::test_cli_help PASSED                                  [  5%]
tests/test_cli.py::test_cli_dry_run PASSED                               [ 10%]
tests/test_cli.py::test_cli_history_empty PASSED                         [ 15%]
tests/test_config.py::test_default_categories_loaded PASSED              [ 21%]
tests/test_config.py::test_extension_category_lookup PASSED              [ 26%]
tests/test_config.py::test_ignore_pattern_matching PASSED                [ 31%]
tests/test_config.py::test_size_bucket_determination PASSED              [ 36%]
tests/test_deduplicator.py::test_deduplicator_finds_exact_duplicates PASSED [ 42%]
tests/test_deduplicator.py::test_plan_deduplication_actions PASSED       [ 47%]
tests/test_executor.py::test_dry_run_leaves_files_untouched PASSED       [ 52%]
tests/test_executor.py::test_live_execution_moves_files PASSED           [ 57%]
tests/test_executor.py::test_collision_auto_rename PASSED                [ 63%]
tests/test_hasher.py::test_partial_and_full_hash PASSED                  [ 68%]
tests/test_hasher.py::test_identical_files_produce_identical_hashes PASSED [ 73%]
tests/test_organizer.py::test_organize_by_extension PASSED               [ 78%]
tests/test_organizer.py::test_organize_by_size PASSED                    [ 84%]
tests/test_scanner.py::test_scanner_filters_ignored_files PASSED         [ 89%]
tests/test_scanner.py::test_scanner_extracts_correct_metadata PASSED     [ 94%]
tests/test_undo.py::test_undo_restores_all_files PASSED                  [100%]

============================= 19 passed in 0.36s ==============================
```

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!
1. Fork the project.
2. Create your feature branch (`git checkout -b feature/AmazingFeature`).
3. Commit your changes (`git commit -m 'feat: Add AmazingFeature'`).
4. Push to the branch (`git push origin feature/AmazingFeature`).
5. Open a Pull Request.

---

## 📄 License

This project is licensed under the **MIT License**. See the [`LICENSE`](LICENSE) file for details.

---

## 👤 Author

- **Author:** Pham Minh Triet (Raven)
- **Email:** `trietpham1912@gmail.com`
- **GitHub:** [@trtpham](https://github.com/trtpham)
- **Repository:** [https://github.com/trtpham/File_Organizer](https://github.com/trtpham/File_Organizer)
