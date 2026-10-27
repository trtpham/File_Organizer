"""
Perceptual Hashing (dHash) and Visual Image Similarity Engine.
"""

from pathlib import Path
from typing import Dict, List, Optional, Tuple
from .models import FileInfo, VisualDuplicateGroup


class PerceptualHasher:
    """Computes 64-bit difference hash (dHash) to identify visually similar images."""

    @staticmethod
    def compute_dhash(image_path: Path) -> Optional[int]:
        """
        Calculates 64-bit difference hash for an image:
        1. Downsample to 9x8 grayscale
        2. Compare adjacent horizontal pixel brightness
        3. Returns 64-bit unsigned integer
        """
        try:
            from PIL import Image
            with Image.open(image_path) as img:
                # Convert to grayscale and resize to 9x8 using Box/Bilinear
                gray = img.convert("L").resize((9, 8), Image.Resampling.BOX)
                pixels = list(gray.tobytes())

                difference = 0
                for row in range(8):
                    for col in range(8):
                        left = pixels[row * 9 + col]
                        right = pixels[row * 9 + col + 1]
                        if left > right:
                            difference |= 1 << (row * 8 + col)
                return difference
        except Exception:
            return None

    @staticmethod
    def hamming_distance(hash1: int, hash2: int) -> int:
        """Counts differing bits between two 64-bit hashes."""
        return bin(hash1 ^ hash2).count("1")

    @staticmethod
    def similarity(hash1: int, hash2: int) -> float:
        """Returns similarity percentage between 0.0 (completely different) and 1.0 (identical)."""
        dist = bin(hash1 ^ hash2).count("1")
        return max(0.0, 1.0 - (dist / 64.0))


class VisualSimilarityEngine:
    """Finds near-duplicate and visually identical photos across directories."""

    IMAGE_EXTENSIONS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tiff", ".tif"}

    def __init__(self, similarity_threshold: float = 0.90):
        # Default 90% similarity corresponds to Hamming distance <= 6
        self.similarity_threshold = similarity_threshold

    def find_visual_duplicates(self, files: List[FileInfo]) -> List[VisualDuplicateGroup]:
        """Scans image files and clusters them into VisualDuplicateGroups."""
        image_files = [f for f in files if f.extension.lower() in self.IMAGE_EXTENSIONS]
        if len(image_files) < 2:
            return []

        # 1. Compute perceptual hashes for all images
        hashes: Dict[Path, int] = {}
        for f in image_files:
            h = PerceptualHasher.compute_dhash(f.path)
            if h is not None:
                hashes[f.path] = h

        # 2. Compare pairs and build connected clusters
        clusters: List[List[FileInfo]] = []
        visited: set = set()

        for i, f1 in enumerate(image_files):
            if f1.path not in hashes or f1.path in visited:
                continue

            current_cluster = [f1]
            visited.add(f1.path)
            h1 = hashes[f1.path]

            for j in range(i + 1, len(image_files)):
                f2 = image_files[j]
                if f2.path not in hashes or f2.path in visited:
                    continue

                h2 = hashes[f2.path]
                sim = PerceptualHasher.similarity(h1, h2)
                if sim >= self.similarity_threshold:
                    current_cluster.append(f2)
                    visited.add(f2.path)

            if len(current_cluster) > 1:
                clusters.append(current_cluster)

        # 3. Build VisualDuplicateGroup with suggested keep (prefer higher resolution or larger size)
        results: List[VisualDuplicateGroup] = []
        for cluster in clusters:
            best_file = max(cluster, key=lambda f: (f.size_bytes, f.path.name))
            primary_hash = hex(hashes[best_file.path])[2:].zfill(16)

            # Calculate average similarity in group
            avg_sim = 1.0
            if len(cluster) > 1:
                sims = [
                    PerceptualHasher.similarity(hashes[best_file.path], hashes[f.path])
                    for f in cluster if f != best_file
                ]
                avg_sim = sum(sims) / len(sims) if sims else 1.0

            results.append(VisualDuplicateGroup(
                phash=primary_hash,
                files=cluster,
                similarity_score=avg_sim,
                suggested_keep=best_file
            ))

        return results
