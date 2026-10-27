"""
Unit tests for DiskAnalyzer and Treemap layout algorithm.
"""

from pathlib import Path
from smart_file_organizer.core.treemap import DiskAnalyzer


def test_disk_tree_analysis(temp_workspace):
    sub = temp_workspace / "subfolder"
    sub.mkdir()
    (sub / "file_a.dat").write_bytes(b"A" * 1000)
    (sub / "file_b.dat").write_bytes(b"B" * 2000)
    (temp_workspace / "root_file.txt").write_bytes(b"C" * 500)

    root_node = DiskAnalyzer.analyze_directory(temp_workspace, max_depth=2)

    assert root_node.size >= 3500
    assert len(root_node.children) >= 2
    # Check largest child is subfolder
    sub_node = next(c for c in root_node.children if c.name == "subfolder")
    assert sub_node.size >= 3000


def test_treemap_layout_coordinates(temp_workspace):
    (temp_workspace / "f1.bin").write_bytes(b"X" * 600)
    (temp_workspace / "f2.bin").write_bytes(b"Y" * 400)

    root_node = DiskAnalyzer.analyze_directory(temp_workspace, max_depth=1)
    DiskAnalyzer.layout_treemap(root_node, x1=0, y1=0, x2=1000, y2=500)

    assert root_node.rect == (0, 0, 1000, 500)
    assert len(root_node.children) == 2

    # Check that children partition the 1000-width box
    child1 = root_node.children[0]
    child2 = root_node.children[1]

    assert child1.rect[0] == 0
    assert child2.rect[2] == 1000


def test_find_largest_files(temp_workspace):
    (temp_workspace / "small.txt").write_bytes(b"1" * 10)
    (temp_workspace / "big.bin").write_bytes(b"2" * 50000)

    largest = DiskAnalyzer.find_largest_files(temp_workspace, limit=5)
    assert len(largest) >= 2
    assert largest[0]["name"] == "big.bin"
    assert largest[0]["size"] == 50000
