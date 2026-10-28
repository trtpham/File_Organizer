"""
Journal recording and transactional undo / rollback engine.
Enhanced with WAL SQLite mode, session pruning, and comprehensive rollback (Move, Deduplicate, Rename).
"""

import json
import sqlite3
import shutil
from pathlib import Path
from typing import List, Optional, Dict, Any
from datetime import datetime, timedelta
from .models import SessionLog, ActionRecord, ActionType


class UndoManager:
    """Records session history and provides 100% reversible file rollback."""

    DEFAULT_DATA_DIR = Path.home() / ".smart_file_organizer"
    DB_PATH = DEFAULT_DATA_DIR / "history.db"

    def __init__(self, data_dir: Optional[Path] = None):
        self.data_dir = Path(data_dir) if data_dir else self.DEFAULT_DATA_DIR
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.db_path = self.data_dir / "history.db"
        self._init_database()

    def _init_database(self) -> None:
        """Initializes SQLite schema with WAL journal mode for performance and durability."""
        with sqlite3.connect(self.db_path) as conn:
            conn.execute("PRAGMA journal_mode=WAL;")
            conn.execute("""
                CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    start_time TEXT,
                    end_time TEXT,
                    source_dir TEXT,
                    destination_dir TEXT,
                    strategy TEXT,
                    dry_run INTEGER,
                    total_files INTEGER,
                    moved_count INTEGER,
                    skipped_count INTEGER,
                    duplicate_count INTEGER,
                    space_saved_bytes INTEGER,
                    status TEXT DEFAULT 'COMPLETED',
                    data_json TEXT
                )
            """)
            conn.commit()

    def save_session(self, session: SessionLog) -> None:
        """Persists session log to both SQLite database and local JSON journal."""
        # 1. Save to SQLite
        with sqlite3.connect(self.db_path) as conn:
            conn.execute("""
                INSERT OR REPLACE INTO sessions (
                    session_id, start_time, end_time, source_dir, destination_dir,
                    strategy, dry_run, total_files, moved_count, skipped_count,
                    duplicate_count, space_saved_bytes, status, data_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                session.session_id,
                session.start_time.isoformat(),
                session.end_time.isoformat() if session.end_time else "",
                session.source_dir,
                session.destination_dir,
                session.strategy,
                1 if session.dry_run else 0,
                session.total_files,
                session.moved_count,
                session.skipped_count,
                session.duplicate_count,
                session.space_saved_bytes,
                "DRY_RUN" if session.dry_run else "COMPLETED",
                json.dumps(session.to_dict(), ensure_ascii=False)
            ))
            conn.commit()

        # 2. Also save to JSON file in data_dir/journals
        journals_dir = self.data_dir / "journals"
        journals_dir.mkdir(parents=True, exist_ok=True)
        journal_file = journals_dir / f"{session.session_id}.json"
        with open(journal_file, "w", encoding="utf-8") as f:
            json.dump(session.to_dict(), f, indent=2, ensure_ascii=False)

    def list_sessions(self, limit: int = 20) -> List[Dict[str, Any]]:
        """Returns summarized list of recorded sessions, newest first."""
        with sqlite3.connect(self.db_path) as conn:
            cursor = conn.execute("""
                SELECT session_id, start_time, source_dir, destination_dir,
                       strategy, dry_run, moved_count, duplicate_count, status
                FROM sessions
                ORDER BY start_time DESC
                LIMIT ?
            """, (limit,))
            rows = cursor.fetchall()

        results = []
        for r in rows:
            results.append({
                "session_id": r[0],
                "start_time": r[1],
                "source_dir": r[2],
                "destination_dir": r[3],
                "strategy": r[4],
                "dry_run": bool(r[5]),
                "moved_count": r[6],
                "duplicate_count": r[7],
                "status": r[8],
            })
        return results

    def get_session(self, session_id: str) -> Optional[SessionLog]:
        """Loads complete SessionLog by session_id."""
        with sqlite3.connect(self.db_path) as conn:
            cursor = conn.execute("SELECT data_json FROM sessions WHERE session_id = ?", (session_id,))
            row = cursor.fetchone()
            if row and row[0]:
                return SessionLog.from_dict(json.loads(row[0]))

        # Fallback to JSON journal file
        journal_file = self.data_dir / "journals" / f"{session_id}.json"
        if journal_file.exists():
            with open(journal_file, "r", encoding="utf-8") as f:
                return SessionLog.from_dict(json.load(f))
        return None

    def get_latest_session(self) -> Optional[SessionLog]:
        """Retrieves most recent non-dry-run completed session."""
        with sqlite3.connect(self.db_path) as conn:
            cursor = conn.execute("""
                SELECT session_id FROM sessions
                WHERE dry_run = 0 AND status = 'COMPLETED'
                ORDER BY start_time DESC
                LIMIT 1
            """)
            row = cursor.fetchone()
            if row:
                return self.get_session(row[0])
        return None

    def rollback_session(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        """
        Reverses all successful operations from the specified or latest session.
        Restores files to their exact original source paths (supports MOVE, DEDUPLICATE, and RENAME).
        """
        if not session_id:
            session = self.get_latest_session()
            if not session:
                return {"success": False, "message": "No undoable completed session found."}
        else:
            session = self.get_session(session_id)
            if not session:
                return {"success": False, "message": f"Session '{session_id}' not found."}

        if session.dry_run:
            return {"success": False, "message": "Cannot undo a Dry-Run simulation session."}

        restored_count = 0
        failed_count = 0
        errors: List[str] = []
        created_dirs_to_check: List[Path] = []

        # Iterate in REVERSE order
        for record in reversed(session.records):
            if record.status != "SUCCESS":
                continue

            # Support rollback for MOVE, DEDUPLICATE, and RENAME
            if record.action_type in (ActionType.MOVE, ActionType.DEDUPLICATE, ActionType.RENAME) and record.destination_path:
                curr_path = record.destination_path
                orig_path = record.source_path

                if not curr_path.exists():
                    failed_count += 1
                    errors.append(f"File missing at organized path: {curr_path}")
                    continue

                try:
                    orig_path.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(curr_path), str(orig_path))
                    restored_count += 1
                    created_dirs_to_check.append(curr_path.parent)
                except OSError as e:
                    failed_count += 1
                    errors.append(f"Failed to restore {curr_path.name}: {str(e)}")

        # Clean empty folders created during organization
        for parent_dir in set(created_dirs_to_check):
            curr = parent_dir
            while curr and curr.exists() and not list(curr.iterdir()):
                try:
                    curr.rmdir()
                    curr = curr.parent
                except OSError:
                    break

        # Update session status in DB
        with sqlite3.connect(self.db_path) as conn:
            conn.execute("UPDATE sessions SET status = 'ROLLED_BACK' WHERE session_id = ?", (session.session_id,))
            conn.commit()

        return {
            "success": True,
            "session_id": session.session_id,
            "restored_count": restored_count,
            "failed_count": failed_count,
            "errors": errors,
            "message": f"Restored {restored_count} files successfully."
        }

    def prune_old_sessions(self, days: int = 30) -> int:
        """Prunes historical sessions and journal files older than specified days."""
        cutoff_date = (datetime.now() - timedelta(days=days)).isoformat()
        pruned = 0
        with sqlite3.connect(self.db_path) as conn:
            cursor = conn.execute("SELECT session_id FROM sessions WHERE start_time < ?", (cutoff_date,))
            session_ids = [r[0] for r in cursor.fetchall()]
            if session_ids:
                conn.execute("DELETE FROM sessions WHERE start_time < ?", (cutoff_date,))
                conn.commit()
                pruned = len(session_ids)

        journals_dir = self.data_dir / "journals"
        if journals_dir.exists():
            for sid in session_ids:
                jf = journals_dir / f"{sid}.json"
                if jf.exists():
                    try:
                        jf.unlink()
                    except OSError:
                        pass

        return pruned
