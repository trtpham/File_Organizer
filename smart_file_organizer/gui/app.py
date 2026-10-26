"""
Main Tkinter Desktop Application for Smart File Organizer.
"""

import sys
import tkinter as tk
from tkinter import ttk, messagebox
from pathlib import Path
from typing import Optional, List

from .theme import Theme, apply_theme
from .components import StatCard, FolderPicker, ScrollableTreeview
from ..core.config import ConfigManager
from ..core.scanner import FileScanner
from ..core.organizer import Organizer
from ..core.deduplicator import Deduplicator
from ..core.executor import TransactionExecutor
from ..core.undo_manager import UndoManager
from ..core.reporter import Reporter
from ..core.models import OrganizeStrategy, ActionRecord, DuplicateGroup


class SmartFileOrganizerApp(tk.Tk):
    """Main desktop application window."""

    def __init__(self):
        super().__init__()
        self.title("Smart File Organizer v2.0 - Enterprise Edition")
        self.geometry("1050x700")
        self.minsize(850, 550)

        self.config_manager = ConfigManager()
        self.undo_manager = UndoManager()
        self.scanner = FileScanner(self.config_manager)
        self.organizer = Organizer(self.config_manager)
        self.deduplicator = Deduplicator(self.config_manager)
        self.executor = TransactionExecutor(self.config_manager)

        self.planned_records: List[ActionRecord] = []
        self.found_duplicates: List[DuplicateGroup] = []

        apply_theme(self)
        self._build_ui()
        self._load_history_tab()

    def _build_ui(self) -> None:
        """Constructs application layout."""
        # Main Header
        header_frame = ttk.Frame(self, padding=(20, 16, 20, 10))
        header_frame.pack(fill="x")

        title_lbl = ttk.Label(header_frame, text="📂 Smart File Organizer", style="Title.TLabel")
        title_lbl.pack(side="left")

        sub_lbl = ttk.Label(
            header_frame,
            text="Automated Cleanup • Deep Deduplication • 100% Rollback",
            style="Muted.TLabel"
        )
        sub_lbl.pack(side="left", padx=(16, 0), pady=(4, 0))

        # Main Tabbed Notebook
        self.notebook = ttk.Notebook(self)
        self.notebook.pack(fill="both", expand=True, padx=20, pady=(0, 10))

        # Build tabs
        self.tab_organize = ttk.Frame(self.notebook, padding=16)
        self.tab_dedupe = ttk.Frame(self.notebook, padding=16)
        self.tab_history = ttk.Frame(self.notebook, padding=16)
        self.tab_settings = ttk.Frame(self.notebook, padding=16)

        self.notebook.add(self.tab_organize, text="  🧹 Organize & Clean  ")
        self.notebook.add(self.tab_dedupe, text="  🔍 Duplicate Center  ")
        self.notebook.add(self.tab_history, text="  ⏪ Undo & History  ")
        self.notebook.add(self.tab_settings, text="  ⚙️ Rules & Settings  ")

        self._build_organize_tab()
        self._build_dedupe_tab()
        self._build_history_tab()
        self._build_settings_tab()

        # Status Bar
        status_bar = ttk.Frame(self, padding=(20, 6, 20, 8), style="Card.TFrame")
        status_bar.pack(fill="x", side="bottom")

        self.status_var = tk.StringVar(value="Ready. Select a folder to organize or scan for duplicates.")
        status_lbl = ttk.Label(status_bar, textvariable=self.status_var, style="Card.TLabel", font=Theme.FONT_SMALL)
        status_lbl.pack(side="left")

    # -------------------------------------------------------------
    # TAB 1: Organize & Clean
    # -------------------------------------------------------------
    def _build_organize_tab(self) -> None:
        panel = ttk.Frame(self.tab_organize, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 12))

        # Source & Dest Pickers
        self.org_source_picker = FolderPicker(panel, label="Source Folder:     ", default_path=str(Path.home() / "Downloads"))
        self.org_source_picker.pack(fill="x", pady=(0, 8))

        self.org_dest_picker = FolderPicker(panel, label="Destination Folder:", default_path=str(Path.home() / "Downloads"))
        self.org_dest_picker.pack(fill="x", pady=(0, 8))

        # Options Row
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
        strategy_combo.pack(side="left", padx=(0, 16))

        self.dry_run_var = tk.BooleanVar(value=True)
        dry_chk = ttk.Checkbutton(opts_frame, text="Dry-Run Mode (Safe Preview)", variable=self.dry_run_var)
        dry_chk.pack(side="left", padx=(0, 16))

        self.sub_date_var = tk.BooleanVar(value=False)
        sub_date_chk = ttk.Checkbutton(opts_frame, text="Sub-organize by Date", variable=self.sub_date_var)
        sub_date_chk.pack(side="left", padx=(0, 16))

        # Action Buttons
        btn_preview = ttk.Button(opts_frame, text="🔍 Preview Actions", style="Secondary.TButton", command=self._on_preview_organize)
        btn_preview.pack(side="right", padx=(8, 0))

        btn_execute = ttk.Button(opts_frame, text="⚡ Execute Now", style="Primary.TButton", command=self._on_execute_organize)
        btn_execute.pack(side="right")

        # Stat cards row
        stats_frame = ttk.Frame(self.tab_organize)
        stats_frame.pack(fill="x", pady=(0, 10))
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
            sub_organize_by_date=self.sub_date_var.get()
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
    # TAB 2: Deduplication Center
    # -------------------------------------------------------------
    def _build_dedupe_tab(self) -> None:
        panel = ttk.Frame(self.tab_dedupe, style="Card.TFrame", padding=14)
        panel.pack(fill="x", pady=(0, 12))

        self.dedupe_folder_picker = FolderPicker(panel, label="Target Folder:    ", default_path=str(Path.home() / "Downloads"))
        self.dedupe_folder_picker.pack(fill="x", pady=(0, 8))

        opts_frame = ttk.Frame(panel, style="Card.TFrame")
        opts_frame.pack(fill="x", pady=(4, 0))

        ttk.Label(opts_frame, text="Keep Original:", style="Card.TLabel").pack(side="left", padx=(0, 6))
        self.keep_policy_var = tk.StringVar(value="oldest")
        keep_combo = ttk.Combobox(
            opts_frame,
            textvariable=self.keep_policy_var,
            values=["oldest", "newest", "shortest_path"],
            state="readonly",
            width=14
        )
        keep_combo.pack(side="left", padx=(0, 16))

        btn_scan_dup = ttk.Button(opts_frame, text="🔍 Scan Duplicates", style="Secondary.TButton", command=self._on_scan_duplicates)
        btn_scan_dup.pack(side="right", padx=(8, 0))

        btn_clean_dup = ttk.Button(opts_frame, text="📦 Move Duplicates to _Duplicates", style="Warning.TButton", command=self._on_clean_duplicates)
        btn_clean_dup.pack(side="right")

        # Stats
        stats_frame = ttk.Frame(self.tab_dedupe)
        stats_frame.pack(fill="x", pady=(0, 10))
        for i in range(3):
            stats_frame.columnconfigure(i, weight=1)

        self.card_dup_groups = StatCard(stats_frame, "Duplicate Clusters", "0")
        self.card_dup_groups.grid(row=0, column=0, sticky="ew", padx=(0, 6))

        self.card_dup_files = StatCard(stats_frame, "Redundant Files", "0", value_color=Theme.WARNING)
        self.card_dup_files.grid(row=0, column=1, sticky="ew", padx=6)

        self.card_dup_space = StatCard(stats_frame, "Wasted Disk Space", "0 B", value_color=Theme.DANGER)
        self.card_dup_space.grid(row=0, column=2, sticky="ew", padx=(6, 0))

        # Table
        cols = ["role", "filename", "size", "folder", "hash"]
        heads = ["Status", "File Name", "Size", "Folder", "SHA-256 Hash"]
        self.dup_table = ScrollableTreeview(self.tab_dedupe, cols, heads)
        self.dup_table.pack(fill="both", expand=True)

    def _on_scan_duplicates(self) -> None:
        target_dir = Path(self.dedupe_folder_picker.get_path())
        if not target_dir.exists():
            messagebox.showerror("Error", f"Folder does not exist:\n{target_dir}")
            return

        self.status_var.set("Scanning and running 3-tiered duplicate identification...")
        self.update_idletasks()

        files = self.scanner.scan(target_dir, recursive=True)
        self.found_duplicates = self.deduplicator.find_duplicates(files)

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

        if not self.found_duplicates:
            messagebox.showinfo("Duplicates Scan", "Great news! No duplicate files were detected.")
            self.status_var.set("Scan complete: No duplicate files found.")
        else:
            self.status_var.set(f"Found {len(self.found_duplicates)} duplicate clusters ({Reporter.format_bytes(total_wasted)} wasted).")

    def _on_clean_duplicates(self) -> None:
        if not self.found_duplicates:
            messagebox.showinfo("Notice", "Please run 'Scan Duplicates' first.")
            return

        target_dir = Path(self.dedupe_folder_picker.get_path())
        records = self.deduplicator.plan_deduplication_actions(
            self.found_duplicates,
            target_dir,
            action_mode="move_to_duplicates"
        )
        if not records:
            messagebox.showinfo("Notice", "No duplicates available to move.")
            return

        confirm = messagebox.askyesno(
            "Confirm Move",
            f"Move {len(records)} duplicate files to '{target_dir / '_Duplicates'}'?\n"
            "Original files will remain untouched!"
        )
        if not confirm:
            return

        session = self.executor.execute_session(
            records=records,
            source_dir=target_dir,
            destination_dir=target_dir,
            strategy="deduplication",
            dry_run=False
        )
        self.undo_manager.save_session(session)
        self._load_history_tab()
        self._on_scan_duplicates()

        messagebox.showinfo(
            "Cleanup Complete",
            f"Successfully isolated {session.duplicate_count} duplicate files into '_Duplicates' folder!"
        )

    # -------------------------------------------------------------
    # TAB 3: Undo & History
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
    # TAB 4: Rules & Settings
    # -------------------------------------------------------------
    def _build_settings_tab(self) -> None:
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


def launch_gui() -> None:
    """Launches the Tkinter desktop application."""
    app = SmartFileOrganizerApp()
    app.mainloop()


if __name__ == "__main__":
    launch_gui()
