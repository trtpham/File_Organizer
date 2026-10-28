# SFO Doctor: Health & Diagnostics Manual

## Overview
`sfo --doctor` provides a built-in automated health inspection utility for diagnosing workstation storage latency, permissions, system dependencies, and database integrity.

---

## Running SFO Doctor

### From Command-Line:
```bash
python main.py --doctor
# Or via compiled binary:
sfo.exe --doctor
```

### From Desktop GUI:
1. Open Smart File Organizer.
2. Navigate to the **🩺 Doctor & Health** tab.
3. Click **🩺 Run System Diagnostics**.

---

## Diagnostic Check Reference

| Diagnostic Check | PASS Criteria | WARN / FAIL Threshold | Recommended Action |
| :--- | :--- | :--- | :--- |
| **Python Runtime** | Version $\ge 3.10$ | $< 3.10$ (FAIL) | Upgrade Python or use standalone binary. |
| **Operating System** | Windows, Linux, Darwin | Unsupported OS | Verified on NT and POSIX kernels. |
| **Disk Headroom** | Free space $\ge 5\text{ GB}$ | $< 5\text{ GB}$ (WARN), $< 1\text{ GB}$ (FAIL) | Run Cold Storage Archiver or Dev Junk Cleaner. |
| **Disk Write Latency** | Flush time $< 50\text{ ms}$ | $\ge 50\text{ ms}$ (WARN), $\ge 150\text{ ms}$ (FAIL) | High I/O contention or failing hard drive sector. |
| **Windows Explorer Menu** | HKCU key registered | Not registered (WARN) | Run `sfo --install-context-menu` to enable. |
| **Cloud Integrations** | Active sync folder found | No sync folders (WARN) | Connect OneDrive, Google Drive, or Dropbox. |
| **Accelerators** | Pillow, PyMuPDF loaded | Standard Library fallback | Optional: run `pip install pillow pymupdf`. |
| **Database Integrity** | SQLite PRAGMA `ok` | Integrity warning (FAIL) | Re-initialize database or restore backup. |
