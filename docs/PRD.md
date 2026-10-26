# Product Requirements Document (PRD)

## Project Overview
- **Product Name:** Smart File Organizer (SFO)
- **Version:** 2.0.0
- **Author:** Pham Minh Triet (Raven)
- **License:** MIT

## 1. Problem Statement
Users' computers, especially `Downloads`, `Desktop`, and shared network storage, quickly accumulate thousands of disordered files. Basic file organizers suffer from critical drawbacks:
1. They compare only file extensions, neglecting content duplicates.
2. They do full-file hashing on every file, causing severe disk thrashing and slow scans.
3. They lack reliable Undo/Rollback capabilities, causing anxiety over accidentally displaced files.
4. They lack an intuitive GUI for regular users while lacking robust CLI flags for power users and automation.

## 2. Product Objectives
- Provide a **zero-external-dependency** desktop & CLI tool runnable out-of-the-box on Python 3.10+.
- Guarantee 100% data safety with transaction journaling and a 1-click Rollback mechanism.
- Deliver lightning-fast duplicate detection using a **3-tiered hashing algorithm** (Size $\rightarrow$ 64KB partial hash $\rightarrow$ Full SHA-256).
- Support multiple organization strategies: File Type (150+ extensions), Modification/Creation Date (`YYYY/MM`), File Size buckets, and Custom Regular Expressions.
- Offer automated folder monitoring via a background daemon with download-stabilization debounce.

## 3. User Personas
- **Everyday Desktop User:** Wants a simple, modern dark-themed GUI where they select their Downloads folder, click "Preview", verify the proposed moves, and click "Execute" with full confidence they can undo anytime.
- **Power User / Developer:** Uses the CLI in terminal or automates it with scheduled tasks / cron / scripts to keep directories clean headlessly.
- **Data & Content Creator:** Needs to deduplicate large video/photo directories without risking data loss.

## 4. Key Requirements & Acceptance Criteria
- **AC-01 (Safe Dry-Run):** Running with `--dry-run` or checking "Dry-Run" in GUI MUST simulate all actions without moving or deleting any file.
- **AC-02 (3-Tiered Deduplication):** Files with differing byte sizes must never be read from disk for hashing. Files with identical sizes and identical 64KB hashes must undergo full SHA-256 verification before being declared duplicates.
- **AC-03 (Transaction Rollback):** Any live session must be 100% reversible via `python main.py --undo [session_id]` or GUI Undo button, returning files to original locations and cleaning empty category directories.
- **AC-04 (Collision Safety):** If a destination file with the same name already exists, the tool must automatically rename with an incremented counter `filename (1).ext` unless configured otherwise.
- **AC-05 (Download Stabilization):** Background watcher must wait until a downloading file size remains stable for 3 seconds and is not marked with `.crdownload` or `.tmp` before organizing.
