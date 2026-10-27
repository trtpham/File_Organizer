"""
Metadata-driven Smart Bulk Renaming Engine (EXIF, ID3, Templates, Casing).
"""

import os
import re
from pathlib import Path
from typing import Dict, Any, List, Optional
from datetime import datetime
from .models import FileInfo, ActionRecord, ActionType


class EXIFExtractor:
    """Extracts photo metadata (Date Taken, Camera Model, Dimensions)."""

    @staticmethod
    def extract(path: Path) -> Dict[str, Any]:
        result = {"date": None, "time": None, "camera": None, "width": None, "height": None}
        try:
            from PIL import Image, ExifTags
            with Image.open(path) as img:
                result["width"] = img.width
                result["height"] = img.height
                exif = img.getexif()
                if exif:
                    # Tag mapping
                    exif_data = {ExifTags.TAGS.get(k, k): v for k, v in exif.items()}
                    date_str = exif_data.get("DateTimeOriginal") or exif_data.get("DateTime")
                    if date_str and isinstance(date_str, str):
                        # Format is typically "YYYY:MM:DD HH:MM:SS"
                        parts = date_str.strip().split(" ")
                        if len(parts) >= 1:
                            result["date"] = parts[0].replace(":", "-")
                        if len(parts) >= 2:
                            result["time"] = parts[1].replace(":", "")

                    make = str(exif_data.get("Make", "")).strip()
                    model = str(exif_data.get("Model", "")).strip()
                    if model:
                        result["camera"] = model if not make or make in model else f"{make}_{model}"
        except Exception:
            pass

        # Fallback to file creation/modification time if EXIF is missing
        if not result["date"]:
            stat = path.stat()
            dt = datetime.fromtimestamp(stat.st_mtime)
            result["date"] = dt.strftime("%Y-%m-%d")
            result["time"] = dt.strftime("%H%M%S")

        return result


class ID3Extractor:
    """Lightweight pure-Python ID3v1 and ID3v2 audio tag reader."""

    @staticmethod
    def extract(path: Path) -> Dict[str, Any]:
        result = {"title": None, "artist": None, "album": None, "year": None, "track": None}
        try:
            with open(path, "rb") as f:
                # 1. Try ID3v2 header
                header = f.read(10)
                if len(header) == 10 and header[:3] == b"ID3":
                    size = (
                        (header[6] & 0x7F) << 21 |
                        (header[7] & 0x7F) << 14 |
                        (header[8] & 0x7F) << 7 |
                        (header[9] & 0x7F)
                    )
                    tag_data = f.read(min(size, 200000))
                    result.update(ID3Extractor._parse_id3v2_frames(tag_data))

                # 2. Try ID3v1 fallback (last 128 bytes)
                if not result["title"] or not result["artist"]:
                    f.seek(-128, os.SEEK_END)
                    v1_data = f.read(128)
                    if len(v1_data) == 128 and v1_data[:3] == b"TAG":
                        if not result["title"]:
                            result["title"] = v1_data[3:33].decode("latin-1", errors="ignore").strip("\x00 ").strip()
                        if not result["artist"]:
                            result["artist"] = v1_data[33:63].decode("latin-1", errors="ignore").strip("\x00 ").strip()
                        if not result["album"]:
                            result["album"] = v1_data[63:93].decode("latin-1", errors="ignore").strip("\x00 ").strip()
                        if not result["year"]:
                            result["year"] = v1_data[93:97].decode("latin-1", errors="ignore").strip("\x00 ").strip()
        except Exception:
            pass

        return result

    @staticmethod
    def _parse_id3v2_frames(data: bytes) -> Dict[str, Any]:
        frames = {}
        tag_map = {"TIT2": "title", "TPE1": "artist", "TALB": "album", "TYER": "year", "TRCK": "track"}
        idx = 0
        while idx + 10 < len(data):
            frame_id = data[idx:idx+4].decode("latin-1", errors="ignore")
            if not frame_id.isalnum() or frame_id not in tag_map:
                idx += 1
                continue
            frame_size = int.from_bytes(data[idx+4:idx+8], byteorder="big")
            if frame_size <= 0 or idx + 10 + frame_size > len(data):
                break
            content = data[idx+10:idx+10+frame_size]
            encoding = content[0] if len(content) > 0 else 0
            raw_text = content[1:]
            try:
                if encoding in (1, 2):
                    text = raw_text.decode("utf-16", errors="ignore")
                elif encoding == 3:
                    text = raw_text.decode("utf-8", errors="ignore")
                else:
                    text = raw_text.decode("latin-1", errors="ignore")
                clean_text = text.replace("\x00", "").strip()
                if clean_text:
                    frames[tag_map[frame_id]] = clean_text
            except Exception:
                pass
            idx += 10 + frame_size
        return frames


class SmartBulkRenamer:
    """Batch renamer with metadata templates, casing, find/replace, and numbering."""

    IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".tiff", ".webp", ".raw", ".cr2", ".nef", ".arw", ".heic"}
    AUDIO_EXTS = {".mp3", ".flac", ".wav", ".aac", ".m4a", ".ogg"}

    def plan_rename(
        self,
        files: List[FileInfo],
        template: str = "{name}",
        find_text: str = "",
        replace_text: str = "",
        case_transform: Optional[str] = None,
        prefix: str = "",
        suffix: str = "",
        start_counter: int = 1,
        counter_padding: int = 3,
        use_regex: bool = False
    ) -> List[ActionRecord]:
        """
        Generates ActionRecords for planned renaming operations.
        Supported casing: 'lowercase', 'uppercase', 'title_case', 'snake_case', 'kebab_case'
        """
        records: List[ActionRecord] = []
        counter = start_counter

        for file_info in files:
            path = file_info.path
            ext = file_info.extension.lstrip(".")
            stem = path.stem

            # Extract contextual metadata
            meta: Dict[str, Any] = {
                "name": stem,
                "ext": ext,
                "counter": f"{counter:0{counter_padding}d}",
                "size": str(file_info.size_bytes),
                "date": file_info.modified_time.strftime("%Y-%m-%d"),
                "time": file_info.modified_time.strftime("%H%M%S"),
                "camera": "",
                "artist": "",
                "title": "",
                "album": ""
            }

            if file_info.extension.lower() in self.IMAGE_EXTS:
                exif = EXIFExtractor.extract(path)
                if exif.get("date"):
                    meta["date"] = exif["date"]
                if exif.get("time"):
                    meta["time"] = exif["time"]
                if exif.get("camera"):
                    meta["camera"] = exif["camera"]

            elif file_info.extension.lower() in self.AUDIO_EXTS:
                id3 = ID3Extractor.extract(path)
                meta["artist"] = id3.get("artist") or ""
                meta["title"] = id3.get("title") or ""
                meta["album"] = id3.get("album") or ""

            # Apply Template
            new_name = template
            for key, val in meta.items():
                pattern_key = "{" + key + "}"
                new_name = new_name.replace(pattern_key, str(val))

            # Find & Replace
            if find_text:
                if use_regex:
                    try:
                        new_name = re.sub(find_text, replace_text, new_name)
                    except re.error:
                        pass
                else:
                    new_name = new_name.replace(find_text, replace_text)

            # Apply Casing Transformation
            if case_transform == "lowercase":
                new_name = new_name.lower()
            elif case_transform == "uppercase":
                new_name = new_name.upper()
            elif case_transform == "title_case":
                new_name = new_name.title()
            elif case_transform == "snake_case":
                new_name = re.sub(r"[\s\-]+", "_", new_name).lower()
            elif case_transform == "kebab_case":
                new_name = re.sub(r"[\s_]+", "-", new_name).lower()

            # Prefix & Suffix
            new_stem = f"{prefix}{new_name}{suffix}".strip()
            # Clean invalid filesystem characters
            new_stem = re.sub(r'[\\/*?:"<>|]', "", new_stem)
            target_filename = f"{new_stem}.{ext}" if ext else new_stem
            target_path = path.parent / target_filename

            if target_path.name == path.name:
                records.append(ActionRecord(
                    action_type=ActionType.SKIP,
                    source_path=path,
                    destination_path=target_path,
                    file_size=file_info.size_bytes,
                    status="SKIPPED",
                    message="Name unchanged"
                ))
            else:
                records.append(ActionRecord(
                    action_type=ActionType.RENAME,
                    source_path=path,
                    destination_path=target_path,
                    file_size=file_info.size_bytes,
                    status="PENDING",
                    message=f"Rename: {path.name} -> {target_filename}"
                ))

            counter += 1

        return records
