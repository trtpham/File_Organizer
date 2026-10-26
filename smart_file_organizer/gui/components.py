"""
Reusable modern UI components for Smart File Organizer GUI.
"""

import tkinter as tk
from tkinter import ttk, filedialog
from typing import Optional, Callable
from .theme import Theme


class StatCard(ttk.Frame):
    """Modern dashboard metric card showing key statistics."""

    def __init__(self, parent, title: str, initial_value: str = "0", value_color: Optional[str] = None):
        super().__init__(parent, style="Card.TFrame", padding=12)
        self.columnconfigure(0, weight=1)

        self.title_lbl = ttk.Label(self, text=title.upper(), style="Muted.TLabel")
        self.title_lbl.pack(anchor="w")

        color = value_color or Theme.TEXT_LIGHT
        self.val_lbl = tk.Label(
            self,
            text=initial_value,
            font=(Theme.FONT_FAMILY, 18, "bold"),
            bg=Theme.CARD_BG,
            fg=color
        )
        self.val_lbl.pack(anchor="w", pady=(4, 0))

    def update_value(self, new_val: str) -> None:
        self.val_lbl.configure(text=new_val)


class FolderPicker(ttk.Frame):
    """File path entry field with a browse button."""

    def __init__(self, parent, label: str, default_path: str = "", on_change: Optional[Callable[[str], None]] = None):
        super().__init__(parent, style="TFrame")
        self.on_change = on_change
        self.columnconfigure(1, weight=1)

        lbl = ttk.Label(self, text=label, font=Theme.FONT_BODY)
        lbl.grid(row=0, column=0, sticky="w", padx=(0, 8))

        self.path_var = tk.StringVar(value=default_path)
        self.entry = ttk.Entry(self, textvariable=self.path_var)
        self.entry.grid(row=0, column=1, sticky="ew", padx=(0, 8))

        self.btn = ttk.Button(self, text="Browse...", style="Secondary.TButton", command=self._browse)
        self.btn.grid(row=0, column=2, sticky="e")

    def _browse(self) -> None:
        chosen = filedialog.askdirectory(initialdir=self.path_var.get() or ".")
        if chosen:
            self.path_var.set(chosen)
            if self.on_change:
                self.on_change(chosen)

    def get_path(self) -> str:
        return self.path_var.get().strip()

    def set_path(self, path: str) -> None:
        self.path_var.set(path)


class ScrollableTreeview(ttk.Frame):
    """Treeview widget with integrated scrollbars and column auto-sizing."""

    def __init__(self, parent, columns: list, headings: list):
        super().__init__(parent, style="TFrame")
        self.columnconfigure(0, weight=1)
        self.rowconfigure(0, weight=1)

        self.tree = ttk.Treeview(self, columns=columns, show="headings", selectmode="browse")

        v_scroll = ttk.Scrollbar(self, orient="vertical", command=self.tree.yview)
        h_scroll = ttk.Scrollbar(self, orient="horizontal", command=self.tree.xview)
        self.tree.configure(yscrollcommand=v_scroll.set, xscrollcommand=h_scroll.set)

        self.tree.grid(row=0, column=0, sticky="nsew")
        v_scroll.grid(row=0, column=1, sticky="ns")
        h_scroll.grid(row=1, column=0, sticky="ew")

        for col, heading in zip(columns, headings):
            self.tree.heading(col, text=heading)
            self.tree.column(col, anchor="w", width=120)

    def clear(self) -> None:
        for item in self.tree.get_children():
            self.tree.delete(item)

    def insert_row(self, values: list, tags: tuple = ()) -> str:
        return self.tree.insert("", "end", values=values, tags=tags)
