# System Architecture & Technical Design (v4.0)

## 1. Principles
- **Layered workspace:** all file-handling logic lives in one library crate
  (`sfo-core`); the command line (`sfo-cli`) and the desktop app (`sfo-gui`)
  are thin front ends over it.
- **Native, self-contained:** two executables, no interpreter or runtime.
  Image decoding, EXIF, PDF text, SQLite and cryptography are compiled in.
- **Transactional integrity:** every file operation is recorded in SQLite and
  a JSON journal before the session ends, so it can be rolled back.
- **Compatible with v3.0 data:** history, journals, audit log, config, cold
  archives and vaults written by the Python v3.0 build are read unchanged
  (golden files in `crates/sfo-compat-tests`).
- **No `unsafe` code** in the workspace (`unsafe_code = "deny"`); Windows APIs
  are reached through `winreg`, `sysinfo` and `schtasks.exe`.

```
+-------------------------------------------------------------------------+
|                               FRONT ENDS                                |
|  crates/sfo-gui  (eframe / egui)          crates/sfo-cli  (clap)        |
|  - app.rs     window, tabs, worker jobs   - args.rs  flags (v3-compatible)
|  - tabs_*.rs  9 tabs, 8 sub-tabs          - run.rs   command router    |
|  - widgets.rs card, stat card, picker,    - out.rs   colours, tables   |
|               table, buttons                                            |
|  - dialog.rs  modal info/confirm                                        |
|  - theme.rs   slate dark palette                                        |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|                    PLATFORM & ENTERPRISE (sfo-core)                     |
|  platform::shell      Explorer context menu (HKCU)                      |
|  platform::startup    start on login (HKCU Run)                         |
|  platform::scheduler  Task Scheduler (schtasks)                         |
|  platform::notify     toast notifications (PowerShell/WinRT)            |
|  guardrails           protected paths, blast radius, SHA-256 audit chain|
|  doctor               health checks                                     |
|  i18n                 English / Tiếng Việt                              |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|                         CORE ENGINE (sfo-core)                          |
|  scanner       directory walk with ignore rules (fnmatch)               |
|  hasher        size -> 128 KB partial -> full SHA-256 (rayon, cached)   |
|  visual_hasher dHash / aHash, Pillow-compatible resize                  |
|  dedup         exact & visual groups, keep policies, plans              |
|  organizer     extension / date / size / regex / receipts               |
|  smart_filter  invoice classifier (lopdf text, keywords, vendors)       |
|  renamer       templates, EXIF, ID3, casing, regex                      |
|  executor      dry run, collisions, moves, deletes                      |
|  undo          SQLite + JSON journals, LIFO rollback                    |
|  reporter      console, Markdown, HTML reports                          |
|  syncer, cloud, archiver, watcher, lan_share, treemap, junk, secrets,   |
|  shredder, vault                                                        |
|  config, models, paths, pyjson, fsutil, fnmatch                         |
+-------------------------------------------------------------------------+
```

## 2. Tiered Deduplication
1. **Size grouping:** files are grouped by byte size; unique sizes are dropped
   without reading the disk. Empty files are ignored.
2. **Partial hash:** the first 128 KB of each candidate is hashed with
   SHA-256 in parallel; differing prefixes are dropped.
3. **Full SHA-256:** only the survivors are streamed in 2 MB buffers.
Hashes are cached per `(path, mtime, size, algorithm)` for the life of the
process.

## 3. Transaction Journaling & Rollback
Every session gets an ID `sfo_YYYYMMDD_HHMMSS_<hex>` and is written to:
- SQLite: `<data dir>/history.db` (WAL mode)
- JSON: `<data dir>/journals/<session_id>.json`

The data directory is `~/.smart_file_organizer` unless `SFO_DATA_DIR` is set.

During rollback:
- Successful MOVE, RENAME and DEDUPLICATE records are reversed in LIFO order.
- Files are moved back to their recorded source paths.
- Folders emptied by the rollback are removed, but never the session's own
  source or destination folder, nor anything above them.
- The session is marked `ROLLED_BACK`; dry runs cannot be rolled back.

## 4. Audit Chain
`audit_chain.jsonl` holds one JSON object per event. Each entry's hash is
`SHA-256(prev | timestamp | operator | action | json(details, sorted keys))`.
The JSON serialisation reproduces Python's `json.dumps` byte for byte so that
chains started by v3.0 keep verifying.

## 5. Vault Format
- **v2 (written by v4):** `SFOVAULT | u16 2 | kdf id | Argon2id m/t/p | salt[16]
  | nonce[24] | ciphertext+tag`, XChaCha20-Poly1305 with the whole header as
  associated data.
- **v1 (v3.0, read-only):** PBKDF2-HMAC-SHA256 ×100 000, then either
  AES-256-GCM or a SHA-256 keystream + HMAC; both are accepted.
  `sfo --vault-upgrade` rewrites a v1 file as v2.

## 6. Desktop App Threading
The egui window renders every frame on the UI thread. Scans, hashing, file
moves, encryption, sync and diagnostics run on a worker thread; results come
back through a channel. One job runs at a time, and its buttons are disabled
until it finishes. The LAN Drop server runs its own 8 worker threads and stops
when the window closes.
