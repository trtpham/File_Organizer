# SFO Doctor: Health & Diagnostics Manual

## Overview
`sfo --doctor` (and the Doctor tab) provides a built-in automated health inspection utility for diagnosing workstation storage latency, permissions, system dependencies, and database integrity.

---

## Running SFO Doctor

### From Command-Line:
```bash
sfo --doctor
sfo --doctor --source "D:/Downloads"
```
The exit code is 0 when the status is `HEALTHY` and 1 when any check fails.

### From Desktop GUI:
1. Open Smart File Organizer.
2. Navigate to the **🩺 Doctor & Health** tab.
3. Click **🩺 Run System Diagnostics**. The checks run in the background.

---

## Diagnostic Check Reference

| Diagnostic Check | PASS Criteria | WARN / FAIL Threshold | Recommended Action |
| :--- | :--- | :--- | :--- |
| **SFO Runtime** | Always | — | Shows the SFO version. |
| **Operating System** | Always | — | Shows OS and CPU architecture. |
| **System Memory (RAM)** | $\ge 1\text{ GB}$ available | $< 1\text{ GB}$ (WARN) | Close memory-heavy apps before large scans. |
| **Disk Headroom** | Free space $\ge 5\text{ GB}$ | $< 5\text{ GB}$ (WARN), $< 1\text{ GB}$ (FAIL) | Run Cold Storage Archiver or Dev Junk Cleaner. |
| **Disk Write Latency** | Flush time $< 50\text{ ms}$ | $\ge 50\text{ ms}$ (WARN), $\ge 150\text{ ms}$ (FAIL) | High I/O contention or failing hard drive sector. |
| **Windows Explorer Menu** | HKCU key registered | Not registered (WARN) | Run `sfo --install-context-menu` to enable. |
| **Cloud Integrations** | Active sync folder found | No sync folders (WARN) | Connect OneDrive, Google Drive, or Dropbox. |
| **Built-in Modules** | Always | — | Image, EXIF, PDF and encryption support are compiled in; nothing to install. |
| **Database Integrity** | SQLite `PRAGMA integrity_check` is `ok` | Integrity warning (FAIL) | Restore `history.db` from a backup. |
| **Audit Log Integrity** | SHA-256 chain verifies | Broken chain (FAIL) | Investigate who edited `audit_chain.jsonl`. |
