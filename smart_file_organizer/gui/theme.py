"""
Modern Dark Theme configuration for Tkinter / ttk widgets.
"""

import tkinter as tk
from tkinter import ttk


class Theme:
    # Modern Slate Dark Palette
    BG_DARK = "#0f172a"        # Deep slate background
    CARD_BG = "#1e293b"        # Card / panel background
    CARD_BORDER = "#334155"    # Subtle borders
    ACCENT = "#38bdf8"         # Sky blue primary accent
    ACCENT_HOVER = "#0284c7"   # Sky blue hover
    SUCCESS = "#22c55e"        # Green for safe/success
    WARNING = "#f59e0b"        # Amber for dry-run/duplicates
    DANGER = "#ef4444"         # Red for delete/errors
    TEXT_LIGHT = "#f8fafc"     # High-contrast white text
    TEXT_MUTED = "#94a3b8"     # Subtle secondary text
    TREE_SELECT = "#2563eb"    # Selection color

    FONT_FAMILY = "Segoe UI" if tk.Tk().tk.call("tk", "windowingsystem") == "win32" else "Helvetica"
    FONT_TITLE = (FONT_FAMILY, 15, "bold")
    FONT_HEADER = (FONT_FAMILY, 11, "bold")
    FONT_BODY = (FONT_FAMILY, 9)
    FONT_SMALL = (FONT_FAMILY, 8)
    FONT_MONO = ("Consolas", 9)


def apply_theme(root: tk.Tk) -> ttk.Style:
    """Applies modern dark styling to standard ttk widgets."""
    root.configure(bg=Theme.BG_DARK)
    style = ttk.Style()
    style.theme_use("clam")

    # Global Frame & Label
    style.configure("TFrame", background=Theme.BG_DARK)
    style.configure("Card.TFrame", background=Theme.CARD_BG, relief="solid", borderwidth=1)
    style.configure("TLabel", background=Theme.BG_DARK, foreground=Theme.TEXT_LIGHT, font=Theme.FONT_BODY)
    style.configure("Card.TLabel", background=Theme.CARD_BG, foreground=Theme.TEXT_LIGHT, font=Theme.FONT_BODY)
    style.configure("Muted.TLabel", foreground=Theme.TEXT_MUTED, font=Theme.FONT_SMALL)
    style.configure("Title.TLabel", foreground=Theme.ACCENT, font=Theme.FONT_TITLE)
    style.configure("Header.TLabel", foreground=Theme.TEXT_LIGHT, font=Theme.FONT_HEADER)

    # Buttons
    style.configure(
        "Primary.TButton",
        background=Theme.ACCENT,
        foreground="#000000",
        font=(Theme.FONT_FAMILY, 9, "bold"),
        borderwidth=0,
        padding=6
    )
    style.map("Primary.TButton", background=[("active", Theme.ACCENT_HOVER)])

    style.configure(
        "Secondary.TButton",
        background=Theme.CARD_BG,
        foreground=Theme.TEXT_LIGHT,
        font=Theme.FONT_BODY,
        borderwidth=1,
        padding=6
    )
    style.map("Secondary.TButton", background=[("active", Theme.CARD_BORDER)])

    style.configure(
        "Warning.TButton",
        background=Theme.WARNING,
        foreground="#000000",
        font=(Theme.FONT_FAMILY, 9, "bold"),
        borderwidth=0,
        padding=6
    )

    style.configure(
        "Danger.TButton",
        background=Theme.DANGER,
        foreground="#ffffff",
        font=(Theme.FONT_FAMILY, 9, "bold"),
        borderwidth=0,
        padding=6
    )

    # Notebook Tabs
    style.configure(
        "TNotebook",
        background=Theme.BG_DARK,
        borderwidth=0
    )
    style.configure(
        "TNotebook.Tab",
        background=Theme.CARD_BG,
        foreground=Theme.TEXT_MUTED,
        padding=[16, 8],
        font=Theme.FONT_BODY
    )
    style.map(
        "TNotebook.Tab",
        background=[("selected", Theme.BG_DARK)],
        foreground=[("selected", Theme.ACCENT)],
        expand=[("selected", [1, 1, 1, 0])]
    )

    # Treeview Table
    style.configure(
        "Treeview",
        background=Theme.CARD_BG,
        foreground=Theme.TEXT_LIGHT,
        fieldbackground=Theme.CARD_BG,
        font=Theme.FONT_BODY,
        rowheight=26,
        borderwidth=0
    )
    style.configure(
        "Treeview.Heading",
        background="#172033",
        foreground=Theme.TEXT_LIGHT,
        font=(Theme.FONT_FAMILY, 9, "bold"),
        padding=6
    )
    style.map(
        "Treeview",
        background=[("selected", Theme.TREE_SELECT)],
        foreground=[("selected", "#ffffff")]
    )

    # Entry and Combobox
    style.configure(
        "TEntry",
        fieldbackground=Theme.CARD_BG,
        foreground=Theme.TEXT_LIGHT,
        insertcolor=Theme.TEXT_LIGHT,
        padding=5
    )
    style.configure(
        "TCombobox",
        fieldbackground=Theme.CARD_BG,
        foreground=Theme.TEXT_LIGHT,
        padding=5
    )

    # Progressbar
    style.configure(
        "Horizontal.TProgressbar",
        background=Theme.ACCENT,
        troughcolor=Theme.CARD_BG,
        borderwidth=0
    )

    return style
