"""
CLI Controller coordinating core engine and output formatting.
"""

import sys
from pathlib import Path
from typing import Optional, List

from .parser import create_parser
from .formatter import (
    print_banner, print_info, print_success, print_warning,
    print_error, print_table, print_progress, Color
)
from ..core.config import ConfigManager
from ..core.scanner import FileScanner
from ..core.organizer import Organizer
from ..core.deduplicator import Deduplicator
from ..core.executor import TransactionExecutor
from ..core.undo_manager import UndoManager
from ..core.watcher import DirectoryWatcher
from ..core.reporter import Reporter
from ..core.renamer import SmartBulkRenamer
from ..core.syncer import DriveDetector, FolderSynchronizer
from ..core.shell_integration import WindowsShellIntegration
from ..core.treemap import DiskAnalyzer
from ..core.junk_cleaner import DevJunkCleaner
from ..core.secret_scanner import SecretScanner
from ..core.models import OrganizeStrategy, ActionType, SyncProfile, SyncMode


def run_cli(args: Optional[List[str]] = None) -> int:
    """Main CLI execution router."""
    parser = create_parser()
    parsed_args = parser.parse_args(args)

    # 1. Check for GUI launch
    if parsed_args.gui:
        try:
            from ..gui.app import launch_gui
            launch_gui()
            return 0
        except ImportError as e:
            print_error(f"Cannot launch GUI: {e}")
            return 1

    print_banner()

    # Load configuration
    config = ConfigManager(custom_path=Path(parsed_args.config) if parsed_args.config else None)
    undo_mgr = UndoManager()

    # 2. Check for Session History listing
    if parsed_args.history:
        sessions = undo_mgr.list_sessions(limit=15)
        if not sessions:
            print_info("No recorded sessions found.")
            return 0
        headers = ["Session ID", "Date & Time", "Strategy", "Mode", "Moved", "Dups", "Status"]
        rows = [
            [
                s["session_id"],
                s["start_time"][:19].replace("T", " "),
                s["strategy"],
                "Dry-Run" if s["dry_run"] else "Live",
                s["moved_count"],
                s["duplicate_count"],
                s["status"]
            ]
            for s in sessions
        ]
        print_info("Recent Organization & Cleanup Sessions:")
        print_table(headers, rows)
        return 0

    # 2b. Check for USB drive detection
    if parsed_args.detect_usb:
        drives = DriveDetector.get_removable_drives()
        if not drives:
            print_info("No removable USB drives currently detected.")
        else:
            print_success(f"Detected {len(drives)} removable drive(s):")
            for d in drives:
                print(f"  - Path: {d['path']} | Label: {d['label']}")
        return 0

    # 2c. Check for Folder / USB Synchronization
    if parsed_args.sync_source and parsed_args.sync_dest:
        src = Path(parsed_args.sync_source)
        dst = Path(parsed_args.sync_dest)
        mode = SyncMode(parsed_args.sync_mode)
        print_info(f"Starting {mode.value.upper()} sync: {src} -> {dst}")
        profile = SyncProfile(name="CLI Sync", source_dir=src, dest_dir=dst, mode=mode)
        res = FolderSynchronizer.sync(profile, dry_run=parsed_args.dry_run)
        if res["success"]:
            print_success(res["message"])
        else:
            print_error(res.get("error", "Sync failed."))
        return 0 if res["success"] else 1

    # 2d. Check for Windows Context Menu actions
    if parsed_args.install_context_menu:
        res = WindowsShellIntegration.install()
        if res["success"]:
            print_success(res["message"])
        else:
            print_error(res["message"])
        return 0 if res["success"] else 1

    if parsed_args.uninstall_context_menu:
        res = WindowsShellIntegration.uninstall()
        if res["success"]:
            print_success(res["message"])
        else:
            print_error(res["message"])
        return 0 if res["success"] else 1

    # 2e. Check for Disk Storage Analyzer
    if parsed_args.analyze_disk:
        target = Path(parsed_args.source or ".").resolve()
        print_info(f"Analyzing disk usage for: {target}...")
        root_node = DiskAnalyzer.analyze_directory(target, max_depth=2)
        print_success(f"Total directory size: {Reporter.format_bytes(root_node.size)}")
        print_info("\nSubdirectories and files by size:")
        for child in root_node.children[:12]:
            t = "[DIR] " if child.is_dir else "[FILE]"
            pct = (child.size / root_node.size * 100) if root_node.size > 0 else 0
            print(f"  {t} {child.name:<30} {Reporter.format_bytes(child.size):>10} ({pct:4.1f}%)")

        print_info("\nTop 10 Largest Individual Files:")
        largest = DiskAnalyzer.find_largest_files(target, limit=10)
        for item in largest:
            print(f"  - {item['name']:<35} {Reporter.format_bytes(item['size']):>10}")
        return 0

    # 2f. Check for Developer Junk Sweeper
    if parsed_args.scan_junk or parsed_args.clean_junk:
        target = Path(parsed_args.source or ".").resolve()
        print_info(f"Scanning for disposable developer bloat in: {target}...")
        junk_items = DevJunkCleaner.scan_directory(target, inactive_days=14)
        if not junk_items:
            print_success("No stale developer junk discovered!")
            return 0

        total_junk_size = sum(item.size_bytes for item in junk_items)
        print_warning(f"Found {len(junk_items)} junk directories totaling {Reporter.format_bytes(total_junk_size)}:")
        for item in junk_items:
            print(f"  [{item.category}] {item.path.name:<25} ({Reporter.format_bytes(item.size_bytes)})")

        if parsed_args.clean_junk:
            print_info("Proceeding to clean junk...")
            clean_res = DevJunkCleaner.clean_junk(junk_items, dry_run=parsed_args.dry_run)
            print_success(clean_res["message"])
        else:
            print_info("To clean these items, re-run with: --clean-junk")
        return 0

    # 2g. Check for Sensitive Data & Secret Scanner
    if parsed_args.scan_secrets or parsed_args.quarantine_secrets:
        target = Path(parsed_args.source or ".").resolve()
        print_info(f"Scanning files for exposed secrets & API credentials in: {target}...")
        findings = SecretScanner.scan_directory(target, recursive=parsed_args.recursive)
        if not findings:
            print_success("Security scan clean! No exposed secrets detected.")
            return 0

        print_warning(f"SECURITY ALERT: Found {len(findings)} potential secret leaks:")
        for f in findings:
            print(f"  [{f.severity}] {f.secret_type:<25} in {f.file_path.name}:{f.line_number} -> {f.masked_snippet}")

        if parsed_args.quarantine_secrets:
            print_info("Quarantining compromised files...")
            q_res = SecretScanner.quarantine_files(findings, target)
            print_success(q_res["message"])
        else:
            print_info("To isolate these files into _Quarantine/, re-run with: --quarantine-secrets")
        return 0

    # 3. Check for Undo command
    if parsed_args.undo:
        target_id = None if parsed_args.undo == "latest" else parsed_args.undo
        print_info(f"Initiating rollback for session: {target_id or 'latest'}...")
        result = undo_mgr.rollback_session(target_id)
        if result["success"]:
            print_success(f"Rollback completed: {result['message']}")
            print_info(f"Restored files: {result['restored_count']}")
            if result.get("failed_count", 0) > 0:
                print_warning(f"Failed to restore {result['failed_count']} files.")
                for err in result.get("errors", []):
                    print_warning(f"  - {err}")
            return 0
        else:
            print_error(f"Rollback failed: {result['message']}")
            return 1

    # 4. Resolve source and destination directories
    source_str = parsed_args.source or "."
    source_dir = Path(source_str).resolve()
    if not source_dir.exists() or not source_dir.is_dir():
        print_error(f"Source directory does not exist or is invalid: {source_dir}")
        return 1

    dest_str = parsed_args.dest or source_str
    dest_dir = Path(dest_str).resolve()

    # Determine strategy
    if parsed_args.by_date:
        strategy = OrganizeStrategy.DATE
    elif parsed_args.by_size:
        strategy = OrganizeStrategy.SIZE
    else:
        try:
            strategy = OrganizeStrategy(parsed_args.strategy)
        except ValueError:
            strategy = OrganizeStrategy.EXTENSION

    # 5. Check for Watcher mode
    if parsed_args.watch:
        print_info(f"Starting Background Directory Watcher on: {source_dir}")
        print_info(f"Destination: {dest_dir} | Strategy: {strategy.value.upper()}")
        print_info("Files will be organized automatically after download completion.")
        print_warning("Press Ctrl+C to stop the watcher daemon.")

        def on_organized(filename: str, target: str):
            print_success(f"[Watcher] Auto-organized: {filename} -> {target}")

        watcher = DirectoryWatcher(
            source_dir=source_dir,
            destination_dir=dest_dir,
            strategy=strategy,
            config=config,
            undo_manager=undo_mgr
        )
        try:
            watcher.start(blocking=True, on_organized=on_organized)
        except KeyboardInterrupt:
            watcher.stop()
            print_info("\nWatcher stopped cleanly.")
        return 0

    # 6. Normal Scanning & Processing
    print_info(f"Scanning directory: {source_dir} (Recursive: {parsed_args.recursive})...")
    scanner = FileScanner(config)
    exclude = [dest_dir] if dest_dir != source_dir else []
    files = scanner.scan(source_dir, recursive=parsed_args.recursive, exclude_dirs=exclude)

    if not files:
        print_warning("No files found to organize or deduplicate.")
        return 0

    print_info(f"Discovered {len(files)} files ({Reporter.format_bytes(sum(f.size_bytes for f in files))}).")

    executor = TransactionExecutor(config)
    all_records = []

    # A1. Smart Bulk Renaming Flow if requested
    if parsed_args.rename_template:
        print_info(f"Planning bulk rename with template: '{parsed_args.rename_template}'...")
        renamer = SmartBulkRenamer()
        rename_records = renamer.plan_rename(
            files,
            template=parsed_args.rename_template,
            find_text=parsed_args.rename_find,
            replace_text=parsed_args.rename_replace,
            case_transform=parsed_args.rename_case
        )
        all_records.extend(rename_records)

    # A2. Visual Duplicate Identification (pHash)
    elif parsed_args.visual_dedupe:
        print_info(f"Running Perceptual Visual Duplicate Detection (Threshold: {parsed_args.similarity * 100:.0f}%)...")
        deduplicator = Deduplicator(config)
        v_groups = deduplicator.find_visual_duplicates(files, similarity_threshold=parsed_args.similarity)
        if not v_groups:
            print_success("No visual duplicate images found!")
        else:
            print_warning(f"Found {len(v_groups)} visual duplicate image clusters!")
            if parsed_args.dedupe_action == "report":
                for g in v_groups:
                    print(f"  Visual Cluster (pHash: {g.phash[:8]}..., Avg Sim: {g.similarity_score*100:.0f}%):")
                    for df in g.files:
                        is_keep = " [ORIGINAL / BEST RES]" if df == g.suggested_keep else " [VISUAL DUPLICATE]"
                        print(f"    - {df.path.name}{is_keep}")
            else:
                v_records = deduplicator.plan_visual_deduplication_actions(
                    v_groups, dest_dir, action_mode=parsed_args.dedupe_action
                )
                all_records.extend(v_records)

    # A3. Deduplication Flow if requested
    elif parsed_args.dedupe:
        print_info("Running 3-tiered duplicate identification...")
        deduplicator = Deduplicator(config)

        def dedupe_prog(stage: str, count: int, total: int):
            if stage == "tier1_complete":
                print_info(f"  Tier 1 (Size Clustering)      : {count} size clusters")
            elif stage == "tier2_complete":
                print_info(f"  Tier 2 (64KB Partial Hashes)  : {count} candidate clusters")
            elif stage == "tier3_complete":
                print_info(f"  Tier 3 (Full SHA-256 Hashes)  : {count} confirmed duplicate groups")

        dup_groups = deduplicator.find_duplicates(files, progress_callback=dedupe_prog)
        total_wasted = sum(g.wasted_space_bytes for g in dup_groups)

        if not dup_groups:
            print_success("No duplicate files found!")
        else:
            print_warning(f"Found {len(dup_groups)} duplicate clusters! Wasted disk space: {Reporter.format_bytes(total_wasted)}")
            if parsed_args.dedupe_action == "report":
                for g in dup_groups:
                    print(f"  Hash: {g.file_hash[:12]}... ({Reporter.format_bytes(g.file_size)})")
                    for df in g.files:
                        is_keep = " [ORIGINAL]" if df == g.suggested_keep else " [DUPLICATE]"
                        print(f"    - {df.path.name}{is_keep}")
            else:
                dup_records = deduplicator.plan_deduplication_actions(
                    dup_groups,
                    dest_dir,
                    action_mode=parsed_args.dedupe_action
                )
                all_records.extend(dup_records)

    # B. Organization Flow (if not rename-only or report-only)
    if not parsed_args.rename_template and not (parsed_args.dedupe and parsed_args.dedupe_action == "report"):
        organizer = Organizer(config)
        # Filter out files that were already planned for duplicate deletion/move
        handled_paths = {r.source_path for r in all_records}
        remaining_files = [f for f in files if f.path not in handled_paths]

        org_records = organizer.plan_organization(
            remaining_files,
            dest_dir,
            strategy=strategy,
            smart_receipts=parsed_args.smart_receipts
        )
        all_records.extend(org_records)

    if not all_records:
        print_info("No actions required.")
        return 0

    # Display preview table (top 15 items)
    sample_records = all_records[:15]
    headers = ["Action", "Source File", "Destination Subfolder"]
    rows = []
    for r in sample_records:
        sub = r.destination_path.parent.name if r.destination_path else "-"
        rows.append([r.action_type.value, r.source_path.name[:35], sub])
    print_table(headers, rows)
    if len(all_records) > 15:
        print_info(f"... and {len(all_records) - 15} more planned actions.")

    # Execute session (Dry-run or Live)
    mode_str = "DRY-RUN SIMULATION" if parsed_args.dry_run else "LIVE EXECUTION"
    print_info(f"Starting {mode_str} for {len(all_records)} operations...")

    session = executor.execute_session(
        records=all_records,
        source_dir=source_dir,
        destination_dir=dest_dir,
        strategy=strategy.value,
        dry_run=parsed_args.dry_run
    )

    # Save session journal
    undo_mgr.save_session(session)

    # Print summary
    print(Reporter.generate_console_summary(session))

    # Generate HTML report if requested
    if parsed_args.report:
        report_path = Reporter.generate_html(session, Path(parsed_args.report))
        print_success(f"HTML dashboard generated: {report_path.resolve()}")

    if not parsed_args.dry_run and session.moved_count > 0:
        print_info(f"To undo this session at any time, run: python main.py --undo {session.session_id}")

    return 0
