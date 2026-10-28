"""
Perceptual Hashing (dHash) and Visual Image Similarity Engine.
Enhanced with EXIF orientation auto-transpose, multi-threaded parallel hashing,
and fast similarity clustering.
"""

import os
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from typing import Dict, List, Optional, Tuple, Set
from .models import FileInfo, VisualDuplicateGroup


class PerceptualHasher:
    """Computes difference hash (dHash) and average hash (aHash) for visual similarity."""

    @staticmethod
    def compute_dhash(image_path: Path) -> Optional[int]:
        """
        Calculates 64-bit difference hash for an image:
        1. Auto-transpose EXIF orientation (portrait vs landscape)
        2. Downsample to 9x8 grayscale
        3. Compare adjacent horizontal pixel brightness
        4. Returns 64-bit unsigned integer
        """
        try:
            from PIL import Image, ImageOps
            with Image.open(image_path) as img:
                # Auto-transpose based on EXIF tag to normalize orientation
                try:
                    img = ImageOps.exif_transpose(img) or img
                except Exception:
                    pass

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
    def compute_ahash(image_path: Path) -> Optional[int]:
        """Computes 64-bit average hash (aHash) for brightness distribution."""
        try:
            from PIL import Image, ImageOps
            with Image.open(image_path) as img:
                try:
                    img = ImageOps.exif_transpose(img) or img
                except Exception:
                    pass
                gray = img.convert("L").resize((8, 8), Image.Resampling.BOX)
                pixels = list(gray.tobytes())
                avg = sum(pixels) / len(pixels)
                hash_val = 0
                for i, p in enumerate(pixels):
                    if p >= avg:
                        hash_val |= 1 << i
                return hash_val
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

        # 1. Compute perceptual hashes concurrently
        hashes: Dict[Path, int] = {}
        workers = min(16, (os.cpu_count() or 4) * 2)

        def _hash_worker(f: FileInfo) -> Tuple[Path, Optional[int]]:
            return f.path, PerceptualHasher.compute_dhash(f.path)

        with ThreadPoolExecutor(max_workers=workers) as executor:
            for path, h in executor.map(_hash_worker, image_files):
                if h is not None:
                    hashes[path] = h

        # 2. Build similarity clusters
        clusters: List[List[FileInfo]] = []
        visited: Set[Path] = set()

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
            # Try to pick best resolution if Pillow is installed
            try:
                from PIL import Image
                max_pixels = -1
                for f in cluster:
                    try:
                        with Image.open(f.path) as img:
                            pixels = img.width * img.height
                            if pixels > max_pixels:
                                max_pixels = pixels
                                best_file = f
                    except Exception:
                        pass
            except Exception:
                pass

            primary_hash = hex(hashes[best_file.path])[2:].zfill(16)

            # Calculate average similarity in group
            avg_sim = 1.0
            if len(cluster) > 1:
                sims = [
                    PerceptualHasher.similarity(hashes[best_file.path], hashes[f.path])
                    for f in cluster if f != best_file and f.path in hashes
                ]
                avg_sim = sum(sims) / len(sims) if sims else 1.0

            results.append(VisualDuplicateGroup(
                phash=primary_hash,
                files=cluster,
                similarity_score=avg_sim,
                suggested_keep=best_file
            ))

        return results
