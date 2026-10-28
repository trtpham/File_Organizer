# Smart File Organizer (SFO) - Enterprise Deployment & Operations Guide

## 1. Overview
Smart File Organizer (SFO) v3.0 Enterprise Edition provides enterprise-grade workstation cleanup, automated tiered deduplication, Data Loss Prevention (DLP) secret detection, and policy-driven cold storage archiving across managed Windows environments.

---

## 2. System Requirements
- **Supported Operating Systems:** Windows 10 (Build 19041+), Windows 11, Windows Server 2019/2022, macOS 12+, Ubuntu 20.04+.
- **Architecture:** x86_64 / ARM64.
- **Privilege Model:** Runs under standard non-elevated user context (`asInvoker`). Administrative rights are **not** required for user directory organization, context menu integration, or local Task Scheduler tasks.

---

## 3. Deployment Modes

### 3.1. Standalone Portable Deployment (Zero-Install)
1. Download `SmartFileOrganizer_v3.0_Windows_x64.zip`.
2. Extract to a network share or user directory (e.g. `%LOCALAPPDATA%\SmartFileOrganizer`).
3. Run `Launch_GUI.bat` or `sfo.exe`.

### 3.2. Automated Scheduled Maintenance via Group Policy (GPO)
To deploy an automated daily organization sweep across organizational endpoints:

```cmd
schtasks /Create /TN "SmartFileOrganizer_DailySweep" /TR "\"C:\Program Files\SFO\sfo.exe\" --source \"%USERPROFILE%\Downloads\" -r --smart-receipts" /SC DAILY /ST 18:00 /F
```

---

## 4. Enterprise Safety Guardrails & Blacklist
SFO enforces safety boundaries to protect system integrity:
1. **Critical Path Blacklist:** Direct execution against `C:\Windows`, `C:\Program Files`, `C:\System Volume Information`, and root volume paths (`C:\`) is blocked by default.
2. **Blast Radius Limiter:** Any operation affecting more than $500$ files or $10\text{ GB}$ of data requires explicit confirmation or `--force`.
3. **Cryptographic SHA-256 Audit Chain:** Every operation appends a hashed record to `~/.smart_file_organizer/audit_chain.jsonl`.
   - To verify log integrity for SOC2/compliance:
     ```cmd
     sfo.exe --verify-audit
     ```

---

## 5. Centralized Configuration
Corporate defaults can be provisioned by deploying `default_config.json` to `%PROGRAMDATA%\SmartFileOrganizer\config.json`.
