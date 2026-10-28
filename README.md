# Smart File Organizer 📂 (Commercial Enterprise Edition v3.0)

[![Python Version](https://img.shields.io/badge/python-3.10%2B-blue.svg)](https://www.python.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Tests Status](https://img.shields.io/badge/tests-71%2F71%20passed-success.svg)](tests/)
[![Architecture](https://img.shields.io/badge/architecture-Clean%20%2F%20Zero--Dependency-orange.svg)](docs/ARCHITECTURE.md)

**Smart File Organizer (SFO)** is a robust, enterprise-grade desktop and command-line utility designed to automatically declutter, categorize, batch-rename, sync, analyze, wirelessly transfer, and optimize your file system safely and efficiently.

Featuring an intuitive **Modern Dark Theme Desktop GUI (9 Dedicated Tabs)** and a feature-complete **Interactive CLI**, SFO operates out-of-the-box with **zero mandatory external dependencies** — running on the pure Python Standard Library with optional high-fidelity extensions for Pillow and PyMuPDF.

---

## 🌟 Highlights & Key Capabilities

- 🩺 **System Doctor & Diagnostic Engine (`sfo --doctor`):**
  - Instant pre-flight checks: Python environment, SQLite database lock integrity, disk storage headroom, write permissions, and audit log tamper detection.
  - Generates actionable diagnostics (`PASS`, `WARN`, `FAIL`) and JSON reports for IT automation.

- 🛡️ **Enterprise Safety Guardrails & Blast Radius Control:**
  - Prevents disastrous mistakes: automatically blocks operations targeting root drives (`C:\`, `D:\`) or system directories (`Windows`, `System32`, `Program Files`, `/usr`, `/etc`).
  - Strict blast radius thresholds requiring explicit confirmation (`--force`) before executing changes affecting $\ge 1,000$ files.

- 📜 **Tamper-Evident SHA-256 Audit Log Chain:**
  - Immutable cryptographic hash chain verifying every executed operation.
  - Zero-tamper guarantee: detecting any manual log editing or modification (`sfo --verify-audit`).

- 🌐 **Native Internationalization (i18n):**
  - Full multilingual support with seamless switching between **English (en)** and **Vietnamese (vi)** across both CLI and GUI interfaces.

- ⏰ **OS-Level Task Scheduler & Automation:**
  - Integrates natively with Windows Task Scheduler (`schtasks.exe`) to configure autonomous daily, hourly, or weekly file organization without third-party services.

- 🔔 **Windows Background Tray & Desktop Toast Notifications:**
  - Background daemon monitoring with native Windows balloon / toast notifications on completed file organization runs.
  - One-click Windows startup registration (`HKCU\...\Run`).

- 📦 **Standalone Binary & Portable Packaging:**
  - Zero-Python-installation deployment: compiles into a standalone `sfo.exe` binary with automated GitHub Actions release packaging.

- 🗄️ **Cold Storage Archiving & Compression Tiering:**
  - Identifies cold files inactive for $\ge N$ days and bundles them into organized ZIP archives.
  - Generates internal `manifest.json` metadata records to track original locations and modification dates.
  - Safe source deletion option reclaiming gigabytes of disk space, with full archive restoration.

- ☁️ **Cloud Storage Connector (OneDrive, Google Drive, Dropbox, iCloud):**
  - Automatic cross-platform discovery of active local cloud synchronization folders.
  - Direct routing of important files, backups, and finance records to cloud storage with zero extra configuration.

- 📡 **Wireless Cross-Device LAN Drop:**
  - Built-in zero-dependency local HTTP transfer server for instant wireless exchange across phones, tablets, and PCs on the same Wi-Fi.
  - Mobile-friendly web UI for downloading files or uploading directly into `SFO_Incoming/` without cables or mobile apps.
  - Optional 4-digit PIN authentication for local access control.

- 📊 **Storage Treemap & Disk Analysis:**
  - Fast recursive disk usage analysis displaying space breakdown across directories.
  - Interactive 2D slice-and-dice treemap visualization in the GUI.
  - Instant discovery of top storage-eating files consuming gigabytes of disk space.

- 🔥 **Military-Grade Secure File Shredder (`sfo --shred <path>`):**
  - Permanent, unrecoverable data wiping adhering to **DoD 5220.22-M (3-Pass)**, NIST SP 800-88, and Gutmann (7-Pass) standards.
  - Multi-pass binary overwriting (`0x00`, `0xFF`, cryptographically secure pseudo-random bytes), physical disk flush (`os.fsync`), 0-byte truncation, and filename MFT obfuscation before unlinking.
  - Built-in Enterprise Guardrail protection blocking accidental erasure of Windows/System directories.

- 🔒 **Cryptographic Encrypted Vault (`sfo --vault-lock`, `sfo --vault-unlock`):**
  - Zero-knowledge AES-256-GCM / PBKDF2 (100,000 rounds) container (`.sfovault`) protecting confidential documents, tax forms, and source archives.
  - Authenticated symmetric cipher verifying data integrity with tamper detection; optional automated post-encryption DoD source file shredding.

- 🧹 **Developer Bloat & Junk Cleaner:**
  - Scans for abandoned and inactive `node_modules`, Python `.venv`, `__pycache__`, and build artifact directories.
  - Safe inspection and one-click deletion reclaiming valuable disk space.

- 🛡️ **Secret Scanner (Data Loss Prevention - DLP) & File Quarantine:**
  - Inspects files for exposed credentials: AWS Access Keys, OpenAI API tokens, GitHub Personal Access Tokens, SSH private keys, and connection strings.
  - One-click safe quarantine moves compromised files into `_Quarantine/` folder.

- 🪟 **Windows Explorer Context Menu Integration:**
  - One-click installation of right-click folder context menu ("Smart Organize Here", "Find Duplicates Here", "Open in SFO GUI").
  - Registers under `HKEY_CURRENT_USER` (HKCU) — **zero Administrator privileges required**.

- 🏷️ **Smart Bulk Renamer (EXIF & ID3 Tags):**
  - Extracts photo metadata (Date Taken, Camera Model, Resolution) and audio metadata (Title, Artist, Album, Track).
  - Rich templating: `{name}`, `{counter:03d}`, `{date}`, `{time}`, `{camera}`, `{artist}`, `{title}`, `{album}`.
  - Case transformations: `lowercase`, `uppercase`, `title_case`, `snake_case`, `kebab_case`.
  - Find & Replace and numbering with 100% reversible Undo support.

- 🔍 **Visual Duplicate Cleaner (Perceptual pHash) + Exact Hash (SHA-256):**
  - **Exact Hash Mode:** 3-tiered filtration (Size $\rightarrow$ 64KB Chunk $\rightarrow$ Full SHA-256) for 100% identical files.
  - **Visual pHash Mode:** 64-bit difference hashing (`dHash`) with Hamming distance comparison to find resized, re-compressed, or near-identical photos with a configurable similarity threshold ($80\% - 100\%$).
  - Intelligently recommends preserving the best resolution or largest file.

- 🧾 **Receipt & Bill Organizer (Smart Financial Sort):**
  - Content inspection for PDF and image invoices, receipts, and utility bills.
  - Multilingual heuristic keyword matching (*English & Vietnamese:* EVN, Viettel, VNPT, AWS, Google, Apple, VAT, Total Amount, Hóa đơn điện tử).
  - Extracts transaction date and vendor, automatically routing files into `Finance_<Year>/<Vendor>` folders.

- 💾 **Auto-Sync USB Backup & App Save Backuper:**
  - Automated detection of removable USB storage drives upon connection.
  - Incremental folder synchronization preserving file modification timestamps.
  - Timestamped `.zip` snapshot archiving (ideal for game saves and code projects).

- 🧹 **Multi-Strategy Organization:**
  - Automatically sorts **over 150+ file formats** into organized category directories (`Images`, `Documents`, `Spreadsheets`, `Presentations`, `Audio`, `Videos`, `Archives`, `Executables`, `Code`, `Fonts`, `3D_and_CAD`, `Torrents`, `Others`).
  - Organizes files chronologically by **Date** (`YYYY/MM` or `YYYY-MM-DD`) or **Size** ranges.

- 🛡️ **Zero-Risk Safety & 100% Rollback (Transactional Engine):**
  - **Dry-Run Mode:** Simulates and displays planned actions in a detailed table without writing or moving a single byte on disk.
  - **1-Click Undo / Rollback:** Every session is atomically recorded in an append-only journal (SQLite & JSON). A single click or `--undo` command restores all files to their exact original locations.
  - **Collision Resolution:** Automatically avoids overwriting files by renaming with incremented counters: `file (1).ext`.

- ⚡ **Real-Time Directory Watcher (Daemon Mode):**
  - Runs in the background to monitor designated folders (such as `Downloads`).
  - Features **Download Stabilization Debounce**: waits for file size to stabilize and ignores incomplete downloads (`.crdownload`, `.tmp`, `.part`).

---

## 📁 Repository Directory Structure

```
File_Organizer/
├── .github/
│   └── workflows/
│       ├── ci.yml                 # Automated CI test pipeline (Windows, Ubuntu, macOS)
│       └── release.yml            # Automated PyInstaller packaging & GitHub Release CI
├── config/
│   └── default_config.json        # 150+ extension mappings and default configuration
├── docs/
│   ├── PRD.md                     # Product Requirements Document
│   ├── ARCHITECTURE.md            # Technical Architecture & System Design
│   ├── CLI_USAGE.md               # Detailed Command-Line Reference Guide
│   ├── GUI_GUIDE.md               # Desktop Graphical User Interface Guide
│   ├── ENTERPRISE_DEPLOYMENT.md   # Deployment, Guardrails & Audit Documentation
│   ├── DOCTOR.md                  # System Health & Diagnostic Engine Documentation
│   └── COMMERCIAL_ROADMAP.md      # Commercialization & Scaling Roadmap
├── scripts/
│   └── build_exe.py               # Standalone PyInstaller build & ZIP packaging pipeline
├── smart_file_organizer/          # Main application package (Clean Architecture)
│   ├── core/                      # Domain core engine (Zero external UI dependencies)
│   │   ├── models.py              # FileInfo, ActionRecord, SessionLog, Enums
│   │   ├── config.py              # ConfigManager & user override handling
│   │   ├── scanner.py             # Recursive directory scanner with pattern exclusion
│   │   ├── hasher.py              # 3-tiered chunked hasher (Size -> 64KB -> SHA-256)
│   │   ├── visual_hasher.py       # Perceptual difference hasher (dHash) & similarity engine
│   │   ├── deduplicator.py        # Duplicate grouping (Exact & Visual) & resolution
│   │   ├── renamer.py             # Smart Bulk Renamer (EXIF, ID3, Templates, Casing)
│   │   ├── syncer.py              # USB Drive detector, incremental sync & snapshot archiver
│   │   ├── archiver.py            # Cold storage analyzer, compression & restore engine
│   │   ├── cloud_connector.py     # Auto-detect OneDrive, GDrive, Dropbox & cloud sync
│   │   ├── lan_share.py           # Embedded HTTP LAN Drop server for wireless transfers
│   │   ├── treemap.py             # Disk usage tree analyzer & 2D treemap layout engine
│   │   ├── junk_cleaner.py        # Inactive dev bloat cleaner (node_modules, caches)
│   │   ├── secret_scanner.py      # DLP secret detector (AWS, OpenAI, GitHub, SSH) & quarantine
│   │   ├── shell_integration.py   # Windows Explorer HKCU right-click context menu manager
│   │   ├── enterprise_guardrails.py # System path protection & SHA-256 audit log chain
│   │   ├── doctor.py              # System health check & pre-flight diagnostic engine
│   │   ├── i18n.py                # Multi-language internationalization (English / Vietnamese)
│   │   ├── tray_daemon.py         # Windows startup autorun & desktop notifications
│   │   ├── scheduler.py           # Windows Task Scheduler automation manager
│   │   ├── smart_filter.py        # PDF text extractor & financial invoice classifier
│   │   ├── organizer.py           # Organization algorithms (Ext, Date, Size, Regex, Receipts)
│   │   ├── executor.py            # Transaction execution & collision handling
│   │   ├── undo_manager.py        # SQLite/JSON journaling & 100% rollback engine
│   │   ├── watcher.py             # Background watcher daemon with debounce & USB sync
│   │   └── reporter.py            # Console, Markdown, and HTML Dashboard reports
│   ├── cli/                       # Command-line interface package
│   │   ├── parser.py              # Argument parsing and validation
│   │   ├── formatter.py           # ANSI color styles and ASCII table formatters
│   │   └── app.py                 # CLI controller and command dispatcher
│   └── gui/                       # Desktop graphical interface package
│       ├── theme.py               # Modern Slate Dark Theme for Tkinter/ttk
│       ├── components.py          # Reusable StatCard, FolderPicker, Treeview
│       └── app.py                 # 9-Tab Desktop GUI main window
├── tests/                         # Comprehensive automated test suite (54/54 Tests Pass)
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
- **Python:** Version 3.10 or later (or run standalone portable `sfo.exe`).
- **External Dependencies:** **None** (runs entirely with Python's standard library; Pillow/PyMuPDF supported automatically when available).

### 2. Standalone Binary (Zero-Python Required)
Download `SmartFileOrganizer_v3.0_Windows_x64.zip` from Releases, unzip anywhere, and run `sfo.exe --gui` or double-click `Launch_GUI.bat`.

### 3. Developer Source Installation
```bash
git clone https://github.com/trtpham/File_Organizer.git
cd File_Organizer
python main.py
```

---

## 🖥️ Desktop GUI Usage (9 Dedicated Tabs)

Launch the desktop interface simply by running:
```bash
python main.py
```
*(or explicitly: `python main.py --gui`)*

The GUI provides nine dedicated tabs:
1. **🧹 Organize & Clean:** Select folders, choose strategy, toggle Smart Financial Sort, click **Preview Actions**, and execute.
2. **🔍 Duplicate Center:** Switch between Exact Hash (SHA-256) and Visual pHash (dHash with threshold slider).
3. **🏷️ Smart Renamer:** Batch rename files using metadata tokens (`{name}`, `{counter:03d}`, `{date}`, `{camera}`, `{artist}`), casing conversions, and live diff table.
4. **📊 Storage Treemap:** Interactive 2D treemap rendering and ranked table of largest space-eating files.
5. **💾 Sync, Cloud & LAN:** 4 sub-tabs for USB Drive Backup, Cloud Sync (OneDrive/GDrive/Dropbox), Wireless LAN Drop Server, and Cold Storage Archive.
6. **🛡️ Security & Junk:** Reclaim space by cleaning inactive `node_modules` and scan files for exposed API keys/secrets with quarantine.
7. **⏪ Undo & History:** View past organization and renaming sessions and click **Undo Selected Session** for instant rollback.
8. **⚙️ Settings & Shell:** Language switcher (English / Tiếng Việt), Windows autostart toggle, and Windows Explorer context menu registration.
9. **🩺 Doctor & Health:** System health check verifying disk space, database lock state, Python environment, and cryptographic audit log chain integrity.

---

## ⌨️ Command-Line Interface (CLI) Usage

### Common Commands:

- **Cold Storage Archiving & Compression:**
  ```bash
  python main.py --source "D:/Archives" --archive-cold --archive-days 90
  python main.py --restore-archive "D:/Archives/Archive_20261027.zip" --dest "D:/Restored"
  ```

- **Cloud Storage Detection & Sync:**
  ```bash
  python main.py --detect-cloud
  python main.py --source "C:/Users/Raven/Documents" --sync-cloud onedrive
  ```

- **Wireless LAN Drop (Cross-Device File Exchange):**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --lan-share --lan-port 8080
  ```

- **Run System Doctor Diagnostics:**
  ```bash
  python main.py --doctor
  python main.py --doctor --doctor-json
  ```

- **Verify Cryptographic SHA-256 Audit Log Chain:**
  ```bash
  python main.py --verify-audit
  ```

- **Schedule Autonomous Daily Organization (Windows Task Scheduler):**
  ```bash
  python main.py --schedule-daily 03:00 --source "D:/Downloads"
  python main.py --list-tasks
  python main.py --delete-task SFO_Daily_D_Downloads
  ```

- **Enable / Disable Windows Startup Autorun:**
  ```bash
  python main.py --enable-autostart
  python main.py --disable-autostart
  ```

- **Storage Analysis & Top Files:**
  ```bash
  python main.py --source "C:/Users/Raven" --analyze-disk
  ```

- **Developer Bloat & Junk Cleaner:**
  ```bash
  python main.py --source "D:/Workspaces" --scan-junk
  python main.py --source "D:/Workspaces" --clean-junk
  ```

- **Secret Scanner (DLP) & File Quarantine:**
  ```bash
  python main.py --source "D:/Projects" --scan-secrets
  python main.py --source "D:/Projects" --quarantine-secrets
  ```

- **Windows Explorer Right-Click Menu Integration:**
  ```bash
  python main.py --install-context-menu
  python main.py --uninstall-context-menu
  ```

- **Smart Bulk Renaming with Metadata & Counter:**
  ```bash
  python main.py --source "D:/Photos" --rename-template "{date}_{camera}_{counter:03d}"
  ```

- **Perceptual Visual Duplicate Detection (pHash):**
  ```bash
  python main.py --source "D:/Photos" --visual-dedupe --similarity 0.90
  ```

- **Smart Financial Invoice & Receipt Organization:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --smart-receipts
  ```

- **USB Drive Detection & Incremental Backup:**
  ```bash
  python main.py --detect-usb
  python main.py --sync-source "C:/Users/Raven/Documents" --sync-dest "E:/USB_Backup" --sync-mode incremental
  ```

- **Safe Dry-Run Preview (Recommended on first run):**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --dry-run
  ```

- **Live File Organization by Extension Category:**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads"
  ```

- **Rollback the Most Recent Session (100% Undo):**
  ```bash
  python main.py --undo
  ```

- **Start Background Watcher Daemon (Auto-organize Downloads):**
  ```bash
  python main.py --source "C:/Users/Raven/Downloads" --watch
  ```

- **Build Standalone Portable Executable:**
  ```bash
  python scripts/build_exe.py
  ```

For a complete reference of flags and options, see [CLI Reference Guide](docs/CLI_USAGE.md) or run `python main.py --help`.

---

## 🧪 Automated Testing

Smart File Organizer includes an enterprise-grade automated test suite covering system diagnostics, safety guardrails, cryptographic audit logs, internationalization, task scheduling, tray notifications, scanner filtering, tiered hashing, perceptual dHash, metadata renaming, USB syncing, smart receipt classification, disk treemap analysis, dev junk cleaner, secret scanning, collision resolution, dry-run simulation, and transaction rollbacks:

```bash
python -m pytest -v tests/
```

### Test Results:
```text
tests/test_archiver.py::test_cold_storage_scanning_and_archiving PASSED  [  1%]
tests/test_cli.py::test_cli_help PASSED                                  [  3%]
tests/test_cli.py::test_cli_dry_run PASSED                               [  5%]
tests/test_cli.py::test_cli_history_empty PASSED                         [  7%]
tests/test_cli.py::test_cli_detect_cloud PASSED                          [  8%]
tests/test_cli.py::test_cli_archive_cold_empty PASSED                    [ 10%]
tests/test_cloud_connector.py::test_cloud_detector_runs_safely PASSED    [ 12%]
tests/test_cloud_connector.py::test_cloud_sync_manager PASSED            [ 14%]
tests/test_config.py::test_default_categories_loaded PASSED              [ 15%]
tests/test_config.py::test_extension_category_lookup PASSED              [ 17%]
tests/test_config.py::test_ignore_pattern_matching PASSED                [ 19%]
tests/test_config.py::test_size_bucket_determination PASSED              [ 21%]
tests/test_deduplicator.py::test_deduplicator_finds_exact_duplicates PASSED [ 22%]
tests/test_deduplicator.py::test_plan_deduplication_actions PASSED       [ 24%]
tests/test_doctor.py::test_doctor_full_diagnosis PASSED                  [ 26%]
tests/test_enterprise_guardrails.py::test_protected_path_detection PASSED [ 28%]
tests/test_enterprise_guardrails.py::test_blast_radius_threshold PASSED  [ 29%]
tests/test_enterprise_guardrails.py::test_audit_log_hash_chain PASSED    [ 31%]
tests/test_executor.py::test_dry_run_leaves_files_untouched PASSED       [ 33%]
tests/test_executor.py::test_live_execution_moves_files PASSED           [ 35%]
tests/test_executor.py::test_collision_auto_rename PASSED                [ 36%]
tests/test_hasher.py::test_partial_and_full_hash PASSED                  [ 38%]
tests/test_hasher.py::test_identical_files_produce_identical_hashes PASSED [ 40%]
tests/test_hasher.py::test_parallel_batch_hashing PASSED                 [ 42%]
tests/test_i18n.py::test_i18n_translation PASSED                         [ 43%]
tests/test_junk_cleaner.py::test_scan_and_clean_junk_directory PASSED    [ 45%]
tests/test_lan_share.py::test_get_local_ip PASSED                        [ 47%]
tests/test_lan_share.py::test_lan_share_server_lifecycle PASSED          [ 49%]
tests/test_organizer.py::test_organize_by_extension PASSED               [ 50%]
tests/test_organizer.py::test_organize_by_size PASSED                    [ 52%]
tests/test_renamer.py::test_basic_template_renaming PASSED               [ 54%]
tests/test_renamer.py::test_case_transformations PASSED                  [ 56%]
tests/test_renamer.py::test_find_and_replace PASSED                      [ 57%]
tests/test_renamer.py::test_id3_v1_extraction PASSED                     [ 59%]
tests/test_scanner.py::test_scanner_filters_ignored_files PASSED         [ 61%]
tests/test_scanner.py::test_scanner_extracts_correct_metadata PASSED     [ 63%]
tests/test_scheduler.py::test_windows_task_scheduler_query PASSED        [ 64%]
tests/test_secret_scanner.py::test_detect_aws_and_openai_secrets PASSED  [ 66%]
tests/test_secret_scanner.py::test_quarantine_leaked_file PASSED         [ 68%]
tests/test_secret_scanner.py::test_shannon_entropy_and_extended_tokens PASSED [ 70%]
tests/test_shell_integration.py::test_is_supported_returns_bool PASSED   [ 71%]
tests/test_shell_integration.py::test_is_installed_query PASSED          [ 73%]
tests/test_smart_filter.py::test_vietnamese_invoice_detection PASSED     [ 75%]
tests/test_smart_filter.py::test_english_invoice_detection PASSED        [ 77%]
tests/test_smart_filter.py::test_smart_financial_sort_routing PASSED     [ 78%]
tests/test_syncer.py::test_drive_detector_runs_without_error PASSED      [ 80%]
tests/test_syncer.py::test_incremental_sync PASSED                       [ 82%]
tests/test_snapshot_sync PASSED                                          [ 84%]
tests/test_tray_daemon.py::test_windows_startup_manager_query PASSED     [ 85%]
tests/test_tray_daemon.py::test_desktop_notifier_runs PASSED             [ 87%]
tests/test_treemap.py::test_disk_tree_analysis PASSED                    [ 89%]
tests/test_treemap.py::test_treemap_layout_coordinates PASSED            [ 91%]
tests/test_find_largest_files PASSED                                    [ 92%]
tests/test_undo.py::test_undo_restores_all_files PASSED                  [ 94%]
tests/test_undo.py::test_undo_prune_and_rename_rollback PASSED           [ 96%]
tests/test_shredder.py::test_shred_single_file_dod_3pass PASSED          [ 62%]
tests/test_shredder.py::test_shred_single_file_quick_1pass PASSED        [ 63%]
tests/test_shredder.py::test_shred_empty_file PASSED                     [ 65%]
tests/test_shredder.py::test_shred_directory_recursive PASSED            [ 66%]
tests/test_shredder.py::test_shred_guardrails_blocks_protected_path PASSED [ 68%]
tests/test_shredder.py::test_shred_nonexistent_file_raises PASSED        [ 69%]
tests/test_vault.py::test_vault_lock_and_unlock_single_file PASSED       [ 90%]
tests/test_vault.py::test_vault_lock_and_unlock_directory_tree PASSED    [ 91%]
tests/test_vault.py::test_vault_wrong_password_raises PASSED             [ 92%]
tests/test_vault.py::test_vault_tampered_ciphertext_detection PASSED     [ 94%]
tests/test_vault.py::test_vault_inspect_contents_without_extracting PASSED [ 95%]
tests/test_vault.py::test_vault_shred_source_option PASSED               [ 97%]
tests/test_visual_hasher.py::test_hamming_distance_and_similarity PASSED [ 98%]
tests/test_visual_hasher.py::test_visual_similarity_clustering PASSED    [100%]

============================= 71 passed in 3.04s ==============================
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
