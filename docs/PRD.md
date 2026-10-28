# Product Requirements Document (PRD)

## Project Overview
- **Product Name:** Smart File Organizer (SFO)
- **Version:** 3.0.0 (Commercial Enterprise Edition)
- **Author:** Pham Minh Triet (Raven)
- **License:** MIT

## 1. Problem Statement
Users' computers, especially `Downloads`, `Desktop`, and shared network storage, quickly accumulate thousands of disordered files. Basic file organizers suffer from critical drawbacks:
1. They compare only file extensions, neglecting content duplicates.
2. They do full-file hashing on every file, causing severe disk thrashing and slow scans.
3. They lack reliable Undo/Rollback capabilities, causing anxiety over accidentally displaced files.
4. They lack an intuitive GUI for regular users while lacking robust CLI flags for power users and automation.
5. Inactive developer artifacts (`node_modules`, `.venv`) silently consume tens of gigabytes of disk space.
6. Leaked credentials and API tokens in local folders represent significant security hazards.

## 2. Product Objectives
- Provide a **zero-external-dependency** desktop & CLI tool runnable out-of-the-box on Python 3.10+.
- Guarantee 100% data safety with transaction journaling and a 1-click Rollback mechanism.
- Deliver lightning-fast duplicate detection using a **3-tiered hashing algorithm** (Size $\rightarrow$ 64KB partial hash $\rightarrow$ Full SHA-256) plus perceptual visual hashing (`dHash`).
- Support multiple organization strategies: File Type (150+ extensions), Modification/Creation Date (`YYYY/MM`), File Size buckets, Custom Regular Expressions, and Smart Financial Routing (`Finance_<Year>/<Vendor>`).
- Reclaim disk space via interactive 2D storage treemaps and automated developer junk cleaning.
- Prevent security leaks by scanning for sensitive API tokens and private keys with safe file quarantine.
- Provide seamless desktop integration with native Windows Explorer right-click context menus.
- Offer automated folder monitoring via a background daemon with download-stabilization debounce.

## 3. User Personas
- **Everyday Desktop User:** Wants a simple, modern dark-themed GUI where they select their Downloads folder, click "Preview", verify the proposed moves, and click "Execute" with full confidence they can undo anytime.
- **Power User / Developer:** Uses the CLI in terminal or automates it with scheduled tasks / cron / scripts to keep directories clean headlessly and reclaim disk space from bloated `node_modules`.
- **Data & Content Creator:** Needs to deduplicate large video/photo directories without risking data loss, batch rename media using camera/artist metadata, and sync backups to USB drives.

## 4. Key Requirements & Acceptance Criteria
- **AC-01 (Safe Dry-Run):** Running with `--dry-run` or checking "Dry-Run" in GUI MUST simulate all actions without moving or deleting any file.
- **AC-02 (3-Tiered Deduplication):** Files with differing byte sizes must never be read from disk for hashing. Files with identical sizes and identical 64KB hashes must undergo full SHA-256 verification before being declared duplicates.
- **AC-03 (Transaction Rollback):** Any live session must be 100% reversible via `python main.py --undo [session_id]` or GUI Undo button, returning files to original locations and cleaning empty category directories.
- **AC-04 (Collision Safety):** If a destination file with the same name already exists, the tool must automatically rename with an incremented counter `filename (1).ext` unless configured otherwise.
- **AC-05 (Download Stabilization):** Background watcher must wait until a downloading file size remains stable for 3 seconds and is not marked with `.crdownload` or `.tmp` before organizing.
- **AC-06 (Smart Bulk Renaming):** Extract EXIF (date, camera) and ID3 tags with customizable template tokens and casing transforms.
- **AC-07 (Perceptual Visual Hashing):** 64-bit dHash computation to cluster visually identical or resized photos with Hamming distance comparisons.
- **AC-08 (Smart Financial Sort):** Extract vendor and invoice date from PDFs and images (multilingual EN/VI) and route to `Finance_<Year>/<Vendor>`.
- **AC-09 (Removable USB Sync):** Auto-detect USB drives and support incremental copy and timestamped `.zip` snapshot archives.
- **AC-10 (Disk Storage Treemap):** Fast recursive directory analysis with ranked largest files and 2D treemap visualization.
- **AC-11 (Dev Junk Cleaner):** Scan and clean inactive `node_modules`, `.venv`, and build caches inactive for over 14 days.
- **AC-12 (Secret DLP Scanner):** Detect AWS keys, OpenAI tokens, GitHub PATs, and SSH keys with one-click quarantine into `_Quarantine/`.
- **AC-13 (Windows Explorer Shell):** Register/unregister right-click menu items under `HKCU` without requiring Administrator rights.
- **AC-14 (Cold Storage Archiver):** Scan files inactive for $\ge N$ days, compress into ZIP archives with internal metadata manifests, and safely reclaim disk space.
- **AC-15 (Cloud Storage Connector):** Auto-discover active local roots for OneDrive, Google Drive, Dropbox, and iCloud Drive, and sync folders with incremental/mirror/snapshot modes.
- **AC-16 (Wireless LAN Drop):** Built-in zero-dependency HTTP server with mobile upload/download UI and optional PIN protection for instant local wireless transfers.
- **AC-17 (Enterprise Guardrails):** Automatically block destructive execution against protected operating system directories (`C:\Windows`, `C:\Program Files`, etc.) and enforce blast radius thresholds.
- **AC-18 (Cryptographic Audit Chain):** Every file mutation must append an entry to an immutable SHA-256 hash-chained log for compliance audits.
- **AC-19 (SFO Doctor Diagnostics):** Provide automated health, latency, permission, and database checks via `sfo --doctor` and GUI.
- **AC-20 (Bilingual Localization & OS Scheduling):** Complete multi-language support (English & Tiếng Việt) and native Windows Task Scheduler integration (`schtasks`).
