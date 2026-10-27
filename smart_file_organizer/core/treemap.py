"""
Interactive Disk Space Treemap and Storage Analyzer.
Computes recursive directory hierarchies and calculates 2D treemap layouts.
"""

import os
from pathlib import Path
from typing import List, Dict, Optional, Tuple, Any


class DiskNode:
    """Represents a node (directory or file) in the disk usage hierarchy."""

    def __init__(self, name: str, path: Path, is_dir: bool = True):
        self.name = name
        self.path = path
        self.is_dir = is_dir
        self.size: int = 0
        self.children: List["DiskNode"] = []
        self.rect: Tuple[float, float, float, float] = (0.0, 0.0, 0.0, 0.0)  # x1, y1, x2, y2
        self.color: str = "#38bdf8"

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "path": str(self.path),
            "is_dir": self.is_dir,
            "size": self.size,
            "children": [c.to_dict() for c in self.children]
        }


class DiskAnalyzer:
    """Calculates storage consumption and partitions 2D treemap layouts."""

    PALETTE = [
        "#38bdf8", "#818cf8", "#a78bfa", "#f472b6", "#fb7185",
        "#f59e0b", "#10b981", "#06b6d4", "#2dd4bf", "#e879f9"
    ]

    @classmethod
    def analyze_directory(cls, root_dir: Path, max_depth: int = 3) -> DiskNode:
        """Builds a recursive size hierarchy tree up to max_depth."""
        root_path = Path(root_dir).resolve()
        root_node = DiskNode(name=root_path.name or str(root_path), path=root_path, is_dir=True)
        cls._populate_node(root_node, current_depth=0, max_depth=max_depth)
        return root_node

    @classmethod
    def _populate_node(cls, node: DiskNode, current_depth: int, max_depth: int) -> int:
        total_size = 0
        try:
            with os.scandir(node.path) as it:
                entries = list(it)

            for entry in entries:
                entry_path = Path(entry.path)
                try:
                    if entry.is_file(follow_symlinks=False):
                        size = entry.stat().st_size
                        total_size += size
                        if current_depth < max_depth:
                            child = DiskNode(name=entry.name, path=entry_path, is_dir=False)
                            child.size = size
                            node.children.append(child)
                    elif entry.is_dir(follow_symlinks=False):
                        child = DiskNode(name=entry.name, path=entry_path, is_dir=True)
                        if current_depth < max_depth:
                            dir_size = cls._populate_node(child, current_depth + 1, max_depth)
                            child.size = dir_size
                            node.children.append(child)
                        else:
                            # Leaf directory beyond max_depth, calculate total size quickly
                            dir_size = cls._quick_dir_size(entry_path)
                            child.size = dir_size
                            node.children.append(child)
                        total_size += dir_size
                except (PermissionError, FileNotFoundError, OSError):
                    continue
        except (PermissionError, FileNotFoundError, OSError):
            pass

        node.size = total_size
        # Sort children largest first
        node.children.sort(key=lambda c: c.size, reverse=True)
        return total_size

    @staticmethod
    def _quick_dir_size(path: Path) -> int:
        total = 0
        for root, _, files in os.walk(path):
            for f in files:
                try:
                    total += (Path(root) / f).stat().st_size
                except OSError:
                    pass
        return total

    @classmethod
    def find_largest_files(cls, root_dir: Path, limit: int = 20) -> List[Dict[str, Any]]:
        """Finds top N largest files in root_dir."""
        files = []
        for root, _, filenames in os.walk(root_dir):
            for f in filenames:
                file_path = Path(root) / f
                try:
                    size = file_path.stat().st_size
                    files.append({"name": f, "path": str(file_path), "size": size})
                except OSError:
                    pass

        files.sort(key=lambda x: x["size"], reverse=True)
        return files[:limit]

    @classmethod
    def layout_treemap(
        cls,
        node: DiskNode,
        x1: float,
        y1: float,
        x2: float,
        y2: float,
        color_idx: int = 0
    ) -> None:
        """
        Calculates slice-and-dice layout for direct children inside bounding box (x1, y1, x2, y2).
        """
        node.rect = (x1, y1, x2, y2)
        if not node.children or node.size <= 0:
            return

        w = x2 - x1
        h = y2 - y1
        if w <= 1 or h <= 1:
            return

        total_children_size = sum(c.size for c in node.children)
        if total_children_size <= 0:
            return

        # Slice along the longer axis
        horizontal = w >= h
        current_offset = x1 if horizontal else y1

        for idx, child in enumerate(node.children):
            ratio = child.size / total_children_size
            child.color = cls.PALETTE[(color_idx + idx) % len(cls.PALETTE)]

            if horizontal:
                child_w = w * ratio
                cx1 = current_offset
                cy1 = y1
                cx2 = current_offset + child_w
                cy2 = y2
                child.rect = (cx1, cy1, cx2, cy2)
                current_offset += child_w
            else:
                child_h = h * ratio
                cx1 = x1
                cy1 = current_offset
                cx2 = x2
                cy2 = current_offset + child_h
                child.rect = (cx1, cy1, cx2, cy2)
                current_offset += child_h
