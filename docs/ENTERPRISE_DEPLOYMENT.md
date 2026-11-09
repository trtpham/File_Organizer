# Smart File Organizer (SFO) - Enterprise Deployment & Operations Guide

## 1. Overview
Smart File Organizer (SFO) v4.0 is two native executables (`sfo`, `sfo-gui`) with no runtime dependencies. It provides enterprise-grade workstation cleanup, automated tiered deduplication, Data Loss Prevention (DLP) secret detection, and policy-driven cold storage archiving across managed Windows environments.

---

## 2. System Requirements
- **Supported Operating Systems:** Windows 10 (Build 19041+), Windows 11, Windows Server 2019/2022, macOS 12+, Ubuntu 20.04+.
- **Architecture:** x86_64 / ARM64.
- **Privilege Model:** Runs under standard non-elevated user context (`asInvoker`). Administrative rights are **not** required for user directory organization, context menu integration, or local Task Scheduler tasks.

---

## 3. Deployment Modes

### 3.1. Standalone Portable Deployment (Zero-Install)
1. Download `SmartFileOrganizer_<version>_x86_64-pc-windows-msvc.zip` from Releases.
2. Extract to a user directory (e.g. `%LOCALAPPDATA%\SmartFileOrganizer`). Keep `sfo.exe` and `sfo-gui.exe` together.
3. Run `sfo-gui.exe`, or `sfo.exe` for scripted use.
4. Optional: set `SFO_DATA_DIR` to keep history, audit log and settings next to the executables.

### 3.2. Automated Scheduled Maintenance via Group Policy (GPO)
To deploy an automated daily organization sweep across organizational endpoints:

```cmd
schtasks /Create /TN "SmartFileOrganizer_DailySweep" /TR "\"C:\Program Files\SFO\sfo.exe\" --source \"%USERPROFILE%\Downloads\" -r --smart-receipts" /SC DAILY /ST 18:00 /F
```

---

## 4. Enterprise Safety Guardrails & Blacklist
SFO enforces safety boundaries to protect system integrity:
1. **Critical Path Blacklist:** Direct execution against `C:\Windows`, `C:\Program Files`, `C:\System Volume Information`, and root volume paths (`C:\`) is blocked by default.
2. **Blast Radius Limiter:** A live operation that would touch more than `blast_radius.max_files` (default 500) files or `blast_radius.max_bytes` (default 10 GB) is refused by the CLI unless `--force` is given, and needs an explicit red confirmation in the desktop app. Every organize, dedupe and rename session can be undone.
3. **Cryptographic SHA-256 Audit Chain:** Every session, shred and vault action appends a hashed record to `~/.smart_file_organizer/audit_chain.jsonl`.
   - To verify log integrity for SOC2/compliance:
     ```cmd
     sfo.exe --verify-audit
     ```

---

## 5. Centralized Configuration
SFO reads its built-in defaults, then the user's `config.json` in the data directory (`~/.smart_file_organizer/`, or `SFO_DATA_DIR`). For a managed rollout, either provision that file per user or pass a shared file explicitly with `sfo --config <path>`; values in it override the defaults key by key.

## 6. Upgrading from v3.0
History, journals, audit log and user config stay in the same place and keep working. The Explorer menu, autostart entry and scheduled tasks created by v3.0 point at the old Python entry point; re-create them with `sfo --install-context-menu`, `sfo --enable-autostart` and `sfo --schedule-daily`. Re-encrypt old vaults with `sfo --vault-upgrade`.
