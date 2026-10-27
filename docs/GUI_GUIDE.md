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

### 4. 💾 USB & Backup Sync
- **Removable Drive Auto-Detection:** Automatically recognizes USB flash drives and external hard drives upon connection.
- **Sync Modes:**
  - `Incremental`: Copies new and modified files only, preserving timestamps.
  - `Mirror`: Full folder replication.
  - `Snapshot`: Packages watched directories into timestamped `.zip` archives (ideal for game saves and code snapshots).
- **Activity Log:** Real-time transfer progress and byte counters.

### 5. ⏪ Undo & History
- Displays all past organization and renaming sessions with date, mode, files moved, and status.
- Select any completed session and click **⏪ Undo Selected Session** to instantly restore files to their previous paths.

### 6. ⚙️ Rules & Settings
- Inspect all 150+ file extensions mapped to categories.
- User overrides are automatically stored in `~/.smart_file_organizer/config.json`.
