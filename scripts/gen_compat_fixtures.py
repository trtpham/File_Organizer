"""
Generates golden compatibility fixtures from the Python v3.0 build.

The Rust port (crates/sfo-compat-tests) must read every file this script
writes. Run from the repo root:

    python scripts/gen_compat_fixtures.py

Outputs (all under crates/sfo-compat-tests/tests/fixtures/):
  undo/history.db            SQLite DB written by UndoManager (WAL checkpointed)
  undo/journals/*.json       JSON journals written by UndoManager
  undo/meta.json             Workspace root used when recording, session ids, file contents
  audit/audit_chain.jsonl    Hash-chained audit log written by AuditLogChain
  config/expected.json       Lookups computed by ConfigManager for the default config
Also exports crates/sfo-core/data/translations.json from i18n.TRANSLATIONS.
"""

import gc
import json
import shutil
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO))

from smart_file_organizer.core.config import ConfigManager  # noqa: E402
from smart_file_organizer.core.enterprise_guardrails import AuditLogChain  # noqa: E402
from smart_file_organizer.core.executor import TransactionExecutor  # noqa: E402
from smart_file_organizer.core.i18n import TRANSLATIONS  # noqa: E402
from smart_file_organizer.core.organizer import Organizer  # noqa: E402
from smart_file_organizer.core.renamer import SmartBulkRenamer  # noqa: E402
from smart_file_organizer.core.scanner import FileScanner  # noqa: E402
from smart_file_organizer.core.undo_manager import UndoManager  # noqa: E402

FIXTURES = REPO / "crates" / "sfo-compat-tests" / "tests" / "fixtures"


def git_commit() -> str:
    try:
        return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
    except Exception:
        return "unknown"


def gen_undo(out: Path) -> None:
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    ws = Path(tempfile.mkdtemp(prefix="sfo_fixture_")).resolve()
    contents = {
        "photo1.jpg": "JPEG_IMAGE_DATA_12345",
        "report.pdf": "PDF_DOCUMENT_CONTENT_ABCDE",
        "notes.txt": "PLAIN_TEXT_NOTES",
        "tài liệu.docx": "UNICODE_NAME_CONTENT",
        "script.py": "print('Hello world!')",
    }
    for name, text in contents.items():
        (ws / name).write_text(text, encoding="utf-8")

    data_dir = out
    undo = UndoManager(data_dir=data_dir)
    cfg = ConfigManager()

    # Session 1: organize by extension (MOVE records).
    files = FileScanner(cfg).scan(ws)
    records = Organizer(cfg).plan_organization(files, ws)
    s1 = TransactionExecutor(cfg).execute_session(records, ws, ws, strategy="extension", dry_run=False)
    undo.save_session(s1)

    # Session 2: dry run (must never be undoable).
    files = FileScanner(cfg).scan(ws / "Documents")
    records = Organizer(cfg).plan_organization(files, ws / "Documents")
    s2 = TransactionExecutor(cfg).execute_session(records, ws, ws, strategy="extension", dry_run=True)
    undo.save_session(s2)

    # Session 3: rename inside Code/ (RENAME records), newest session.
    files = FileScanner(cfg).scan(ws / "Code")
    records = SmartBulkRenamer().plan_rename(files, template="renamed_{name}")
    s3 = TransactionExecutor(cfg).execute_session(records, ws / "Code", ws / "Code", strategy="rename", dry_run=False)
    undo.save_session(s3)

    # UndoManager's `with sqlite3.connect()` blocks commit but never close; drop
    # those connections before switching journal mode.
    del undo
    gc.collect()

    # Fold WAL into the main DB so the fixture is a single self-contained file.
    conn = sqlite3.connect(data_dir / "history.db")
    conn.execute("PRAGMA wal_checkpoint(TRUNCATE);")
    conn.execute("PRAGMA journal_mode=DELETE;")
    conn.close()
    for extra in ("history.db-wal", "history.db-shm"):
        p = data_dir / extra
        if p.exists():
            p.unlink()

    # Final on-disk state after both live sessions, relative to root.
    state = {
        str(p.relative_to(ws)).replace("\\", "/"): p.read_text(encoding="utf-8")
        for p in sorted(ws.rglob("*")) if p.is_file()
    }

    meta = {
        "python_commit": git_commit(),
        "root": str(ws),
        "sessions": {"organize": s1.session_id, "dry_run": s2.session_id, "rename": s3.session_id},
        "original_files": contents,
        "state_after_sessions": state,
    }
    (out / "meta.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False), encoding="utf-8")
    shutil.rmtree(ws, ignore_errors=True)


def gen_audit(out: Path) -> None:
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    chain = AuditLogChain(log_path=out / "audit_chain.jsonl")
    chain.log_event("ORGANIZE", {"moved_files": 10, "source": "C:\\Users\\demo\\Downloads"}, user="test_admin")
    chain.log_event("CLEAN_JUNK", {"freed_bytes": 500000, "ratio": 0.25, "tiny": 1e-05, "big": 1.5e16}, user="test_admin")
    chain.log_event("VAULT", {"name": "tài liệu 📄", "nested": {"b": [1, 2.0, None, True], "a": "x"}}, user="Phạm")
    ok, count, msg = chain.verify_integrity()
    assert ok and count == 3, msg


def gen_config(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    cfg = ConfigManager()
    exts = [".png", "docx", ".PY", ".ts", ".xyz123", "", "jpg"]
    names = [".git", "desktop.ini", "video.mp4.crdownload", "temp.tmp", "document.pdf",
             "Thumbs.DB", "a/.git/b", "C:\\x\\$RECYCLE.BIN", "file.part2"]
    sizes = [0, 500, 1048575, 1048576, 5 * 1024 * 1024, 50 * 1024 * 1024,
             500 * 1024 * 1024, 1073741824, 2 * 1024 * 1024 * 1024]
    expected = {
        "python_commit": git_commit(),
        "category_for_extension": {e: cfg.get_category_for_extension(e) for e in exts},
        "is_ignored": {n: cfg.is_ignored(n) for n in names},
        "size_folder": {str(s): cfg.get_size_folder(s) for s in sizes},
        "collision_strategy": cfg.collision_strategy.value,
        "date_format": cfg.date_format,
    }
    (out / "expected.json").write_text(json.dumps(expected, indent=2, ensure_ascii=False), encoding="utf-8")


def gen_vault(out: Path) -> None:
    """SFOVAULT v1 files. Without pycryptodome Python writes the XOR+HMAC variant."""
    from smart_file_organizer.core import vault as vault_mod
    from smart_file_organizer.core.vault import VaultEngine, VaultManager

    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    src = Path(tempfile.mkdtemp(prefix="sfo_vault_src_"))
    (src / "secret.txt").write_text("top secret ✓", encoding="utf-8")
    (src / "sub").mkdir()
    (src / "sub" / "data.bin").write_bytes(bytes(range(256)) * 40)
    VaultManager.lock_path(src, out / "folder_v1.sfovault", "correct horse ⚡")
    shutil.rmtree(src, ignore_errors=True)

    raw = VaultEngine.encrypt_data(b"raw payload \x00\x01 bytes" * 7, "pw")
    (out / "raw_v1.bin").write_bytes(raw)
    meta = {
        "python_commit": git_commit(),
        "variant": "aes_gcm" if vault_mod._HAS_PYCRYPTODOME else "xor_hmac",
        "folder_password": "correct horse ⚡",
        "folder_files": {"secret.txt": "top secret ✓", "sub/data.bin_len": 256 * 40},
        "raw_password": "pw",
        "raw_plaintext_hex": (b"raw payload \x00\x01 bytes" * 7).hex(),
    }
    (out / "meta.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False), encoding="utf-8")


def gen_archive(out: Path) -> None:
    """Cold-storage ZIP with the internal manifest, written by ColdStorageArchiver."""
    from smart_file_organizer.core.archiver import ColdStorageArchiver
    from smart_file_organizer.core.models import FileInfo

    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    base = Path(tempfile.mkdtemp(prefix="sfo_arch_")).resolve()
    (base / "old").mkdir()
    (base / "old" / "report 2019.txt").write_text("cold data " * 50, encoding="utf-8")
    (base / "photo.jpg").write_bytes(b"\xff\xd8" + b"x" * 500)
    files = [FileInfo.from_path(p) for p in sorted(base.rglob("*")) if p.is_file()]
    res = ColdStorageArchiver.create_cold_archive(files, out / "cold.zip", base_dir=base)
    assert res["success"], res
    meta = {"python_commit": git_commit(), "base_dir": str(base),
            "members": {"old/report 2019.txt": "cold data " * 50}}
    (out / "meta.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False), encoding="utf-8")
    shutil.rmtree(base, ignore_errors=True)


def gen_visual(out: Path) -> None:
    """Pillow dHash values for synthetic images."""
    from PIL import Image
    from smart_file_organizer.core.visual_hasher import PerceptualHasher

    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    img = Image.new("L", (100, 100))
    for x in range(100):
        for y in range(100):
            img.putpixel((x, y), int(255 - x * 2.5))
    img.save(out / "gradient.png")
    rgb = Image.new("RGB", (64, 48))
    for x in range(64):
        for y in range(48):
            rgb.putpixel((x, y), ((x * 37 + y * 11) % 256, (x * y) % 256, (y * 53) % 256))
    rgb.save(out / "pattern.png")
    hashes = {n: f"{PerceptualHasher.compute_dhash(out / n):016x}" for n in ("gradient.png", "pattern.png")}
    (out / "expected.json").write_text(json.dumps({"python_commit": git_commit(), "dhash": hashes}, indent=2),
                                       encoding="utf-8")


def export_translations() -> None:
    target = REPO / "crates" / "sfo-core" / "data" / "translations.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(TRANSLATIONS, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


if __name__ == "__main__":
    gen_undo(FIXTURES / "undo")
    gen_audit(FIXTURES / "audit")
    gen_config(FIXTURES / "config")
    gen_vault(FIXTURES / "vault")
    gen_archive(FIXTURES / "archive")
    gen_visual(FIXTURES / "visual")
    export_translations()
    print(f"Fixtures written to {FIXTURES}")
