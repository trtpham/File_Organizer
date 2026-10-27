"""
Unit tests for PerceptualHasher and VisualSimilarityEngine.
"""

from pathlib import Path
from PIL import Image
from smart_file_organizer.core.models import FileInfo
from smart_file_organizer.core.visual_hasher import PerceptualHasher, VisualSimilarityEngine


def test_hamming_distance_and_similarity():
    # Exactly identical
    h1 = 0x123456789ABCDEF0
    assert PerceptualHasher.hamming_distance(h1, h1) == 0
    assert PerceptualHasher.similarity(h1, h1) == 1.0

    # 1 bit difference
    h2 = h1 ^ 1
    assert PerceptualHasher.hamming_distance(h1, h2) == 1
    assert PerceptualHasher.similarity(h1, h2) == 63 / 64.0


def test_visual_similarity_clustering(temp_workspace):
    # Create an original image with a horizontal gradient (high to low)
    img1_path = temp_workspace / "original.jpg"
    img1 = Image.new("L", (100, 100))
    for x in range(100):
        for y in range(100):
            img1.putpixel((x, y), int(255 - x * 2.5))
    img1.save(img1_path)

    # Create a resized copy of the same gradient (visual duplicate)
    img2_path = temp_workspace / "resized_copy.png"
    img_resized = img1.resize((60, 60))
    img_resized.save(img2_path)

    # Create a completely different image with vertical gradient
    img3_path = temp_workspace / "different.jpg"
    img3 = Image.new("L", (100, 100))
    for x in range(100):
        for y in range(100):
            img3.putpixel((x, y), int(255 - y * 2.5))
    img3.save(img3_path)

    files = [
        FileInfo.from_path(img1_path),
        FileInfo.from_path(img2_path),
        FileInfo.from_path(img3_path)
    ]

    engine = VisualSimilarityEngine(similarity_threshold=0.90)
    groups = engine.find_visual_duplicates(files)

    # Should find exactly 1 group connecting original and resized_copy
    assert len(groups) == 1
    assert len(groups[0].files) == 2
    filenames = {f.filename for f in groups[0].files}
    assert "original.jpg" in filenames
    assert "resized_copy.png" in filenames
    assert "different.jpg" not in filenames
