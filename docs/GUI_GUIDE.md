# Smart File Organizer Desktop GUI User Manual

## Launching the Desktop Application
Run `sfo-gui` (`sfo-gui.exe` on Windows). Running `sfo` with no arguments, or
`sfo --gui`, also opens it, provided `sfo-gui` sits in the same folder. The
Explorer right-click entry "Open in SFO GUI" passes the clicked folder with
`--source`, which pre-fills every folder field.

The app is built with egui (v3.0 used Tkinter). Long operations — scans,
hashing, moves, encryption — run in the background: the status bar shows a
spinner and the action buttons are disabled until the job finishes. Every
confirmation and result appears in a dialog inside the window.

Scans, hashing and file moves show a progress bar in the status bar with a **✖ Cancel** button. Cancelling a scan changes nothing; cancelling a move stops before the next file, and what was already moved can be undone.

Live actions that would touch more than the blast-radius limit (default 500 files or 10 GB, `blast_radius` in `config.json`) ask for confirmation with a red warning first.

While a job runs the window cannot be closed, so a move is never cut off before
it is recorded for undo. Folders that move or delete files are checked against
the same guardrails as the CLI: drive roots and system folders are refused.

---

## Tabs & Navigation

### 1. 🧹 Organize & Clean
- **Source & Destination Folder Pickers:** Click "Browse..." to select target folders.
- **Strategy Selector:** Choose between `extension`, `date`, `size`, or `regex`.
- **Smart Financial Sort Checkbox:** Automatically analyzes PDF/image text and routes invoices and bills into `Finance_<Year>/<Vendor>`.
- **Dry-Run Checkbox:** Enabled by default for 100% safe preview before execution.
- **Include subfolders:** Also organizes files inside subfolders of the source (the destination folder is skipped).
- **Preview Actions & Execute Now:** Real-time diff table with one-click live execution.
- **Folder Watcher:** *Start Watching* organizes every new file in the Source Folder as soon as its download finishes, using the folders and strategy above; each file becomes its own undoable session and appears in the watcher log. Manual Preview/Execute and the folder fields are locked while it runs; *Stop Watching* ends it.

### 2. 🔍 Duplicate Center (Exact Hash & Visual pHash)
- **Dual Deduplication Modes:**
  - `Exact Hash (SHA-256)`: Finds 100% bit-for-bit identical files across any format.
  - `Visual pHash (dHash)`: Finds visually similar, resized, or re-compressed photos.
- **Similarity Threshold:** Choose similarity percentage (80%, 85%, 90%, 95%, 100%).
- **Keep Original Selector:** Choose between `oldest`, `newest` and `shortest_path` (visual mode always keeps the highest resolution).
- **Move Duplicates:** Safely isolates redundant files into `_Duplicates/` folder.

### 3. 🏷️ Smart Bulk Renamer
- **Metadata Tokens:** `{name}`, `{counter:03d}`, `{date}`, `{time}`, `{camera}`, `{artist}`, `{title}`, `{album}`, `{size}`.
- **Find & Replace:** Replace text across hundreds of files in seconds.
- **Casing Transformations:** Convert to `lowercase`, `uppercase`, `title_case`, `snake_case`, `kebab_case` or `slugify` (also strips accents).
- **Interactive Diff Table:** View Original Filename $\rightarrow$ New Filename before executing.
- **Undoable:** Batch renames are saved as transactions and can be rolled back anytime.

### 4. 📊 Storage Treemap & Disk Analysis
- **Interactive Treemap Canvas:** Visualizes nested disk space usage with colored geometric blocks proportional to folder and file sizes.
- **Top Storage Eaters:** Ranked list of the largest files consuming hard drive capacity.
- **Fast Traversal:** Scans gigabytes of disk space in seconds with real-time size metrics.

### 5. 💾 Sync, Cloud & LAN Drop (4 Sub-Tabs)
- **🔌 USB Drive Backup:**
  - Automatically recognizes USB flash drives and external hard drives upon connection.
  - Supports `Incremental`, `Mirror`, and timestamped `.zip` `Snapshot` modes.
- **☁️ Cloud Storage Sync:**
  - One-click auto-detection of local sync directories for Microsoft OneDrive, Google Drive, Dropbox, and Apple iCloud Drive.
  - Syncs critical documents, game saves, or financial folders straight to the cloud for offsite redundancy.
- **📡 Wireless Cross-Device LAN Drop:**
  - Built-in zero-config local HTTP server.
  - Allows smartphones, tablets, and laptops on the same Wi-Fi to browse and download shared files or upload photos/documents directly into `SFO_Incoming/` without installing any apps or cords.
  - Optional 4-digit PIN security protection.
- **🗄️ Cold Storage Archive & Compression Tiering:**
  - Identifies cold and inactive files (unmodified for $\ge 90$ days).
  - One-click compression into structured ZIP archives with internal metadata manifests.
  - Optional automatic deletion of original files to instantly recover gigabytes of hard drive space.

### 6. 🛡️ Security & Dev Junk Cleaner
- **Dev Junk Cleaner:**
  - Scans for abandoned `node_modules`, Python `.venv`, `__pycache__`, Rust `target/`, and build artifacts inactive for over 14 days.
  - One-click safe cleanup reclaiming gigabytes of disk space.
- **Secret Scanner (Data Loss Prevention - DLP):**
  - Scans directories for exposed AWS keys, OpenAI API tokens, GitHub Personal Access Tokens, RSA/SSH private keys, and database connection strings.
  - **Quarantine Mode:** Moves affected files into a `_Quarantine/` folder as `<name>.quarantined`.
- **Secure File Shredder:** Choose a file or folder and a wiping standard (1, 2, 3 or 7 passes). Protected system folders are refused. Shredding cannot be undone.
- **Encrypted Vault:** Lock a file or folder into a `.sfovault` (Argon2id + XChaCha20-Poly1305), optionally shredding the original; inspect or unlock a vault with its passphrase. Vaults from v3.0 open too.

### 7. 🩺 Doctor & Health Diagnostics
- **One-Click Diagnostic Battery:**
  - Evaluates the SFO build, platform, available memory, disk headroom, SQLite database and audit-log integrity.
  - Measures real-time disk write latency (512 KB flush benchmark).
  - Inspects Windows Explorer right-click menu integration and connected cloud storage providers.
- **Health Status & Table:** Clear status (`HEALTHY` / `UNHEALTHY`) with `PASS`, `WARN`, `FAIL` badges and actionable guidance.

### 8. ⏪ Undo & History
- Displays all past organization and renaming sessions with date, mode, files moved, and status.
- Select any completed session and click **⏪ Undo Selected Session** to instantly restore files to their previous paths.

### 9. ⚙️ Enterprise Settings & Shell
- **Bilingual Language Selector:** Switch between **English** and **Tiếng Việt**; tab names and main buttons change immediately.
- **Windows Autostart on Boot:** Enable or disable automatic background launch on Windows login with one click.
- **Cryptographic Audit Verification:** Validate the SHA-256 tamper-evident audit journal.
- **Windows Explorer Context Menu:** One-click registration/removal under `HKCU` without Administrator rights.
- **Custom Regex Rules:** Add, edit and remove the rules used by the `regex` strategy. Each rule is checked as you type (invalid patterns and folders that leave the destination, such as `..` or `C:\`, are marked ✖), a test box shows which folder a file name would go to, and **Save Rules** writes them to your `config.json`.
- **File Categories:** Inspect all 150+ file extensions mapped to categories. User overrides are read from `~/.smart_file_organizer/config.json` (or `$SFO_DATA_DIR/config.json`).
