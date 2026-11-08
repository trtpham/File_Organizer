# Rust Migration Plan — Smart File Organizer v3.0 → v4.0 (Rust)

Status: P0–P7 implemented (see table in §5); approvals in `TRANSITION_APPROVAL.md`
Date: 2026-11-08

## 1. Baseline (ground truth, surveyed 2026-11-08)

| Area | Python files | Lines |
|---|---|---|
| Core engine (`smart_file_organizer/core/`) | 28 modules | ~6,100 |
| CLI (`cli/app.py`, `parser.py`, `formatter.py`) | 3 | ~1,200 |
| GUI (Tkinter, 9 tabs: `gui/app.py`, `theme.py`, `components.py`) | 3 | ~1,900 |
| Tests (pytest) | 28 | ~1,500 |
| Packaging | `sfo.spec`, `scripts/build_exe.py`, `.github/workflows/{ci,release}.yml` | — |

Toolchain present: `cargo 1.99.0`, `rustc 1.99.0`.

## 2. Decisions (PO, 2026-11-08)

| # | Decision | Choice |
|---|---|---|
| D1 | GUI stack | **egui / eframe** (single native .exe) |
| D2 | Transition | **Side-by-side** Cargo workspace; delete Python only at parity |
| D3 | Data compatibility | **Keep formats**: `~/.smart_file_organizer/history.db` schema, JSON journals, `config/default_config.json`, CLI flags |
| D4 | Vault crypto | **New vaults use vetted AEAD** (XChaCha20-Poly1305 or AES-256-GCM via RustCrypto, Argon2id/PBKDF2 KDF). Keep a **read-only v1 decoder** for existing `SFOVAULT` v1 files (custom XOR keystream + HMAC-SHA256) |

## 3. Target layout

```
Cargo.toml                 # workspace
crates/
  sfo-core/                # all domain logic (lib), no UI deps
  sfo-cli/                 # `sfo` binary (clap), same subcommands/flags as Python
  sfo-gui/                 # `sfo-gui` binary (eframe/egui)
  sfo-compat-tests/        # golden tests: data written by Python, read by Rust
smart_file_organizer/      # Python — removed in Phase 7 (v4.0)
```

## 4. Module → crate mapping

| Python module | Rust module (`sfo-core`) | Key crates |
|---|---|---|
| models.py | `models` | serde, chrono |
| config.py | `config` | serde_json |
| i18n.py | `i18n` | embedded tables (no runtime dep) |
| scanner.py | `scanner` | walkdir / ignore, globset |
| hasher.py | `hasher` | sha2, rayon |
| visual_hasher.py | `visual_hasher` | image (dHash) |
| deduplicator.py | `dedup` | — |
| organizer.py | `organizer` | — |
| executor.py | `executor` | — |
| undo_manager.py | `undo` | rusqlite (bundled) — same schema |
| enterprise_guardrails.py | `guardrails` | sha2 |
| renamer.py | `renamer` | kamadak-exif, id3 |
| smart_filter.py | `smart_filter` | pdf-extract / lopdf, regex |
| reporter.py | `reporter` | (format strings / askama) |
| junk_cleaner.py | `junk` | — |
| secret_scanner.py | `secrets` | regex |
| treemap.py | `treemap` | — |
| archiver.py | `archiver` | zip |
| syncer.py | `syncer` | — |
| watcher.py | `watcher` | notify, notify-debouncer |
| cloud_connector.py | `cloud` | dirs |
| lan_share.py | `lan_share` | tiny_http |
| shredder.py | `shredder` | rand |
| vault.py | `vault` | chacha20poly1305 / aes-gcm, argon2, pbkdf2, hmac, zeroize |
| shell_integration.py | `platform::shell` | winreg (cfg windows) |
| scheduler.py | `platform::scheduler` | std::process (schtasks) |
| tray_daemon.py | `platform::tray` | winreg, tray-icon / winrt-notification |
| doctor.py | `doctor` | sysinfo |
| cli/* | `sfo-cli` | clap, comfy-table / owo-colors |
| gui/* | `sfo-gui` | eframe, egui_extras, rfd |

## 5. Phases (each ends with DoD: `cargo test` green + ported tests pass + review)

| Phase | Scope | Exit criteria |
|---|---|---|
| P0 | Workspace scaffold, CI job (`cargo fmt --check`, `clippy -D warnings`, `test`) on Windows + Linux | CI green on empty crates |
| P1 | Foundation: models, config, i18n, scanner, hasher, guardrails, organizer, executor, undo | Rust reads a Python-written `history.db` + journal and rolls it back (compat test) — **DONE locally 2026-11-08**; `smart_filter` is a stub until P2 |
| P2 | dedup, visual_hasher, renamer, smart_filter, reporter | Ported tests for these modules pass — **DONE locally 2026-11-08** |
| P3 | junk, secrets, treemap, archiver, syncer, watcher, cloud, lan_share, shredder, vault | Vault: Rust opens Python v1 vault; new v2 AEAD round-trips — **DONE locally 2026-11-08** |
| P4 | Windows platform: shell menu, scheduler, tray/startup, doctor | Manual check on Windows 11 recorded in evidence — **DONE locally 2026-11-08** |
| P5 | `sfo` CLI parity — every subcommand/flag in `docs/CLI_USAGE.md` | Ported `test_cli.py` cases pass against Rust binary — **DONE locally 2026-11-08** |
| P6 | egui GUI — 9 tabs parity per `docs/GUI_GUIDE.md` | Manual walkthrough + screenshots — **Built 2026-11-08**; layout checked by self-captured screenshots; interactive click-through NOT_RUN |
| P7 | Release: replace PyInstaller with `cargo build --release` artifacts in `release.yml`; delete Python package, `setup.py`, `pyproject.toml`, `sfo.spec`, `tests/`; update README + docs | PO sign-off on parity checklist — **DONE 2026-11-08**: Python removed, release workflow builds Rust archives, docs rewritten; first tagged release not yet built |

## 6. Intentional behaviour changes vs Python v3.0

Recorded so P5/P6 parity reviews don't flag them as regressions.

| Area | Python v3.0 | Rust | Why |
|---|---|---|---|
| Undo cleanup | Deletes emptied folders upward with no limit | Stops at the session's source/destination folder; both survive (PO, 2026-11-08) | Could delete unrelated empty user folders |
| Vault format | v1: PBKDF2 + AES-GCM *or* SHA-256 XOR+HMAC under one version number | Writes v2 (Argon2id + XChaCha20-Poly1305, header as AAD); reads both v1 variants; `upgrade_vault` re-encrypts | D4 |
| Hash cache | Ignores algorithm (MD5 result could be returned for SHA-256) | Keyed by algorithm | Correctness |
| EXIF | Pillow `getexif()` sees IFD0 only (no DateTimeOriginal/ISO/FNumber) | Reads all IFDs | Intended behaviour of the Python code |
| ID3 UTF-16BE (enc 2) | Decoded as little-endian | Decoded as big-endian | Correctness |
| PDF text | PyMuPDF/PyPDF2 if installed, else raw-string regex | `lopdf` (first 5 pages), else the same regex | No Python deps |
| HTML report / LAN pages | File names inserted unescaped | HTML-escaped | XSS |
| Archive / vault extract | Archive restore trusted `../` member names | Unsafe member names skipped | Zip-slip |
| LAN Drop | No upload cap; overwrote same-name uploads; PIN compare not constant-time; bad Range → 500 | 512 MiB cap (413); unique names; constant-time; 416 | Hardening |
| Google Drive detect | Any existing `F:`–`J:` drive root counted | Only drives with `My Drive` | USB sticks misdetected |
| Watcher | Polling | Polling (same algorithm); `notify` not needed | Parity |
| Data directory | Always `~/.smart_file_organizer` | `SFO_DATA_DIR` overrides it (portable installs, isolated tests) | Testability |
| CLI | argparse | clap with the same flags/defaults/choices; adds `--vault-upgrade`, `slugify` for `--rename-case`; usage errors still exit 2; `sfo` with no args opens the GUI | D4 / parity |
| Explorer menu, autostart, schtasks | Point at `python main.py` | Point at the `sfo` executable next to the running binary | Native build |
| Toast notifications | Title/message spliced into a PowerShell script | Passed as environment variables | Script injection |
| Doctor | "Python Runtime", Pillow/PyMuPDF accelerators | "SFO Runtime", "Built-in Modules" | Native build |
| GUI | Tkinter; long work on the UI thread (window freezes) | egui; long work on a worker thread with a spinner, one job at a time | Responsiveness |
| GUI text | Hard-coded English | Tab names and main buttons go through `i18n`, so the language setting applies immediately | Translations existed but were unused |

## 7. Risks

| Risk | Mitigation |
|---|---|
| Undo DB schema drift breaks old history | Golden compat tests built from real Python output before P1 code |
| Vault v1 decode mismatch locks users out | Byte-for-byte test vectors generated by Python `vault.py` |
| PDF text extraction differs from Python output | Compare classifier result, not raw text |
| GUI parity is the largest single piece (~1,900 lines) | Keep GUI thin over `sfo-core`; land last |
| Weak v1 crypto stays readable | v1 is decrypt-only; CLI offers `vault upgrade` to re-encrypt as v2 |

## 8. Out of scope

- New features beyond Python v3.0 behaviour.
- Changes to the on-disk formats in D3 (except new vault v2).
