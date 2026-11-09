# Smart File Organizer 📂 (v4.0, native Rust)

[![CI / Rust](https://github.com/trtpham/File_Organizer/actions/workflows/rust.yml/badge.svg)](https://github.com/trtpham/File_Organizer/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

**Smart File Organizer (SFO)** declutters, categorizes, deduplicates, renames,
syncs, archives and protects your files — from a desktop app (`sfo-gui`) or the
command line (`sfo`). Every change it makes is journaled and can be undone.

Version 4.0 is a complete rewrite in Rust: two self-contained executables, no
Python, no runtime to install. The desktop app is built with
[egui](https://github.com/emilk/egui) (the Tkinter UI of v3 is gone). History,
undo journals, audit logs, settings, cold archives and vaults created by v3.0
keep working.

---

## Features

- **Organize** by extension category (150+ formats), date (`YYYY/MM`, …), size
  bucket or your own regex rules; optional date sub-folders.
- **Smart Financial Sort** — detects invoices/receipts (English & Vietnamese
  keywords, vendors such as EVN, Viettel, AWS, Google) in PDF/TXT/CSV and files
  them under `Finance_<Year>/<Vendor>`.
- **Dry run, collisions and 100% undo** — preview before anything moves;
  collisions become `file (1).ext`, are skipped, overwritten or compared by
  hash; every session is journaled (SQLite + JSON) and `sfo --undo` restores it.
- **Duplicate Center** — exact duplicates by size → 128 KB partial hash →
  full SHA-256, and visually similar photos by 64-bit dHash with a similarity
  threshold. Move them to `_Duplicates/` (undoable) or delete them.
- **Smart Renamer** — templates with `{name}`, `{counter:03d}`, `{date}`,
  `{time}`, `{camera}`, `{iso}`, `{artist}`, `{title}`, `{album}` … from EXIF
  and ID3, find/replace (plain or regex), and casing (`lowercase`,
  `snake_case`, `slugify`, …).
- **Storage Treemap** — size tree, slice-and-dice treemap and the largest files.
- **USB & folder sync** — removable-drive detection, incremental copy (staged
  writes), timestamped ZIP snapshots.
- **Cloud folders** — finds OneDrive, Google Drive, Dropbox, iCloud Drive,
  Nextcloud and Box folders and syncs into them; lists sync conflicts.
- **LAN Drop** — share a folder with phones and laptops on the same Wi-Fi:
  browse, download (resumable) and upload into `SFO_Incoming/`, with an
  optional PIN.
- **Cold storage** — finds files untouched for N days, packs them into a
  verified ZIP with a manifest, optionally deletes originals, restores them
  with their timestamps.
- **Dev junk cleaner** — inactive `node_modules`, venvs, `__pycache__`,
  `target/`, `.next/`, `.gradle/` …
- **Secret scanner (DLP)** — AWS, OpenAI, GitHub, Slack, Stripe, Google keys,
  JWTs, private keys, connection strings, password assignments, Vietnamese
  citizen IDs; masked output and one-click quarantine.
- **Secure shredder** — 1/2/3/7-pass overwrite, truncate, rename, delete. On
  SSDs and synced folders copies may survive outside the file.
- **Encrypted vault** — `.sfovault` containers sealed with Argon2id +
  XChaCha20-Poly1305; v3 vaults still open and `--vault-upgrade` re-encrypts
  them.
- **Safety** — operations on drive roots and Windows system folders are
  refused; live operations over 500 files or 10 GB need explicit confirmation
  (`--force` on the command line); long scans and moves can be cancelled; a SHA-256 hash-chained audit log records sessions, shreds and vault
  actions (`sfo --verify-audit`).
- **Windows integration** — Explorer right-click menu, start on login,
  Task Scheduler daily runs, toast notifications (all per-user, no admin).
- **SFO Doctor** — memory, disk headroom, write latency, Explorer menu, cloud
  folders, database and audit-log integrity.
- **Watcher** — waits for downloads to finish, then organizes each file in its
  own undoable session; start and stop it from the Organize tab or run `sfo --watch`.
- **English / Tiếng Việt** interface.

---

## Install

### Download

Get the archive for your platform from
[Releases](https://github.com/trtpham/File_Organizer/releases), unzip it
anywhere and run `sfo-gui` (desktop) or `sfo` (command line). Keep both files
in the same folder: `sfo --gui` and the Explorer menu start the app next to
them.

### Build from source

Requires a recent stable Rust toolchain
([rustup.rs](https://rustup.rs)).

```bash
git clone https://github.com/trtpham/File_Organizer.git
```

```bash
cd File_Organizer && cargo build --release
```

The binaries are `target/release/sfo` and `target/release/sfo-gui`
(`.exe` on Windows). On Linux the GUI needs the GTK 3 and X11/xkb development
packages to build (`libgtk-3-dev libxkbcommon-dev libxcb-render0-dev
libxcb-shape0-dev libxcb-xfixes0-dev` on Debian/Ubuntu).

---

## Desktop app

Run `sfo-gui`, or `sfo` with no arguments. Nine tabs:

1. **Organize** — folders, strategy, dry run, Smart Financial Sort, preview, execute.
2. **Duplicate Center** — exact or visual mode, similarity, keep policy.
3. **Smart Renamer** — template, find/replace, casing, preview, rename.
4. **Storage Treemap** — treemap and largest files.
5. **Sync, Cloud & LAN** — USB backup, cloud sync, LAN Drop, cold storage.
6. **Security & Junk** — junk cleaner, secret scanner, shredder, vault.
7. **Doctor & Health** — diagnostics.
8. **Undo & History** — select a session and undo it.
9. **Settings & Shell** — language, start on login, audit-log check, Explorer menu, categories.

Long operations run in the background; the window stays responsive. See
[docs/GUI_GUIDE.md](docs/GUI_GUIDE.md).

---

## Command line

Preview first, then run for real, then undo if you change your mind:

```bash
sfo --source "C:/Users/Raven/Downloads" --dry-run
```

```bash
sfo --source "C:/Users/Raven/Downloads"
```

```bash
sfo --undo
```

More examples:

```bash
sfo --source "D:/Photos" --by-date
sfo --source "D:/Storage" --dedupe --dedupe-action report
sfo --source "D:/Photos" --visual-dedupe --similarity 0.90
sfo --source "D:/Photos" --rename-template "{date}_{camera}_{counter:03d}"
sfo --source "C:/Users/Raven/Downloads" --smart-receipts
sfo --source "C:/Users/Raven/Downloads" --watch
sfo --sync-source "C:/Users/Raven/Documents" --sync-dest "E:/Backup" --sync-mode snapshot
sfo --source "D:/Archives" --archive-cold --archive-days 90
sfo --source "C:/Users/Raven/Downloads" --lan-share --lan-pin 4321
sfo --source "D:/Projects" --scan-secrets
sfo --vault-lock "D:/Tax" --dest "D:/Tax.sfovault"
sfo --doctor
sfo --history
```

Every flag is in [docs/CLI_USAGE.md](docs/CLI_USAGE.md) and `sfo --help`.

---

## Data and upgrading from v3.0

SFO keeps its data in `~/.smart_file_organizer/` (`history.db`, `journals/`,
`audit_chain.jsonl`, `config.json`), exactly where v3.0 did, so existing
sessions can still be undone. Set `SFO_DATA_DIR` to use another folder (for a
portable install). If you registered the Explorer menu, autostart or a
scheduled task with v3.0, register them again from v4 so they point at the new
executables.

---

## Development

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

| Crate | What it is |
|---|---|
| `crates/sfo-core` | All file-handling logic (library) |
| `crates/sfo-cli` | The `sfo` command-line program |
| `crates/sfo-gui` | The `sfo-gui` desktop app (eframe/egui) |
| `crates/sfo-compat-tests` | Golden files written by v3.0 that v4 must read |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Releases are built by
`.github/workflows/release.yml` when a `v*` tag is pushed.

---

## License

MIT — see [LICENSE](LICENSE).

## Author

Pham Minh Triet (Raven) · [@trtpham](https://github.com/trtpham) ·
<https://github.com/trtpham/File_Organizer>
