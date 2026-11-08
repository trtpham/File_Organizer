//! Tabs 5–6: Sync, Cloud & LAN (4 sub-tabs) and Security & Junk (4 sub-tabs).

use crate::app::{App, Done};
use crate::dialog::{Confirmed, Dialog};
use crate::tabs_main::{combo, fb};
use crate::theme;
use crate::widgets::{self, Kind, Pick};
use egui::{Align, Layout, Ui};
use sfo_core::guardrails::AuditLogChain;
use sfo_core::lan_share::LanShareServer;
use sfo_core::models::{SyncMode, SyncProfile};
use sfo_core::shredder::{self, ShredMethod};
use sfo_core::vault::{self, KdfParams, VaultError};
use sfo_core::{archiver, cloud, junk, secrets, syncer};
use std::path::PathBuf;

const SHRED_METHODS: [(&str, ShredMethod); 4] = [
    (
        "DoD 5220.22-M (3-Pass: 0x00, 0xFF, Random)",
        ShredMethod::Dod3Pass,
    ),
    ("Quick Wipe (1-Pass: Random)", ShredMethod::Quick1Pass),
    ("NIST SP 800-88 (2-Pass)", ShredMethod::Nist80088),
    ("Gutmann (7-Pass High-Security)", ShredMethod::Gutmann7Pass),
];

fn vault_msg(e: &VaultError) -> String {
    match e {
        VaultError::InvalidPassword => {
            "Incorrect passphrase or vault data has been tampered with!".into()
        }
        other => other.to_string(),
    }
}

fn log_sync(log: &mut String, r: &syncer::SyncResult) {
    log.push_str(&format!("[+] {}\n", r.message));
    for e in &r.errors {
        log.push_str(&format!("[!] Error: {e}\n"));
    }
}

impl App {
    // ================================================================ Sync
    pub fn sync_ui(&mut self, ui: &mut Ui) {
        let labels = [
            "🔌 USB Drive Backup",
            "☁️ Cloud Storage Sync",
            "📡 Cross-Device LAN Drop",
            "🗄️ Cold Storage Archive",
        ]
        .map(String::from);
        widgets::sub_tabs(ui, &mut self.sync.sub, &labels);
        match self.sync.sub {
            0 => self.usb_ui(ui),
            1 => self.cloud_ui(ui),
            2 => self.lan_ui(ui),
            _ => self.cold_ui(ui),
        }
    }

    pub fn refresh_usb(&mut self) {
        let drives = syncer::removable_drives();
        if drives.is_empty() {
            self.sync.usb_label = "No removable USB drive detected.".into();
        } else {
            let names: Vec<String> = drives
                .iter()
                .map(|d| format!("{} ({})", d.path, d.label))
                .collect();
            self.sync.usb_label = format!("Connected USB: {}", names.join(", "));
            self.sync.usb_dest = drives[0].path.clone();
        }
    }

    fn usb_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::status_title(&self.sync.usb_label, theme::ACCENT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, Kind::Secondary, "🔄 Refresh USB Drives").clicked() {
                        self.refresh_usb();
                    }
                });
            });
            widgets::folder_picker(ui, "Source Folder:", &mut self.sync.usb_source);
            widgets::folder_picker(ui, "Target / USB Path:", &mut self.sync.usb_dest);
            ui.horizontal(|ui| {
                ui.label("Sync Mode:");
                combo(
                    ui,
                    "usb_mode",
                    &mut self.sync.usb_mode,
                    &["incremental", "mirror", "snapshot"],
                    120.0,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Primary, "💾 Sync to USB", idle).clicked()
                    {
                        self.run_usb_sync();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_ROW);
        ui.label(theme::header("USB Synchronization Log"));
        widgets::log_view(ui, "usb_log", &self.sync.usb_log);
    }

    fn run_usb_sync(&mut self) {
        let Some(src) = self.existing_dir(&self.sync.usb_source.clone()) else {
            return;
        };
        let dst = PathBuf::from(self.sync.usb_dest.trim());
        let mode = match self.sync.usb_mode.as_str() {
            "mirror" => SyncMode::Mirror,
            "snapshot" => SyncMode::Snapshot,
            _ => SyncMode::Incremental,
        };
        self.sync.usb_log.push_str(&format!(
            "\n[*] Starting {} sync: {} -> {}\n",
            self.sync.usb_mode.to_uppercase(),
            src.display(),
            dst.display()
        ));
        let profile = SyncProfile {
            name: "Desktop Sync".into(),
            source_dir: src,
            dest_dir: dst,
            mode,
            enabled: true,
            trigger_on_usb: false,
            usb_volume_name: None,
        };
        self.spawn("Synchronizing...", move || {
            Done::UsbSynced(syncer::sync(&profile, false))
        });
    }

    pub fn refresh_clouds(&mut self) {
        self.sync.clouds = cloud::detect_all();
        self.sync.cloud_idx = 0;
    }

    fn cloud_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Cloud Provider:");
                let names: Vec<String> = self
                    .sync
                    .clouds
                    .iter()
                    .map(|c| format!("{} ({})", c.name, c.local_path.display()))
                    .collect();
                let selected = names
                    .get(self.sync.cloud_idx)
                    .cloned()
                    .unwrap_or_else(|| "No active cloud folders found".into());
                egui::ComboBox::from_id_salt("cloud")
                    .selected_text(selected)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut self.sync.cloud_idx, i, n);
                        }
                    });
                if widgets::button(ui, Kind::Secondary, "🔄 Detect Cloud Folders").clicked() {
                    self.refresh_clouds();
                }
            });
            widgets::folder_picker(ui, "Source Folder:", &mut self.sync.cloud_source);
            ui.horizontal(|ui| {
                ui.label("Cloud Subfolder:");
                ui.add_sized(
                    [160.0, theme::BUTTON_HEIGHT],
                    egui::TextEdit::singleline(&mut self.sync.cloud_sub),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Primary, "☁️ Sync to Cloud", idle)
                        .clicked()
                    {
                        self.run_cloud_sync();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_ROW);
        ui.label(theme::header("Cloud Synchronization Activity Log"));
        widgets::log_view(ui, "cloud_log", &self.sync.cloud_log);
    }

    fn run_cloud_sync(&mut self) {
        let Some(provider) = self.sync.clouds.get(self.sync.cloud_idx).cloned() else {
            self.dialogs.push(Dialog::warn(
                "Notice",
                "No cloud storage provider detected.",
            ));
            return;
        };
        let Some(src) = self.existing_dir(&self.sync.cloud_source.clone()) else {
            return;
        };
        let sub = if self.sync.cloud_sub.trim().is_empty() {
            "SFO_Cloud_Sync".to_string()
        } else {
            self.sync.cloud_sub.trim().to_string()
        };
        self.sync.cloud_log.push_str(&format!(
            "\n[*] Syncing {} -> {} ({})\n",
            src.display(),
            provider.name,
            provider.local_path.join(&sub).display()
        ));
        self.spawn("Syncing to cloud...", move || {
            Done::CloudSynced(cloud::sync_to_cloud(
                &src,
                &provider,
                &sub,
                SyncMode::Incremental,
                false,
            ))
        });
    }

    fn lan_ui(&mut self, ui: &mut Ui) {
        let running = self
            .sync
            .lan
            .as_ref()
            .is_some_and(LanShareServer::is_running);
        widgets::card(ui, |ui| {
            ui.label(theme::header(&sfo_core::i18n::t("lan_drop_title")));
            ui.label(theme::muted(
                "Share files wirelessly with smartphones, tablets, and laptops on the same Wi-Fi.\n\
                 Phones can download files from this PC and upload photos/documents directly into SFO_Incoming/.",
            ));
            ui.add_space(theme::GAP_ROW);
            ui.add_enabled_ui(!running, |ui| {
                widgets::folder_picker(ui, "Folder to Share:", &mut self.sync.lan_folder)
            });
            ui.horizontal(|ui| {
                ui.label("Port:");
                ui.add_enabled(
                    !running,
                    egui::TextEdit::singleline(&mut self.sync.lan_port).desired_width(60.0),
                );
                ui.label("PIN (Optional):");
                ui.add_enabled(
                    !running,
                    egui::TextEdit::singleline(&mut self.sync.lan_pin).desired_width(80.0),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if running {
                        if widgets::button(ui, Kind::Danger, &sfo_core::i18n::t("stop_lan"))
                            .clicked()
                        {
                            if let Some(mut s) = self.sync.lan.take() {
                                s.stop();
                            }
                            self.info("LAN Drop", "LAN Drop server stopped.");
                        }
                    } else if widgets::button(ui, Kind::Primary, &sfo_core::i18n::t("start_lan"))
                        .clicked()
                    {
                        self.start_lan();
                    }
                    if widgets::button_enabled(
                        ui,
                        Kind::Secondary,
                        &sfo_core::i18n::t("open_browser"),
                        running,
                    )
                    .clicked()
                    {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(&self.sync.lan_url));
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::card(ui, |ui| {
            if running {
                ui.label(theme::status_title(
                    &sfo_core::i18n::t("lan_server_online"),
                    theme::SUCCESS,
                ));
                let pin = if self.sync.lan_pin.trim().is_empty() {
                    " | No PIN required".to_string()
                } else {
                    format!(" | PIN: {}", self.sync.lan_pin.trim())
                };
                ui.label(
                    egui::RichText::new(format!("👉 {}{pin}", self.sync.lan_url))
                        .font(theme::bold(theme::HEADER)),
                );
            } else {
                ui.label(theme::status_title(
                    &sfo_core::i18n::t("lan_server_offline"),
                    theme::ACCENT,
                ));
                ui.label(theme::muted(
                    "Start server to generate access URL for mobile devices.",
                ));
            }
        });
    }

    fn start_lan(&mut self) {
        let Some(dir) = self.existing_dir(&self.sync.lan_folder.clone()) else {
            return;
        };
        let port = self.sync.lan_port.trim().parse().unwrap_or(8080);
        let pin = Some(self.sync.lan_pin.trim().to_string()).filter(|p| !p.is_empty());
        let mut server = LanShareServer::new(&dir, port, pin);
        match server.start() {
            Ok(url) => {
                self.sync.lan_url = url.clone();
                self.sync.lan = Some(server);
                self.info("LAN Drop Online", format!("Server running at:\n{url}\n\nOpen this link on any phone or laptop on your Wi-Fi!"));
            }
            Err(e) => self.error("Error", format!("Failed to start LAN server: {e}")),
        }
    }

    fn cold_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(ui, "Scan Folder:", &mut self.sync.cold_folder);
            ui.horizontal(|ui| {
                ui.label("Inactive for >=");
                ui.add(egui::TextEdit::singleline(&mut self.sync.cold_days).desired_width(44.0));
                ui.label("days");
                ui.checkbox(
                    &mut self.sync.cold_delete,
                    "Delete originals after ZIP creation",
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Secondary, "🔍 Find Cold Files", idle)
                        .clicked()
                    {
                        self.scan_cold();
                    }
                    if widgets::button_enabled(ui, Kind::Primary, "🗄️ Compress Cold Files", idle)
                        .clicked()
                    {
                        self.archive_clicked();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::table(
            ui,
            "cold",
            &["File Name", "Size", "Last Modified / Accessed", "Full Path"],
            &self.sync.cold_rows,
            None,
        );
    }

    pub fn scan_cold(&mut self) {
        let Some(dir) = self.existing_dir(&self.sync.cold_folder.clone()) else {
            return;
        };
        let days: u64 = self.sync.cold_days.trim().parse().unwrap_or(90);
        self.spawn(
            &format!("Scanning for files inactive >= {days} days..."),
            move || Done::Cold {
                files: archiver::scan_cold_files(&dir, days, 0, true),
                days,
            },
        );
    }

    fn cold_zip(&self) -> PathBuf {
        PathBuf::from(self.sync.cold_folder.trim()).join(format!(
            "ColdStorage_{}.zip",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ))
    }

    fn archive_clicked(&mut self) {
        if self.sync.cold_files.is_empty() {
            self.info(
                "Notice",
                "No cold files selected. Run 'Find Cold Files' first.",
            );
            return;
        }
        let zip = self.cold_zip();
        self.dialogs.push(Dialog::confirm(
            "Confirm Compression",
            format!(
                "Are you sure you want to package {} files into:\n{}?\n\nDelete originals: {}",
                self.sync.cold_files.len(),
                sfo_core::fsutil::name_of(&zip),
                if self.sync.cold_delete { "YES" } else { "NO" }
            ),
            Confirmed::ArchiveCold,
        ));
    }

    pub fn run_archive(&mut self) {
        let files = self.sync.cold_files.clone();
        let base = PathBuf::from(self.sync.cold_folder.trim());
        let (zip, delete) = (self.cold_zip(), self.sync.cold_delete);
        self.spawn("Compressing cold files...", move || {
            let r = archiver::create_cold_archive(&files, &zip, Some(&base), delete, None);
            Done::Archived {
                message: r.message,
                success: r.success,
                dest: zip,
            }
        });
    }

    // ============================================================ Security
    pub fn security_ui(&mut self, ui: &mut Ui) {
        let labels = [
            "🧹 Dev Junk Cleaner",
            "🛡️ Secret Scanner (DLP)",
            "🔥 Secure File Shredder",
            "🔒 Encrypted Vault",
        ]
        .map(String::from);
        widgets::sub_tabs(ui, &mut self.sec.sub, &labels);
        match self.sec.sub {
            0 => self.junk_ui(ui),
            1 => self.secrets_ui(ui),
            2 => self.shred_ui(ui),
            _ => self.vault_ui(ui),
        }
    }

    fn junk_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(ui, "Scan Folder:", &mut self.sec.junk_folder);
            ui.horizontal(|ui| {
                ui.label(theme::muted(&self.sec.junk_status));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Secondary, "🔍 Scan Junk", idle).clicked()
                    {
                        self.scan_junk();
                    }
                    if widgets::button_enabled(ui, Kind::Danger, "🗑️ Clean Discovered Junk", idle)
                        .clicked()
                    {
                        if self.sec.junk.is_empty() {
                            self.info("Notice", "No junk items discovered. Run 'Scan Junk' first.");
                        } else {
                            self.dialogs.push(Dialog::confirm_danger(
                                "Confirm Delete",
                                format!(
                                    "Are you sure you want to permanently delete {} junk folders?",
                                    self.sec.junk.len()
                                ),
                                Confirmed::CleanJunk,
                            ));
                        }
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::table(
            ui,
            "junk",
            &["Category", "Artifact Name", "Size", "Folder Path"],
            &self.sec.junk_rows,
            None,
        );
    }

    pub fn scan_junk(&mut self) {
        let Some(dir) = self.existing_dir(&self.sec.junk_folder.clone()) else {
            return;
        };
        self.spawn("Scanning for developer junk...", move || {
            Done::Junk(junk::scan_directory(&dir, 14))
        });
    }

    pub fn run_clean_junk(&mut self) {
        let items = self.sec.junk.clone();
        self.spawn("Cleaning junk...", move || {
            Done::JunkCleaned(junk::clean_junk(&items, false))
        });
    }

    fn secrets_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(ui, "Scan Folder:", &mut self.sec.secret_folder);
            ui.horizontal(|ui| {
                ui.label(theme::muted(&self.sec.secret_status));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Secondary, "🔍 Scan Secrets", idle)
                        .clicked()
                    {
                        self.scan_secrets();
                    }
                    if widgets::button_enabled(
                        ui,
                        Kind::Warning,
                        "🛡️ Quarantine Compromised Files",
                        idle,
                    )
                    .clicked()
                    {
                        if self.sec.secrets.is_empty() {
                            self.info("Notice", "No secret leaks to quarantine.");
                        } else {
                            let n: std::collections::BTreeSet<_> =
                                self.sec.secrets.iter().map(|f| &f.file_path).collect();
                            self.dialogs.push(Dialog::confirm(
                                "Confirm Quarantine",
                                format!("Move {} files into '_Quarantine' folder?", n.len()),
                                Confirmed::QuarantineSecrets,
                            ));
                        }
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::table(
            ui,
            "secrets",
            &[
                "Severity",
                "Secret Type",
                "File Name",
                "Line",
                "Masked Snippet",
            ],
            &self.sec.secret_rows,
            None,
        );
    }

    pub fn scan_secrets(&mut self) {
        let Some(dir) = self.existing_dir(&self.sec.secret_folder.clone()) else {
            return;
        };
        self.spawn("Scanning for sensitive credentials...", move || {
            Done::Secrets(secrets::scan_directory(&dir, true))
        });
    }

    pub fn run_quarantine(&mut self) {
        let findings = self.sec.secrets.clone();
        let root = PathBuf::from(self.sec.secret_folder.trim());
        self.spawn("Quarantining files...", move || {
            Done::Quarantined(
                secrets::quarantine_files(&findings, &root).map_err(|e| e.to_string()),
            )
        });
    }

    fn shred_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            ui.label(theme::header("Irreversible Data Erasure"));
            ui.label(theme::muted(
                "Overwrites file contents with multi-pass patterns, truncates, renames, then deletes. \
                 On SSDs and cloud-synced folders, copies may survive outside the file.",
            ));
            ui.add_space(theme::GAP_ROW);
            widgets::path_picker(
                ui,
                "Target Path:",
                &mut self.sec.shred_path,
                Pick::Either,
                130.0,
            );
            ui.horizontal(|ui| {
                widgets::field_label(ui, "Wiping Standard:", 130.0);
                egui::ComboBox::from_id_salt("shred_method")
                    .selected_text(SHRED_METHODS[self.sec.shred_method].0)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for (i, (label, _)) in SHRED_METHODS.iter().enumerate() {
                            ui.selectable_value(&mut self.sec.shred_method, i, *label);
                        }
                    });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(
                        ui,
                        Kind::Danger,
                        "🔥 Permanently Shred Target",
                        idle,
                    )
                    .clicked()
                    {
                        self.shred_clicked();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_ROW);
        ui.label(egui::RichText::new(&self.sec.shred_status).color(theme::WARNING));
    }

    fn shred_clicked(&mut self) {
        let p = self.sec.shred_path.trim().to_string();
        if p.is_empty() {
            self.dialogs.push(Dialog::warn(
                "Notice",
                "Please select a target file or folder to shred.",
            ));
            return;
        }
        let target = sfo_core::paths::resolve(std::path::Path::new(&p));
        if !target.exists() {
            self.error(
                "Error",
                format!("Target path does not exist:\n{}", target.display()),
            );
            return;
        }
        let method = SHRED_METHODS[self.sec.shred_method].1;
        self.dialogs.push(Dialog::confirm_danger(
            "CRITICAL: Confirm Permanent Erasure",
            format!(
                "Are you ABSOLUTELY sure you want to shred:\n\n{}\n\nWiping Standard: {}\n\n\
                 This overwrites the file contents and deletes them. Data CANNOT be recovered!",
                target.display(),
                method.as_str().to_uppercase()
            ),
            Confirmed::Shred,
        ));
    }

    pub fn run_shred(&mut self) {
        let target = sfo_core::paths::resolve(std::path::Path::new(self.sec.shred_path.trim()));
        let method = SHRED_METHODS[self.sec.shred_method].1;
        self.spawn("Shredding...", move || {
            let audit = AuditLogChain::new(None).ok();
            let name = sfo_core::fsutil::name_of(&target);
            let res = if target.is_file() {
                match shredder::shred_file(&target, method, audit.as_ref(), None) {
                    Ok(r) if r.success => Ok(format!(
                        "Successfully shredded {name} ({} bytes) in {}s.",
                        r.original_size, r.duration_seconds
                    )),
                    Ok(r) => Err(r.error_message),
                    Err(e) => Err(e.to_string()),
                }
            } else {
                match shredder::shred_directory(&target, method, true, audit.as_ref(), None) {
                    Ok(b) if b.failed_files.is_empty() => Ok(format!(
                        "Shredded {} files in {}s.",
                        b.shredded_files.len(),
                        b.duration_seconds
                    )),
                    Ok(b) => Err(format!(
                        "Shredded {} files; {} failed:\n{}",
                        b.shredded_files.len(),
                        b.failed_files.len(),
                        b.failed_files
                            .iter()
                            .take(5)
                            .map(|(p, e)| format!("{}: {e}", p.display()))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )),
                    Err(e) => Err(e.to_string()),
                }
            };
            Done::Shredded(res)
        });
    }

    fn vault_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            ui.label(theme::header("Encrypted Local Vault (.sfovault)"));
            ui.label(theme::muted(
                "Locks files with Argon2id key derivation and XChaCha20-Poly1305 authenticated encryption. \
                 Older v1 vaults still open.",
            ));
            ui.add_space(theme::GAP_SECTION);
            ui.label(
                egui::RichText::new("[1] Lock File or Folder into Vault")
                    .font(theme::bold(theme::BODY)),
            );
            widgets::path_picker(
                ui,
                "Source to Lock:",
                &mut self.sec.vault_src,
                Pick::Either,
                130.0,
            );
            ui.horizontal(|ui| {
                widgets::field_label(ui, "Passphrase:", 130.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.sec.vault_pw)
                        .password(true)
                        .desired_width(220.0),
                );
                ui.checkbox(
                    &mut self.sec.vault_shred,
                    "Shred original after locking (DoD 3-pass)",
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Primary, "🔒 Lock into Vault", idle)
                        .clicked()
                    {
                        self.vault_lock();
                    }
                });
            });
            ui.separator();
            ui.label(
                egui::RichText::new("[2] Unlock and Extract Vault Container")
                    .font(theme::bold(theme::BODY)),
            );
            widgets::path_picker(
                ui,
                "Vault (.sfovault):",
                &mut self.sec.vault_file,
                Pick::File,
                130.0,
            );
            ui.horizontal(|ui| {
                widgets::field_label(ui, "Passphrase:", 130.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.sec.vault_unlock_pw)
                        .password(true)
                        .desired_width(220.0),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Primary, "🔓 Unlock Vault", idle).clicked()
                    {
                        self.vault_unlock();
                    }
                    if widgets::button_enabled(ui, Kind::Secondary, "📋 Inspect", idle).clicked()
                    {
                        self.vault_inspect();
                    }
                });
            });
            ui.add_space(theme::GAP_SECTION);
            ui.label(theme::muted(&self.sec.vault_status));
        });
    }

    fn vault_lock(&mut self) {
        let src = self.sec.vault_src.trim().to_string();
        let pw = self.sec.vault_pw.clone();
        if src.is_empty() || pw.is_empty() {
            self.dialogs.push(Dialog::warn(
                "Notice",
                "Please specify both source path and passphrase.",
            ));
            return;
        }
        let src = sfo_core::paths::resolve(std::path::Path::new(&src));
        if !src.exists() {
            self.error(
                "Error",
                format!("Source path does not exist:\n{}", src.display()),
            );
            return;
        }
        let default = src.with_extension(vault::EXTENSION);
        let Some(dst) = rfd::FileDialog::new()
            .set_title("Save Vault Container As")
            .set_directory(src.parent().unwrap_or(&src))
            .set_file_name(sfo_core::fsutil::name_of(&default))
            .add_filter("SFO Vault Container", &["sfovault"])
            .save_file()
        else {
            return;
        };
        let shred = self.sec.vault_shred;
        self.sec.vault_pw.clear();
        self.spawn("Encrypting vault...", move || {
            let audit = AuditLogChain::new(None).ok();
            Done::VaultLocked(
                vault::lock_path(&src, &dst, &pw, shred, KdfParams::default(), audit.as_ref())
                    .map(|p| (p, shred))
                    .map_err(|e| vault_msg(&e)),
            )
        });
    }

    fn vault_inputs(&mut self, what: &str) -> Option<(PathBuf, String)> {
        let v = self.sec.vault_file.trim().to_string();
        let pw = self.sec.vault_unlock_pw.clone();
        if v.is_empty() || pw.is_empty() {
            self.dialogs.push(Dialog::warn(
                "Notice",
                format!("Please specify both vault container and passphrase{what}."),
            ));
            return None;
        }
        let path = sfo_core::paths::resolve(std::path::Path::new(&v));
        if !path.exists() {
            self.error(
                "Error",
                format!("Vault file not found:\n{}", path.display()),
            );
            return None;
        }
        Some((path, pw))
    }

    fn vault_unlock(&mut self) {
        let Some((v, pw)) = self.vault_inputs("") else {
            return;
        };
        let Some(dest) = rfd::FileDialog::new()
            .set_title("Select Destination Folder for Unlocked Files")
            .set_directory(v.parent().unwrap_or(&v))
            .pick_folder()
        else {
            return;
        };
        self.sec.vault_unlock_pw.clear();
        self.spawn("Decrypting vault...", move || {
            let audit = AuditLogChain::new(None).ok();
            Done::VaultUnlocked(
                vault::unlock_vault(&v, &dest, &pw, audit.as_ref())
                    .map(|f| (f.len(), dest))
                    .map_err(|e| vault_msg(&e)),
            )
        });
    }

    fn vault_inspect(&mut self) {
        let Some((v, pw)) = self.vault_inputs(" to inspect") else {
            return;
        };
        self.spawn("Inspecting vault...", move || {
            Done::VaultInspected(
                vault::inspect_vault(&v, &pw)
                    .map(|m| (v, m))
                    .map_err(|e| vault_msg(&e)),
            )
        });
    }

    pub fn on_sync_done(&mut self, cloud_sync: bool, r: syncer::SyncResult) {
        let log = if cloud_sync {
            &mut self.sync.cloud_log
        } else {
            &mut self.sync.usb_log
        };
        log_sync(log, &r);
        self.status = r.message.clone();
        let title = if cloud_sync {
            "Cloud Sync Finished"
        } else {
            "Sync Finished"
        };
        if r.success {
            self.info(title, r.message)
        } else {
            self.error(title, r.message)
        }
    }

    pub fn on_cold(&mut self, files: Vec<sfo_core::models::FileInfo>, days: u64) {
        let total: u64 = files.iter().map(|f| f.size_bytes).sum();
        self.sync.cold_rows = files
            .iter()
            .take(300)
            .map(|f| {
                vec![
                    f.filename.clone(),
                    fb(f.size_bytes),
                    f.modified_time.format("%Y-%m-%d %H:%M").to_string(),
                    f.path.display().to_string(),
                ]
            })
            .collect();
        self.status = format!("Found {} cold files ({}).", files.len(), fb(total));
        if files.is_empty() {
            self.info(
                "Cold Storage",
                format!("No files inactive for >= {days} days."),
            );
        } else {
            self.info(
                "Cold Files Discovered",
                format!("Found {} cold files totaling {}.", files.len(), fb(total)),
            );
        }
        self.sync.cold_files = files;
    }
}
