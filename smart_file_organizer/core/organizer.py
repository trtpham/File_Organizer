"""
Rule-based file organization engine supporting multiple strategies.
"""

import re
from pathlib import Path
from typing import List, Optional, Dict, Any
from .models import FileInfo, ActionRecord, ActionType, OrganizeStrategy
from .config import ConfigManager
from .smart_filter import ReceiptClassifier


class Organizer:
    """Computes destination paths for files based on selected categorization strategy."""

    def __init__(self, config: Optional[ConfigManager] = None):
        self.config = config or ConfigManager()

    def plan_organization(
        self,
        files: List[FileInfo],
        destination_dir: Path,
        strategy: OrganizeStrategy = OrganizeStrategy.EXTENSION,
        sub_organize_by_date: bool = False,
        smart_receipts: bool = False
    ) -> List[ActionRecord]:
        """
        Calculates destination folders and generates planned ActionRecords for each file.
        """
        dest_root = Path(destination_dir).resolve()
        records: List[ActionRecord] = []

        for f in files:
            is_financial = False
            msg_prefix = "Organize"

            if smart_receipts and f.extension.lower() in (".pdf", ".txt", ".csv", ".jpg", ".png"):
                receipt = ReceiptClassifier.analyze_file(f)
                if receipt.is_receipt:
                    vendor_name = receipt.vendor or "Invoices"
                    year_val = receipt.year or f.modified_time.year
                    subfolder = f"Finance_{year_val}/{vendor_name}"
                    is_financial = True
                    msg_prefix = f"Smart Sort ({receipt.summary})"

            if not is_financial:
                subfolder = self._determine_folder(f, strategy)

            if sub_organize_by_date and strategy != OrganizeStrategy.DATE and not is_financial:
                date_sub = self._format_date_folder(f)
                target_dir = dest_root / subfolder / date_sub
            else:
                target_dir = dest_root / subfolder

            target_path = target_dir / f.filename

            # If file is already at destination path, skip it
            if f.path.resolve() == target_path.resolve():
                records.append(ActionRecord(
                    action_type=ActionType.SKIP,
                    source_path=f.path,
                    destination_path=target_path,
                    file_size=f.size_bytes,
                    status="SKIPPED",
                    message="File already in correct destination"
                ))
            else:
                records.append(ActionRecord(
                    action_type=ActionType.MOVE,
                    source_path=f.path,
                    destination_path=target_path,
                    file_size=f.size_bytes,
                    status="PENDING",
                    message=f"{msg_prefix} into '{subfolder}'"
                ))

        return records

    def _determine_folder(self, file_info: FileInfo, strategy: OrganizeStrategy) -> str:
        """Determines target folder name based on strategy."""
        if strategy == OrganizeStrategy.DATE:
            return self._format_date_folder(file_info)

        elif strategy == OrganizeStrategy.SIZE:
            return self.config.get_size_folder(file_info.size_bytes)

        elif strategy == OrganizeStrategy.REGEX:
            return self._match_regex_rules(file_info)

        else:  # OrganizeStrategy.EXTENSION (default)
            return self.config.get_category_for_extension(file_info.extension)

    def _format_date_folder(self, file_info: FileInfo) -> str:
        """Formats target directory structure based on file creation or modification date."""
        attr = self.config.date_attribute
        dt = file_info.modified_time if attr == "modified" else file_info.created_time
        fmt = self.config.date_format

        if fmt == "YYYY/MM":
            return f"{dt.year}/{dt.month:02d}"
        elif fmt == "YYYY-MM":
            return f"{dt.year}-{dt.month:02d}"
        elif fmt == "YYYY/MM/DD":
            return f"{dt.year}/{dt.month:02d}/{dt.day:02d}"
        elif fmt == "YYYY-MM-DD":
            return f"{dt.year}-{dt.month:02d}-{dt.day:02d}"
        else:
            return f"{dt.year}/{dt.month:02d}"

    def _match_regex_rules(self, file_info: FileInfo) -> str:
        """Evaluates custom regular expression rules in order, returning matching folder."""
        rules = self.config.data.get("custom_regex_rules", [])
        for rule in rules:
            pattern = rule.get("pattern", "")
            target_folder = rule.get("folder", "")
            if pattern and target_folder:
                try:
                    if re.search(pattern, file_info.filename, re.IGNORECASE):
                        return target_folder
                except re.error:
                    continue

        # Fallback to extension category if no regex matches
        return self.config.get_category_for_extension(file_info.extension)
