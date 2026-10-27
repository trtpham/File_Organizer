"""
Main Tkinter Desktop Application for Smart File Organizer (Enterprise Edition v2.1).
Tabs: Organize & Clean, Duplicate Center (Hash + pHash), Smart Bulk Renamer,
      USB & Backup Sync, Undo & History, Rules & Settings.
"""

import sys
import tkinter as tk
from tkinter import ttk, messagebox
from pathlib import Path
from typing import Optional, List, Dict, Any

from .theme import Theme, apply_theme
from .components import StatCard, FolderPicker, ScrollableTreeview
from ..core.config import ConfigManager
from ..core.scanner import FileScanner
from ..core.organizer import Organizer
from ..core.deduplicator import Deduplicator
from ..core.executor import TransactionExecutor
from ..core.undo_manager import UndoManager
from ..core.reporter import Reporter
from ..core.renamer import SmartBulkRenamer
from ..core.syncer import DriveDetector, FolderSynchronizer
from ..core.shell_integration import WindowsShellIntegration
from ..core.treemap import DiskAnalyzer, DiskNode
from ..core.junk_cleaner import DevJunkCleaner, JunkItem
from ..core.secret_scanner import SecretScanner, SecretFinding
from ..core.models import (
    OrganizeStrategy, ActionRecord, DuplicateGroup,
    VisualDuplicateGroup, SyncProfile, SyncMode
)


class SmartFileOrganizerApp(tk.Tk):
    """Main desktop application window."""

    def __init__(self):
        super().__init__()
        self.title("Smart File Organizer v2.2 - Enterprise Edition")
        self.geometry("1100x740")
        self.minsize(900, 600)

        self.config_manager = ConfigManager()
        self.undo_manager = UndoManager()
        self.scanner = FileScanner(self.config_manager)
        self.organizer = Organizer(self.config_manager)
        self.deduplicator = Deduplicator(self.config_manager)
        self.executor = TransactionExecutor(self.config_manager)
        self.renamer = SmartBulkRenamer()

        self.planned_records: List[ActionRecord] = []
        self.found_duplicates: List[DuplicateGroup] = []
        self.found_visual_duplicates: List[VisualDuplicateGroup] = []
        self.rename_records: List[ActionRecord] = []

        apply_theme(self)
        self._build_ui()
        self._load_history_tab()
        self._refresh_usb_drives()

    def _build_ui(self) -> None:
        """Constructs application layout."""
        # Main Header
        header_frame = ttk.Frame(self, padding=(20, 14, 20, 8))
        header_frame.pack(fill="x")

        title_lbl = ttk.Label(header_frame, text="📂 Smart File Organizer", style="Title.TLabel")
        title_lbl.pack(side="left")

        sub_lbl = ttk.Label(
            header_frame,
            text="Clean Architecture • Visual pHash • EXIF/ID3 Renamer • Auto-Sync • 100% Rollback",
            style="Muted.TLabel"
        )
        sub_lbl.pack(side="left", padx=(16, 0), pady=(4, 0))

        # Main Tabbed Notebook
        self.notebook = ttk.Notebook(self)
        self.notebook.pack(fill="both", expand=True, padx=20, pady=(0, 10))

        # Build tabs
        self.tab_organize = ttk.Frame(self.notebook, padding=14)
        self.tab_dedupe = ttk.Frame(self.notebook, padding=14)
        self.tab_renamer = ttk.Frame(self.notebook, padding=14)
        self.tab_storage = ttk.Frame(self.notebook, padding=14)
        self.tab_sync = ttk.Frame(self.notebook, padding=14)
        self.tab_security = ttk.Frame(self.notebook, padding=14)
        self.tab_history = ttk.Frame(self.notebook, padding=14)
        self.tab_settings = ttk.Frame(self.notebook, padding=14)

        self.notebook.add(self.tab_organize, text="  🧹 Organize  ")
        self.notebook.add(self.tab_dedupe, text="  🔍 Duplicate Center  ")
        self.notebook.add(self.tab_renamer, text="  🏷️ Smart Renamer  ")
        self.notebook.add(self.tab_storage, text="  📊 Storage Treemap  ")
        self.notebook.add(self.tab_sync, text="  💾 USB & Backup Sync  ")
        self.notebook.add(self.tab_security, text="  🛡️ Security & Junk  ")
        self.notebook.add(self.tab_history, text="  ⏪ Undo & History  ")
        self.notebook.add(self.tab_settings, text="  ⚙️ Settings & Shell  ")

        self._build_organize_tab()
        self._build_dedupe_tab()
        self._build_renamer_tab()
        self._build_storage_tab()
        self._build_sync_tab()
        self._build_security_tab()
        self._build_history_tab()
        self._build_settings_tab()

        # Status Bar
        status_bar = ttk.Frame(self, padding=(20, 6, 20, 8), style="Card.TFrame")
        status_bar.pack(fill="x", side="bottom")

        self.status_var = tk.StringVar(value="Ready. Select an operation or folder to begin.")
        status_lbl = ttk.Label(status_bar, textvariable=self.status_var, style="Card.TLabel", font=Theme.FONT_SMALL)
        status_lbl.pack(side="left")

    # -------------------------------------------------------------
    # TAB 1: Organize & Clean
    # -------------------------------------------------------------
    def _build_organize_tab(self) -> None:
        panel = ttk.Frame(self.tab_organize, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 10))

        self.org_source_picker = FolderPicker(panel, label="Source Folder:     ", default_path=str(Path.home() / "Downloads"))
        self.org_source_picker.pack(fill="x", pady=(0, 6))

        self.org_dest_picker = FolderPicker(panel, label="Destination Folder:", default_path=str(Path.home() / "Downloads"))
        self.org_dest_picker.pack(fill="x", pady=(0, 6))

        opts_frame = ttk.Frame(panel, style="Card.TFrame")
        opts_frame.pack(fill="x", pady=(4, 0))

        ttk.Label(opts_frame, text="Strategy:", style="Card.TLabel").pack(side="left", padx=(0, 6))
        self.strategy_var = tk.StringVar(value="extension")
        strategy_combo = ttk.Combobox(
            opts_frame,
            textvariable=self.strategy_var,
            values=["extension", "date", "size", "regex"],
            state="readonly",
            width=12
        )
        strategy_combo.pack(side="left", padx=(0, 14))

        self.dry_run_var = tk.BooleanVar(value=True)
        dry_chk = ttk.Checkbutton(opts_frame, text="Dry-Run Mode (Safe Preview)", variable=self.dry_run_var)
        dry_chk.pack(side="left", padx=(0, 14))

        self.smart_receipts_var = tk.BooleanVar(value=True)
        receipt_chk = ttk.Checkbutton(opts_frame, text="Smart Financial Sort (Invoices -> Finance_2026)", variable=self.smart_receipts_var)
        receipt_chk.pack(side="left", padx=(0, 14))

        self.sub_date_var = tk.BooleanVar(value=False)
        sub_date_chk = ttk.Checkbutton(opts_frame, text="Sub-date", variable=self.sub_date_var)
        sub_date_chk.pack(side="left", padx=(0, 14))

        btn_preview = ttk.Button(opts_frame, text="🔍 Preview Actions", style="Secondary.TButton", command=self._on_preview_organize)
        btn_preview.pack(side="right", padx=(8, 0))

        btn_execute = ttk.Button(opts_frame, text="⚡ Execute Now", style="Primary.TButton", command=self._on_execute_organize)
        btn_execute.pack(side="right")

        # Stats
        stats_frame = ttk.Frame(self.tab_organize)
        stats_frame.pack(fill="x", pady=(0, 8))
        for i in range(4):
            stats_frame.columnconfigure(i, weight=1)

        self.card_files = StatCard(stats_frame, "Files Scanned", "0")
        self.card_files.grid(row=0, column=0, sticky="ew", padx=(0, 6))

        self.card_size = StatCard(stats_frame, "Total Size", "0 B")
        self.card_size.grid(row=0, column=1, sticky="ew", padx=6)

        self.card_planned = StatCard(stats_frame, "Planned Moves", "0", value_color=Theme.ACCENT)
        self.card_planned.grid(row=0, column=2, sticky="ew", padx=6)

        self.card_mode = StatCard(stats_frame, "Current Mode", "DRY RUN", value_color=Theme.WARNING)
        self.card_mode.grid(row=0, column=3, sticky="ew", padx=(6, 0))

        # Preview Table
        columns = ["action", "filename", "destination_folder", "size", "status"]
        headings = ["Action", "File Name", "Target Folder", "Size", "Status"]
        self.org_table = ScrollableTreeview(self.tab_organize, columns, headings)
        self.org_table.pack(fill="both", expand=True)

    def _on_preview_organize(self) -> None:
        source_dir = Path(self.org_source_picker.get_path())
        dest_dir = Path(self.org_dest_picker.get_path())

        if not source_dir.exists():
            messagebox.showerror("Error", f"Source directory does not exist:\n{source_dir}")
            return

        self.status_var.set("Scanning directory files...")
        self.update_idletasks()

        files = self.scanner.scan(source_dir, recursive=False, exclude_dirs=[dest_dir] if dest_dir != source_dir else [])
        if not files:
            messagebox.showinfo("Result", "No files found to organize.")
            return

        strat = OrganizeStrategy(self.strategy_var.get())
        self.planned_records = self.organizer.plan_organization(
            files,
            dest_dir,
            strategy=strat,
            sub_organize_by_date=self.sub_date_var.get(),
            smart_receipts=self.smart_receipts_var.get()
        )

        total_bytes = sum(f.size_bytes for f in files)
        planned_moves = sum(1 for r in self.planned_records if r.action_type.value == "MOVE")

        self.card_files.update_value(str(len(files)))
        self.card_size.update_value(Reporter.format_bytes(total_bytes))
        self.card_planned.update_value(str(planned_moves))
        self.card_mode.update_value("DRY RUN" if self.dry_run_var.get() else "LIVE")

        self.org_table.clear()
        for r in self.planned_records[:300]:
            sub = r.destination_path.parent.name if r.destination_path else "-"
            self.org_table.insert_row([
                r.action_type.value,
                r.source_path.name,
                sub,
                Reporter.format_bytes(r.file_size),
                r.status
            ])

        self.status_var.set(f"Preview ready: {planned_moves} planned moves out of {len(files)} files.")

    def _on_execute_organize(self) -> None:
        if not self.planned_records:
            self._on_preview_organize()
            if not self.planned_records:
                return

        source_dir = Path(self.org_source_picker.get_path())
        dest_dir = Path(self.org_dest_picker.get_path())
        dry_run = self.dry_run_var.get()

        if not dry_run:
            confirm = messagebox.askyesno(
                "Confirm Execution",
                f"Are you sure you want to organize {len(self.planned_records)} files in LIVE mode?\n\n"
                f"Source: {source_dir}\nDestination: {dest_dir}\n\n"
                "(Note: You can undo this operation anytime in the Undo tab!)"
            )
            if not confirm:
                return

        self.status_var.set("Executing file operations...")
        self.update_idletasks()

        session = self.executor.execute_session(
            records=self.planned_records,
            source_dir=source_dir,
            destination_dir=dest_dir,
            strategy=self.strategy_var.get(),
            dry_run=dry_run
        )
        self.undo_manager.save_session(session)
        self._load_history_tab()

        self.org_table.clear()
        for r in session.records[:300]:
            sub = r.destination_path.parent.name if r.destination_path else "-"
            self.org_table.insert_row([
                r.action_type.value,
                r.source_path.name,
                sub,
                Reporter.format_bytes(r.file_size),
                r.status
            ])

        mode_str = "Dry-Run Simulation" if dry_run else "Live Cleanup"
        messagebox.showinfo(
            "Session Finished",
            f"{mode_str} Complete!\n\n"
            f"Files Organized: {session.moved_count}\n"
            f"Files Skipped: {session.skipped_count}\n"
            f"Session ID: {session.session_id}"
        )
        self.status_var.set(f"Session {session.session_id} finished ({session.moved_count} moved).")

    # -------------------------------------------------------------
    # TAB 2: Deduplication Center (Hash + pHash)
    # -------------------------------------------------------------
    def _build_dedupe_tab(self) -> None:
        panel = ttk.Frame(self.tab_dedupe, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 10))

        self.dedupe_folder_picker = FolderPicker(panel, label="Target Folder:    ", default_path=str(Path.home() / "Downloads"))
        self.dedupe_folder_picker.pack(fill="x", pady=(0, 6))

        opts_frame = ttk.Frame(panel, style="Card.TFrame")
        opts_frame.pack(fill="x", pady=(4, 0))

        ttk.Label(opts_frame, text="Deduplication Mode:", style="Card.TLabel").pack(side="left", padx=(0, 6))
        self.dedupe_mode_var = tk.StringVar(value="exact")
        mode_combo = ttk.Combobox(
            opts_frame,
            textvariable=self.dedupe_mode_var,
            values=["exact", "visual_phash"],
            state="readonly",
            width=14
        )
        mode_combo.pack(side="left", padx=(0, 14))

        ttk.Label(opts_frame, text="Similarity:", style="Card.TLabel").pack(side="left", padx=(0, 4))
        self.sim_threshold_var = tk.StringVar(value="90%")
        sim_combo = ttk.Combobox(
            opts_frame,
            textvariable=self.sim_threshold_var,
            values=["80%", "85%", "90%", "95%", "100%"],
            state="readonly",
            width=6
        )
        sim_combo.pack(side="left", padx=(0, 14))

        ttk.Label(opts_frame, text="Keep:", style="Card.TLabel").pack(side="left", padx=(0, 4))
        self.keep_policy_var = tk.StringVar(value="oldest")
        keep_combo = ttk.Combobox(
            opts_frame,
            textvariable=self.keep_policy_var,
            values=["oldest", "newest", "shortest_path"],
            state="readonly",
            width=12
        )
        keep_combo.pack(side="left", padx=(0, 14))

        btn_scan_dup = ttk.Button(opts_frame, text="🔍 Scan Duplicates", style="Secondary.TButton", command=self._on_scan_duplicates)
        btn_scan_dup.pack(side="right", padx=(8, 0))

        btn_clean_dup = ttk.Button(opts_frame, text="📦 Move Duplicates", style="Warning.TButton", command=self._on_clean_duplicates)
        btn_clean_dup.pack(side="right")

        # Stats
        stats_frame = ttk.Frame(self.tab_dedupe)
        stats_frame.pack(fill="x", pady=(0, 8))
        for i in range(3):
            stats_frame.columnconfigure(i, weight=1)

        self.card_dup_groups = StatCard(stats_frame, "Duplicate Clusters", "0")
        self.card_dup_groups.grid(row=0, column=0, sticky="ew", padx=(0, 6))

        self.card_dup_files = StatCard(stats_frame, "Redundant Files", "0", value_color=Theme.WARNING)
        self.card_dup_files.grid(row=0, column=1, sticky="ew", padx=6)

        self.card_dup_space = StatCard(stats_frame, "Wasted Disk Space", "0 B", value_color=Theme.DANGER)
        self.card_dup_space.grid(row=0, column=2, sticky="ew", padx=(6, 0))

        # Table
        cols = ["role", "filename", "size", "folder", "hash_or_sim"]
        heads = ["Status", "File Name", "Size", "Folder", "Hash / Similarity"]
        self.dup_table = ScrollableTreeview(self.tab_dedupe, cols, heads)
        self.dup_table.pack(fill="both", expand=True)

    def _on_scan_duplicates(self) -> None:
        target_dir = Path(self.dedupe_folder_picker.get_path())
        if not target_dir.exists():
            messagebox.showerror("Error", f"Folder does not exist:\n{target_dir}")
            return

        is_visual = (self.dedupe_mode_var.get() == "visual_phash")
        self.status_var.set("Scanning for duplicates...")
        self.update_idletasks()

        files = self.scanner.scan(target_dir, recursive=True)

        if is_visual:
            sim_val = float(self.sim_threshold_var.get().replace("%", "")) / 100.0
            self.found_visual_duplicates = self.deduplicator.find_visual_duplicates(files, similarity_threshold=sim_val)
            self.found_duplicates = []

            total_redundant = sum(len(g.files) - 1 for g in self.found_visual_duplicates)
            total_wasted = sum(sum(f.size_bytes for f in g.files if f != g.suggested_keep) for g in self.found_visual_duplicates)

            self.card_dup_groups.update_value(str(len(self.found_visual_duplicates)))
            self.card_dup_files.update_value(str(total_redundant))
            self.card_dup_space.update_value(Reporter.format_bytes(total_wasted))

            self.dup_table.clear()
            for g in self.found_visual_duplicates:
                for f in g.files:
                    is_orig = (f == g.suggested_keep)
                    role = "BEST RESOLUTION (Keep)" if is_orig else "VISUAL DUPLICATE"
                    sim_txt = "100% (Original)" if is_orig else f"{g.similarity_score * 100:.0f}% Similarity"
                    self.dup_table.insert_row([
                        role,
                        f.filename,
                        Reporter.format_bytes(f.size_bytes),
                        str(f.path.parent.name),
                        sim_txt
                    ])
        else:
            self.found_duplicates = self.deduplicator.find_duplicates(files)
            self.found_visual_duplicates = []

            total_redundant = sum(len(g.files) - 1 for g in self.found_duplicates)
            total_wasted = sum(g.wasted_space_bytes for g in self.found_duplicates)

            self.card_dup_groups.update_value(str(len(self.found_duplicates)))
            self.card_dup_files.update_value(str(total_redundant))
            self.card_dup_space.update_value(Reporter.format_bytes(total_wasted))

            self.dup_table.clear()
            for g in self.found_duplicates:
                for f in g.files:
                    is_orig = (f == g.suggested_keep)
                    role = "ORIGINAL (Keep)" if is_orig else "DUPLICATE"
                    self.dup_table.insert_row([
                        role,
                        f.filename,
                        Reporter.format_bytes(f.size_bytes),
                        str(f.path.parent.name),
                        g.file_hash[:16] + "..."
                    ])

        total_groups = len(self.found_visual_duplicates) if is_visual else len(self.found_duplicates)
        if total_groups == 0:
            messagebox.showinfo("Duplicates Scan", "No duplicate files were detected.")
            self.status_var.set("Scan complete: No duplicate files found.")
        else:
            self.status_var.set(f"Found {total_groups} duplicate clusters.")

    def _on_clean_duplicates(self) -> None:
        target_dir = Path(self.dedupe_folder_picker.get_path())
        is_visual = (self.dedupe_mode_var.get() == "visual_phash")

        if is_visual:
            if not self.found_visual_duplicates:
                messagebox.showinfo("Notice", "No visual duplicates to move.")
                return
            records = self.deduplicator.plan_visual_deduplication_actions(self.found_visual_duplicates, target_dir)
        else:
            if not self.found_duplicates:
                messagebox.showinfo("Notice", "No duplicates to move.")
                return
            records = self.deduplicator.plan_deduplication_actions(self.found_duplicates, target_dir)

        if not records:
            return

        confirm = messagebox.askyesno("Confirm Move", f"Move {len(records)} duplicate files to '_Duplicates' folder?")
        if not confirm:
            return

        session = self.executor.execute_session(records, target_dir, target_dir, strategy="deduplication", dry_run=False)
        self.undo_manager.save_session(session)
        self._load_history_tab()
        self._on_scan_duplicates()
        messagebox.showinfo("Complete", f"Successfully moved {session.duplicate_count} duplicates!")

    # -------------------------------------------------------------
    # TAB 3: Smart Bulk Renamer
    # -------------------------------------------------------------
    def _build_renamer_tab(self) -> None:
        panel = ttk.Frame(self.tab_renamer, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 10))

        self.rename_picker = FolderPicker(panel, label="Source Folder: ", default_path=str(Path.home() / "Downloads"))
        self.rename_picker.pack(fill="x", pady=(0, 8))

        row1 = ttk.Frame(panel, style="Card.TFrame")
        row1.pack(fill="x", pady=(0, 6))

        ttk.Label(row1, text="Template:", style="Card.TLabel").pack(side="left", padx=(0, 6))
        self.rename_tpl_var = tk.StringVar(value="{counter:03d}_{name}")
        tpl_entry = ttk.Entry(row1, textvariable=self.rename_tpl_var, width=32)
        tpl_entry.pack(side="left", padx=(0, 14))

        ttk.Label(row1, text="Tokens: {name}, {counter:03d}, {date}, {time}, {camera}, {artist}, {title}", style="Muted.TLabel").pack(side="left")

        row2 = ttk.Frame(panel, style="Card.TFrame")
        row2.pack(fill="x", pady=(0, 4))

        ttk.Label(row2, text="Find:", style="Card.TLabel").pack(side="left", padx=(0, 4))
        self.find_text_var = tk.StringVar(value="")
        ttk.Entry(row2, textvariable=self.find_text_var, width=14).pack(side="left", padx=(0, 10))

        ttk.Label(row2, text="Replace:", style="Card.TLabel").pack(side="left", padx=(0, 4))
        self.replace_text_var = tk.StringVar(value="")
        ttk.Entry(row2, textvariable=self.replace_text_var, width=14).pack(side="left", padx=(0, 10))

        ttk.Label(row2, text="Casing:", style="Card.TLabel").pack(side="left", padx=(0, 4))
        self.rename_case_var = tk.StringVar(value="None")
        case_combo = ttk.Combobox(
            row2,
            textvariable=self.rename_case_var,
            values=["None", "lowercase", "uppercase", "title_case", "snake_case", "kebab_case"],
            state="readonly",
            width=12
        )
        case_combo.pack(side="left", padx=(0, 14))

        btn_preview_rename = ttk.Button(row2, text="🔍 Preview Rename", style="Secondary.TButton", command=self._on_preview_rename)
        btn_preview_rename.pack(side="right", padx=(8, 0))

        btn_exec_rename = ttk.Button(row2, text="🏷️ Rename Files", style="Primary.TButton", command=self._on_execute_rename)
        btn_exec_rename.pack(side="right")

        # Table
        cols = ["action", "old_name", "new_name", "status"]
        heads = ["Action", "Original Filename", "New Filename", "Status"]
        self.rename_table = ScrollableTreeview(self.tab_renamer, cols, heads)
        self.rename_table.pack(fill="both", expand=True)

    def _on_preview_rename(self) -> None:
        source_dir = Path(self.rename_picker.get_path())
        if not source_dir.exists():
            messagebox.showerror("Error", f"Folder does not exist:\n{source_dir}")
            return

        files = self.scanner.scan(source_dir, recursive=False)
        case_val = None if self.rename_case_var.get() == "None" else self.rename_case_var.get()

        self.rename_records = self.renamer.plan_rename(
            files,
            template=self.rename_tpl_var.get(),
            find_text=self.find_text_var.get(),
            replace_text=self.replace_text_var.get(),
            case_transform=case_val
        )

        self.rename_table.clear()
        for r in self.rename_records[:300]:
            target_name = r.destination_path.name if r.destination_path else "-"
            self.rename_table.insert_row([r.action_type.value, r.source_path.name, target_name, r.status])

        self.status_var.set(f"Rename preview: {len(self.rename_records)} files.")

    def _on_execute_rename(self) -> None:
        if not self.rename_records:
            self._on_preview_rename()
            if not self.rename_records:
                return

        source_dir = Path(self.rename_picker.get_path())
        confirm = messagebox.askyesno("Confirm Rename", f"Are you sure you want to rename {len(self.rename_records)} files?")
        if not confirm:
            return

        session = self.executor.execute_session(self.rename_records, source_dir, source_dir, strategy="rename", dry_run=False)
        self.undo_manager.save_session(session)
        self._load_history_tab()
        self._on_preview_rename()
        messagebox.showinfo("Rename Complete", f"Successfully renamed {session.moved_count} files! (Undoable anytime)")

    # -------------------------------------------------------------
    # TAB 4: Storage & Treemap
    # -------------------------------------------------------------
    def _build_storage_tab(self) -> None:
        panel = ttk.Frame(self.tab_storage, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 10))

        self.storage_picker = FolderPicker(panel, label="Analyze Folder: ", default_path=str(Path.home() / "Downloads"))
        self.storage_picker.pack(fill="x", pady=(0, 6))

        row = ttk.Frame(panel, style="Card.TFrame")
        row.pack(fill="x")

        self.storage_status_lbl = ttk.Label(row, text="Click 'Analyze Storage' to compute space distribution.", style="Muted.TLabel")
        self.storage_status_lbl.pack(side="left")

        btn_analyze = ttk.Button(row, text="📊 Analyze Storage", style="Primary.TButton", command=self._on_analyze_storage)
        btn_analyze.pack(side="right")

        # Split pane: Left Canvas, Right Top Files
        split_frame = ttk.Frame(self.tab_storage)
        split_frame.pack(fill="both", expand=True)
        split_frame.columnconfigure(0, weight=3)
        split_frame.columnconfigure(1, weight=2)
        split_frame.rowconfigure(0, weight=1)

        # Treemap Canvas
        canvas_card = ttk.Frame(split_frame, style="Card.TFrame", padding=8)
        canvas_card.grid(row=0, column=0, sticky="nsew", padx=(0, 6))

        ttk.Label(canvas_card, text="Storage Treemap Distribution", style="Header.TLabel").pack(anchor="w", pady=(0, 4))
        self.treemap_canvas = tk.Canvas(canvas_card, bg="#131d31", highlightthickness=0)
        self.treemap_canvas.pack(fill="both", expand=True)

        # Largest Files table
        table_card = ttk.Frame(split_frame, style="Card.TFrame", padding=8)
        table_card.grid(row=0, column=1, sticky="nsew", padx=(6, 0))

        ttk.Label(table_card, text="Top Storage-Eating Files", style="Header.TLabel").pack(anchor="w", pady=(0, 4))
        cols = ["name", "size"]
        heads = ["File Name", "Size"]
        self.largest_table = ScrollableTreeview(table_card, cols, heads)
        self.largest_table.pack(fill="both", expand=True)

    def _on_analyze_storage(self) -> None:
        target = Path(self.storage_picker.get_path())
        if not target.exists():
            messagebox.showerror("Error", f"Folder does not exist:\n{target}")
            return

        self.status_var.set("Analyzing disk storage...")
        self.update_idletasks()

        root_node = DiskAnalyzer.analyze_directory(target, max_depth=2)
        largest = DiskAnalyzer.find_largest_files(target, limit=15)

        self.storage_status_lbl.config(text=f"Total Storage: {Reporter.format_bytes(root_node.size)} ({len(root_node.children)} direct subfolders/files)")

        # Render Largest files
        self.largest_table.clear()
        for f in largest:
            self.largest_table.insert_row([f["name"], Reporter.format_bytes(f["size"])])

        # Draw Treemap on Canvas
        self.treemap_canvas.delete("all")
        cw = self.treemap_canvas.winfo_width() or 450
        ch = self.treemap_canvas.winfo_height() or 300

        DiskAnalyzer.layout_treemap(root_node, x1=10, y1=10, x2=cw - 10, y2=ch - 10)

        for child in root_node.children:
            x1, y1, x2, y2 = child.rect
            if x2 - x1 > 2 and y2 - y1 > 2:
                self.treemap_canvas.create_rectangle(
                    x1, y1, x2, y2,
                    fill=child.color,
                    outline="#0f172a",
                    width=2
                )
                if (x2 - x1) > 40 and (y2 - y1) > 20:
                    text_label = f"{child.name[:12]}\n{Reporter.format_bytes(child.size)}"
                    self.treemap_canvas.create_text(
                        (x1 + x2) / 2, (y1 + y2) / 2,
                        text=text_label,
                        fill="#000000",
                        font=(Theme.FONT_FAMILY, 8, "bold")
                    )

        self.status_var.set(f"Storage analysis complete: {Reporter.format_bytes(root_node.size)}.")

    # -------------------------------------------------------------
    # TAB 5: Security & Dev Junk
    # -------------------------------------------------------------
    def _build_security_tab(self) -> None:
        notebook_sec = ttk.Notebook(self.tab_security)
        notebook_sec.pack(fill="both", expand=True)

        tab_junk = ttk.Frame(notebook_sec, padding=10)
        tab_secrets = ttk.Frame(notebook_sec, padding=10)

        notebook_sec.add(tab_junk, text="  🧹 Dev Junk Cleaner  ")
        notebook_sec.add(tab_secrets, text="  🛡️ Secret Scanner (DLP)  ")

        # --- JUNK PANEL ---
        panel_j = ttk.Frame(tab_junk, style="Card.TFrame", padding=12)
        panel_j.pack(fill="x", pady=(0, 8))

        self.junk_picker = FolderPicker(panel_j, label="Scan Folder: ", default_path=str(Path.home() / "Documents"))
        self.junk_picker.pack(fill="x", pady=(0, 6))

        row_j = ttk.Frame(panel_j, style="Card.TFrame")
        row_j.pack(fill="x")

        self.junk_status_var = tk.StringVar(value="Scan for node_modules, .venv, and build caches.")
        ttk.Label(row_j, textvariable=self.junk_status_var, style="Muted.TLabel").pack(side="left")

        btn_scan_j = ttk.Button(row_j, text="🔍 Scan Junk", style="Secondary.TButton", command=self._on_scan_junk)
        btn_scan_j.pack(side="right", padx=(6, 0))

        btn_clean_j = ttk.Button(row_j, text="🗑️ Clean Discovered Junk", style="Danger.TButton", command=self._on_clean_junk)
        btn_clean_j.pack(side="right")

        cols_j = ["category", "name", "size", "path"]
        heads_j = ["Category", "Artifact Name", "Size", "Folder Path"]
        self.junk_table = ScrollableTreeview(tab_junk, cols_j, heads_j)
        self.junk_table.pack(fill="both", expand=True)
        self.discovered_junk: List[JunkItem] = []

        # --- SECRETS PANEL ---
        panel_s = ttk.Frame(tab_secrets, style="Card.TFrame", padding=12)
        panel_s.pack(fill="x", pady=(0, 8))

        self.secret_picker = FolderPicker(panel_s, label="Scan Folder: ", default_path=str(Path.home() / "Documents"))
        self.secret_picker.pack(fill="x", pady=(0, 6))

        row_s = ttk.Frame(panel_s, style="Card.TFrame")
        row_s.pack(fill="x")

        self.secret_status_var = tk.StringVar(value="Scan for exposed API keys, tokens, and SSH private keys.")
        ttk.Label(row_s, textvariable=self.secret_status_var, style="Muted.TLabel").pack(side="left")

        btn_scan_s = ttk.Button(row_s, text="🔍 Scan Secrets", style="Secondary.TButton", command=self._on_scan_secrets)
        btn_scan_s.pack(side="right", padx=(6, 0))

        btn_quarantine = ttk.Button(row_s, text="🛡️ Quarantine Compromised Files", style="Warning.TButton", command=self._on_quarantine_secrets)
        btn_quarantine.pack(side="right")

        cols_s = ["severity", "type", "file", "line", "snippet"]
        heads_s = ["Severity", "Secret Type", "File Name", "Line", "Masked Snippet"]
        self.secret_table = ScrollableTreeview(tab_secrets, cols_s, heads_s)
        self.secret_table.pack(fill="both", expand=True)
        self.discovered_secrets: List[SecretFinding] = []

    def _on_scan_junk(self) -> None:
        target = Path(self.junk_picker.get_path())
        if not target.exists():
            messagebox.showerror("Error", f"Folder does not exist:\n{target}")
            return

        self.status_var.set("Scanning for developer junk...")
        self.update_idletasks()

        self.discovered_junk = DevJunkCleaner.scan_directory(target, inactive_days=14)
        total_size = sum(item.size_bytes for item in self.discovered_junk)

        self.junk_table.clear()
        for item in self.discovered_junk:
            self.junk_table.insert_row([item.category, item.name, Reporter.format_bytes(item.size_bytes), str(item.path)])

        self.junk_status_var.set(f"Found {len(self.discovered_junk)} junk items ({Reporter.format_bytes(total_size)} reclaimable).")
        self.status_var.set(f"Discovered {len(self.discovered_junk)} junk items.")

    def _on_clean_junk(self) -> None:
        if not self.discovered_junk:
            messagebox.showinfo("Notice", "No junk items discovered. Run 'Scan Junk' first.")
            return

        confirm = messagebox.askyesno(
            "Confirm Delete",
            f"Are you sure you want to permanently delete {len(self.discovered_junk)} junk folders?"
        )
        if not confirm:
            return

        res = DevJunkCleaner.clean_junk(self.discovered_junk, dry_run=False)
        self._on_scan_junk()
        messagebox.showinfo("Cleanup Finished", res["message"])

    def _on_scan_secrets(self) -> None:
        target = Path(self.secret_picker.get_path())
        if not target.exists():
            messagebox.showerror("Error", f"Folder does not exist:\n{target}")
            return

        self.status_var.set("Scanning for sensitive credentials...")
        self.update_idletasks()

        self.discovered_secrets = SecretScanner.scan_directory(target, recursive=True)
        self.secret_table.clear()
        for f in self.discovered_secrets:
            self.secret_table.insert_row([f.severity, f.secret_type, f.file_path.name, str(f.line_number), f.masked_snippet])

        if not self.discovered_secrets:
            self.secret_status_var.set("Clean! No exposed secrets detected.")
            messagebox.showinfo("Clean Scan", "No sensitive tokens or secrets were found.")
        else:
            self.secret_status_var.set(f"ALERT: Found {len(self.discovered_secrets)} exposed secrets!")
            messagebox.showwarning("Security Alert", f"Detected {len(self.discovered_secrets)} sensitive credentials!")

    def _on_quarantine_secrets(self) -> None:
        if not self.discovered_secrets:
            messagebox.showinfo("Notice", "No secret leaks to quarantine.")
            return

        target = Path(self.secret_picker.get_path())
        confirm = messagebox.askyesno("Confirm Quarantine", f"Move {len(self.discovered_secrets)} files into '_Quarantine' folder?")
        if not confirm:
            return

        res = SecretScanner.quarantine_files(self.discovered_secrets, target)
        self._on_scan_secrets()
        messagebox.showinfo("Quarantine Finished", res["message"])
    def _build_sync_tab(self) -> None:
        panel = ttk.Frame(self.tab_sync, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 10))

        # USB Banner
        usb_bar = ttk.Frame(panel, style="Card.TFrame")
        usb_bar.pack(fill="x", pady=(0, 8))

        self.usb_label_var = tk.StringVar(value="Detecting removable drives...")
        ttk.Label(usb_bar, textvariable=self.usb_label_var, style="Title.TLabel", font=(Theme.FONT_FAMILY, 10, "bold")).pack(side="left")

        btn_detect = ttk.Button(usb_bar, text="🔄 Refresh USB Drives", style="Secondary.TButton", command=self._refresh_usb_drives)
        btn_detect.pack(side="right")

        self.sync_source_picker = FolderPicker(panel, label="Source Folder:     ", default_path=str(Path.home() / "Documents"))
        self.sync_source_picker.pack(fill="x", pady=(0, 6))

        self.sync_dest_picker = FolderPicker(panel, label="Target / USB Path: ", default_path="D:/Backup")
        self.sync_dest_picker.pack(fill="x", pady=(0, 6))

        row = ttk.Frame(panel, style="Card.TFrame")
        row.pack(fill="x", pady=(4, 0))

        ttk.Label(row, text="Sync Mode:", style="Card.TLabel").pack(side="left", padx=(0, 6))
        self.sync_mode_var = tk.StringVar(value="incremental")
        mode_combo = ttk.Combobox(
            row,
            textvariable=self.sync_mode_var,
            values=["incremental", "mirror", "snapshot"],
            state="readonly",
            width=14
        )
        mode_combo.pack(side="left", padx=(0, 16))

        btn_sync = ttk.Button(row, text="💾 Sync Now", style="Primary.TButton", command=self._on_execute_sync)
        btn_sync.pack(side="right")

        # Log Text Box
        ttk.Label(self.tab_sync, text="Synchronization Activity Log", style="Header.TLabel").pack(anchor="w", pady=(6, 4))
        self.sync_log_text = tk.Text(self.tab_sync, bg=Theme.CARD_BG, fg=Theme.TEXT_LIGHT, font=Theme.FONT_MONO, height=12)
        self.sync_log_text.pack(fill="both", expand=True)

    def _refresh_usb_drives(self) -> None:
        drives = DriveDetector.get_removable_drives()
        if not drives:
            self.usb_label_var.set("No removable USB drive detected.")
        else:
            names = [f"{d['path']} ({d['label']})" for d in drives]
            self.usb_label_var.set("Connected USB: " + ", ".join(names))
            if drives:
                self.sync_dest_picker.set_path(drives[0]["path"])

    def _on_execute_sync(self) -> None:
        src = Path(self.sync_source_picker.get_path())
        dst = Path(self.sync_dest_picker.get_path())

        if not src.exists():
            messagebox.showerror("Error", f"Source folder not found:\n{src}")
            return

        mode = SyncMode(self.sync_mode_var.get())
        profile = SyncProfile(name="Desktop Sync", source_dir=src, dest_dir=dst, mode=mode)

        self.sync_log_text.insert("end", f"\n[*] Starting {mode.value.upper()} sync: {src} -> {dst}\n")
        self.update_idletasks()

        res = FolderSynchronizer.sync(profile, dry_run=False)
        self.sync_log_text.insert("end", f"[+] {res['message']}\n")
        if res.get("errors"):
            for err in res["errors"]:
                self.sync_log_text.insert("end", f"[!] Error: {err}\n")
        self.sync_log_text.see("end")
        messagebox.showinfo("Sync Finished", res["message"])

    # -------------------------------------------------------------
    # TAB 5: Undo & History
    # -------------------------------------------------------------
    def _build_history_tab(self) -> None:
        top_bar = ttk.Frame(self.tab_history)
        top_bar.pack(fill="x", pady=(0, 10))

        ttk.Label(top_bar, text="Recorded Organization Sessions", style="Header.TLabel").pack(side="left")

        btn_undo = ttk.Button(top_bar, text="⏪ Undo Selected Session", style="Danger.TButton", command=self._on_undo_session)
        btn_undo.pack(side="right", padx=(8, 0))

        btn_refresh = ttk.Button(top_bar, text="🔄 Refresh", style="Secondary.TButton", command=self._load_history_tab)
        btn_refresh.pack(side="right")

        cols = ["session_id", "date", "strategy", "mode", "moved", "duplicates", "status"]
        heads = ["Session ID", "Date & Time", "Strategy", "Mode", "Files Moved", "Dups", "Status"]
        self.history_table = ScrollableTreeview(self.tab_history, cols, heads)
        self.history_table.pack(fill="both", expand=True)

    def _load_history_tab(self) -> None:
        self.history_table.clear()
        sessions = self.undo_manager.list_sessions(limit=30)
        for s in sessions:
            self.history_table.insert_row([
                s["session_id"],
                s["start_time"][:19].replace("T", " "),
                s["strategy"],
                "Dry-Run" if s["dry_run"] else "Live",
                s["moved_count"],
                s["duplicate_count"],
                s["status"]
            ])

    def _on_undo_session(self) -> None:
        selected = self.history_table.tree.selection()
        if not selected:
            messagebox.showinfo("Select Session", "Please select a session from the list to undo.")
            return

        item = self.history_table.tree.item(selected[0])
        session_id = item["values"][0]
        status = item["values"][6]

        if status == "ROLLED_BACK":
            messagebox.showwarning("Notice", "This session has already been rolled back.")
            return
        if item["values"][3] == "Dry-Run":
            messagebox.showinfo("Notice", "Cannot rollback a Dry-Run session.")
            return

        confirm = messagebox.askyesno(
            "Confirm Undo",
            f"Are you sure you want to rollback session:\n{session_id}?\n\n"
            "All moved files will be restored to their exact original locations!"
        )
        if not confirm:
            return

        res = self.undo_manager.rollback_session(session_id)
        if res["success"]:
            messagebox.showinfo("Rollback Successful", f"Restored {res['restored_count']} files back to their original paths!")
            self._load_history_tab()
        else:
            messagebox.showerror("Rollback Error", res["message"])

    # -------------------------------------------------------------
    # TAB 8: Rules & Settings
    # -------------------------------------------------------------
    def _build_settings_tab(self) -> None:
        # Shell Integration Card
        shell_card = ttk.Frame(self.tab_settings, style="Card.TFrame", padding=16)
        shell_card.pack(fill="x", pady=(0, 14))

        ttk.Label(shell_card, text="Windows Explorer Context Menu Integration", style="Header.TLabel").pack(anchor="w", pady=(0, 4))
        ttk.Label(
            shell_card,
            text="Adds right-click shortcuts ('Smart Organize Here', 'Find Duplicates Here', 'Open in GUI') directly into Windows Explorer.",
            style="Muted.TLabel"
        ).pack(anchor="w", pady=(0, 8))

        row_shell = ttk.Frame(shell_card, style="Card.TFrame")
        row_shell.pack(fill="x")

        self.shell_status_var = tk.StringVar()
        self._update_shell_status()

        ttk.Label(row_shell, textvariable=self.shell_status_var, style="Card.TLabel", font=(Theme.FONT_FAMILY, 9, "bold")).pack(side="left")

        btn_uninstall_shell = ttk.Button(row_shell, text="❌ Remove Context Menu", style="Danger.TButton", command=self._on_uninstall_shell)
        btn_uninstall_shell.pack(side="right", padx=(8, 0))

        btn_install_shell = ttk.Button(row_shell, text="✨ Register Context Menu", style="Primary.TButton", command=self._on_install_shell)
        btn_install_shell.pack(side="right")

        info_card = ttk.Frame(self.tab_settings, style="Card.TFrame", padding=16)
        info_card.pack(fill="x", pady=(0, 14))

        ttk.Label(info_card, text="Category File Formats", style="Header.TLabel").pack(anchor="w", pady=(0, 6))
        ttk.Label(
            info_card,
            text="Smart File Organizer maps over 150+ file extensions into clean categories automatically.\n"
                 "Settings and custom extensions are persisted to your local profile (~/.smart_file_organizer/config.json).",
            style="Muted.TLabel"
        ).pack(anchor="w")

        cols = ["category", "extensions"]
        heads = ["Category Folder", "Supported File Extensions"]
        self.settings_table = ScrollableTreeview(self.tab_settings, cols, heads)
        self.settings_table.pack(fill="both", expand=True)

        for cat, exts in self.config_manager.categories.items():
            self.settings_table.insert_row([cat, ", ".join(exts)])

    def _update_shell_status(self) -> None:
        if not WindowsShellIntegration.is_supported():
            self.shell_status_var.set("Shell integration: Windows only.")
        elif WindowsShellIntegration.is_installed():
            self.shell_status_var.set("✅ Status: Context menu is registered in Windows Explorer.")
        else:
            self.shell_status_var.set("⚪ Status: Context menu is not registered.")

    def _on_install_shell(self) -> None:
        res = WindowsShellIntegration.install()
        self._update_shell_status()
        if res["success"]:
            messagebox.showinfo("Success", res["message"])
        else:
            messagebox.showerror("Error", res["message"])

    def _on_uninstall_shell(self) -> None:
        res = WindowsShellIntegration.uninstall()
        self._update_shell_status()
        if res["success"]:
            messagebox.showinfo("Success", res["message"])
        else:
            messagebox.showerror("Error", res["message"])


def launch_gui() -> None:
    """Launches the Tkinter desktop application."""
    app = SmartFileOrganizerApp()
    app.mainloop()


if __name__ == "__main__":
    launch_gui()
