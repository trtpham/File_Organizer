# Smart File Organizer Desktop GUI User Manual

## Launching the Desktop Application
You can launch the GUI in two ways:
1. Double-click `main.py` (or run without arguments):
   ```bash
   python main.py
   ```
2. Or explicitly pass the `--gui` flag:
   ```bash
   python main.py --gui
   ```

---

## Tabs & Navigation

### 1. 🧹 Organize & Clean
- **Source & Destination Folder Pickers:** Click "Browse..." to select target folders.
- **Strategy Selector:** Choose between `extension`, `date`, `size`, or `regex`.
- **Smart Financial Sort Checkbox:** Automatically analyzes PDF/image text and routes invoices and bills into `Finance_<Year>/<Vendor>`.
- **Dry-Run Checkbox:** Enabled by default for 100% safe preview before execution.
- **Preview Actions & Execute Now:** Real-time diff table with one-click live execution.

### 2. 🔍 Duplicate Center (Exact Hash & Visual pHash)
- **Dual Deduplication Modes:**
  - `Exact Hash (SHA-256)`: Finds 100% bit-for-bit identical files across any format.
  - `Visual pHash (dHash)`: Finds visually similar, resized, or re-compressed photos.
- **Similarity Threshold:** Choose similarity percentage (80%, 85%, 90%, 95%, 100%).
- **Keep Original Selector:** Choose between `oldest`, `newest`, `shortest_path`, or best resolution.
- **Move Duplicates:** Safely isolates redundant files into `_Duplicates/` folder.

### 3. 🏷️ Smart Bulk Renamer
- **Metadata Tokens:** `{name}`, `{counter:03d}`, `{date}`, `{time}`, `{camera}`, `{artist}`, `{title}`, `{album}`, `{size}`.
- **Find & Replace:** Replace text across hundreds of files in seconds.
- **Casing Transformations:** Convert to `lowercase`, `uppercase`, `title_case`, `snake_case`, or `kebab_case`.
- **Interactive Diff Table:** View Original Filename $\rightarrow$ New Filename before executing.
- **Undoable:** Batch renames are saved as transactions and can be rolled back anytime.

### 4. 📊 Storage Treemap & Disk Analysis
- **Interactive Treemap Canvas:** Visualizes nested disk space usage with colored geometric blocks proportional to folder and file sizes.
- **Top Storage Eaters:** Ranked list of the largest files consuming hard drive capacity.
- **Fast Traversal:** Scans gigabytes of disk space in seconds with real-time size metrics.

### 5. 💾 USB & Backup Sync
- **Removable Drive Auto-Detection:** Automatically recognizes USB flash drives and external hard drives upon connection.
- **Sync Modes:**
  - `Incremental`: Copies new and modified files only, preserving timestamps.
  - `Mirror`: Full folder replication.
  - `Snapshot`: Packages watched directories into timestamped `.zip` archives (ideal for game saves and code snapshots).
- **Activity Log:** Real-time transfer progress and byte counters.

### 6. 🛡️ Security & Dev Junk Cleaner
- **Dev Junk Cleaner:**
  - Scans for abandoned `node_modules`, Python `.venv`, `__pycache__`, Rust `target/`, and build artifacts inactive for over 14 days.
  - One-click safe cleanup reclaiming gigabytes of disk space.
- **Secret Scanner (Data Loss Prevention - DLP):**
  - Scans directories for exposed AWS keys, OpenAI API tokens, GitHub Personal Access Tokens, RSA/SSH private keys, and database connection strings.
  - **Quarantine Mode:** Safely relocates leaked files into a secure `_Quarantine/` folder with permissions restricted.

### 7. ⏪ Undo & History
- Displays all past organization and renaming sessions with date, mode, files moved, and status.
- Select any completed session and click **⏪ Undo Selected Session** to instantly restore files to their previous paths.

### 8. ⚙️ Rules, Settings & Shell Integration
- **Windows Explorer Context Menu:**
  - One-click registration of right-click menu items under current user (`HKCU`): "Smart Organize Here", "Find Duplicates Here", and "Open in SFO GUI". No Administrator privileges needed.
  - One-click uninstaller.
- **File Categories:** Inspect all 150+ file extensions mapped to categories.
- User overrides are automatically stored in `~/.smart_file_organizer/config.json`.
