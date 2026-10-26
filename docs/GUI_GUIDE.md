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
- **Strategy Selector:** Choose between:
  - `extension`: Group into categorized folders (`Images`, `Documents`, `Code`, etc.).
  - `date`: Group into `YYYY/MM` subdirectories.
  - `size`: Group by file sizes (`Tiny`, `Medium`, `Large`, `Huge`).
  - `regex`: Group based on custom pattern rules.
- **Dry-Run Checkbox:** Enabled by default. Ensures you can safely inspect planned moves before any disk modification.
- **Preview Actions Button:** Runs a fast scan and populates the preview table.
- **Execute Now Button:** Performs the organization (shows confirmation dialog in Live mode).

### 2. 🔍 Duplicate Center
- **Scan Duplicates:** Uses 3-tiered hashing to find true byte-identical duplicates without getting slowed down by large non-duplicate files.
- **Keep Original Selector:** Choose which file to keep as master (`oldest`, `newest`, or `shortest_path`).
- **Move to `_Duplicates`:** Isolates duplicate files into a safe `_Duplicates/` folder while leaving the original file intact.

### 3. ⏪ Undo & History
- Displays a table of all past sessions with timestamp, strategy, files moved, and status.
- Select any completed session and click **⏪ Undo Selected Session** to instantly restore all files to their original paths.

### 4. ⚙️ Rules & Settings
- Inspect all 150+ file extensions mapped to categories.
- User overrides are automatically stored in `~/.smart_file_organizer/config.json`.
