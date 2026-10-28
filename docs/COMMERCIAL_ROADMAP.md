# Smart File Organizer (SFO) - Commercial Roadmap & Product Editions

## 1. Product Editions Matrix

| Feature Dimension | Community Edition (Free / OSS) | Enterprise Edition (Commercial) |
| :--- | :--- | :--- |
| **Clean Architecture Core** | ✅ Full source code (MIT) | ✅ Full source code & binary bundles |
| **Multi-Strategy Organization** | ✅ Extension, Date, Size, Regex | ✅ Full + Smart Receipt Routing |
| **Exact & Visual Deduplication** | ✅ SHA-256 & Perceptual dHash | ✅ Multi-threaded hash acceleration |
| **Smart Bulk Renamer** | ✅ EXIF & ID3 metadata tokens | ✅ EXIF, ID3 & Batch regex casing |
| **Safety & Rollback** | ✅ 100% Journal Rollback | ✅ 100% Rollback + Blast Radius Cap |
| **Enterprise Guardrails** | ❌ None | ✅ System path blacklist & safety limits |
| **Cryptographic Audit Chain** | ❌ None | ✅ SHA-256 tamper-evident compliance log |
| **SFO Doctor Diagnostics** | ❌ Basic | ✅ Full latency benchmark & health suite |
| **Windows Shell Integration** | ✅ Explorer Context Menu | ✅ Explorer Menu + System Tray Daemon |
| **Scheduling Engine** | ❌ Polling loop only | ✅ Native Windows Task Scheduler (`schtasks`) |
| **Wireless LAN Drop** | ✅ Standalone HTTP server | ✅ Mobile UI with PIN authentication |
| **Cloud Storage Connectors** | ❌ Manual | ✅ Auto-detect OneDrive, GDrive, Dropbox |
| **Cold Storage Archiver** | ❌ Manual | ✅ Automated age-based ZIP compression |
| **Multi-Language (i18n)** | ❌ English only | ✅ Bilingual English / Tiếng Việt |
| **Binary Packaging** | ❌ Manual build | ✅ Automated GitHub Actions `.exe` Releases |

---

## 2. Version Release History
- **v1.0 (Legacy):** Basic extension sorter script.
- **v2.0 (Clean Architecture):** Decoupled domain engine, 3-tiered hasher, SQLite rollback journal, Tkinter dark theme GUI.
- **v2.1 (Sprint 1):** Smart Renamer (EXIF/ID3), Visual pHash cleaner, USB auto-sync, Receipt organizer.
- **v2.2 (Sprint 2):** Windows Explorer context menu, Storage Treemap, Dev Junk cleaner, Secret DLP scanner.
- **v2.3 (Sprint 3):** Cold Storage Archiver, Cloud Connector, Wireless LAN Drop.
- **v3.0 (Enterprise Scaling Milestone):** Enterprise Guardrails, SFO Doctor, i18n Localization (EN/VI), Windows Task Scheduler, System Tray Daemon, and PyInstaller Standalone Packaging.
