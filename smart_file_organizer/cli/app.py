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
from ..core.models import OrganizeStrategy, ActionType


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

    # A. Deduplication Flow if requested
    if parsed_args.dedupe:
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

    # B. Organization Flow (if not dedupe-only or along with dedupe)
    if not (parsed_args.dedupe and parsed_args.dedupe_action == "report"):
        organizer = Organizer(config)
        # Filter out files that were already planned for duplicate deletion/move
        handled_paths = {r.source_path for r in all_records}
        remaining_files = [f for f in files if f.path not in handled_paths]

        org_records = organizer.plan_organization(remaining_files, dest_dir, strategy=strategy)
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
