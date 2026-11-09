//! Tabs 7–9 (Doctor, Undo & History, Settings) and job-result handling.

use crate::app::{App, Done, SessionKind};
use crate::dialog::{Confirmed, Dialog};
use crate::tabs_main::fb;
use crate::theme;
use crate::widgets::{self, Kind};
use egui::{Align, Layout, Ui};
use sfo_core::doctor;
use sfo_core::guardrails::AuditLogChain;
use sfo_core::i18n;
use sfo_core::platform::{self, ActionResult, shell, startup};

impl App {
    // -------------------------------------------------------------- Doctor
    pub fn doctor_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            ui.label(theme::header(&format!("🩺 {}", i18n::t("doctor_title"))));
            ui.label(theme::muted(&i18n::t("doctor_subtitle")));
            ui.add_space(theme::GAP_ROW);
            ui.horizontal(|ui| {
                ui.label(theme::status_title(&self.sys.doctor_status, theme::ACCENT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Primary, &i18n::t("run_diagnostics"), idle)
                        .clicked()
                    {
                        self.spawn("Running system self-diagnostics...", || {
                            Done::Doctor(doctor::run_full_diagnosis(None))
                        });
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::table(
            ui,
            "doctor",
            &["Diagnostic Item", "Result", "Evaluation & Details"],
            &self.sys.doctor_rows,
            None,
        );
    }

    // ------------------------------------------------------------- History
    pub fn load_history(&mut self) {
        let Some(undo) = &self.undo else { return };
        self.sys.sessions = undo.list_sessions(30).unwrap_or_default();
        self.sys.history_rows = self
            .sys
            .sessions
            .iter()
            .map(|s| {
                vec![
                    s.session_id.clone(),
                    s.start_time
                        .chars()
                        .take(19)
                        .collect::<String>()
                        .replace('T', " "),
                    s.strategy.clone(),
                    if s.dry_run { "Dry-Run" } else { "Live" }.into(),
                    s.moved_count.to_string(),
                    s.duplicate_count.to_string(),
                    s.status.clone(),
                ]
            })
            .collect();
        self.sys.history_sel = None;
    }

    pub fn history_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        ui.horizontal(|ui| {
            ui.label(theme::header("Recorded Organization Sessions"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, Kind::Secondary, &i18n::t("refresh")).clicked() {
                    self.load_history();
                }
                if widgets::button_enabled(
                    ui,
                    Kind::Danger,
                    &i18n::t("undo_session"),
                    idle && self.undo.is_some(),
                )
                .clicked()
                {
                    self.undo_clicked();
                }
            });
        });
        ui.add_space(theme::GAP_SECTION);
        let rows = self.sys.history_rows.clone();
        widgets::table(
            ui,
            "history",
            &[
                "Session ID",
                "Date & Time",
                "Strategy",
                "Mode",
                "Files Moved",
                "Dups",
                "Status",
            ],
            &rows,
            Some(&mut self.sys.history_sel),
        );
    }

    fn undo_clicked(&mut self) {
        let Some(s) = self
            .sys
            .history_sel
            .and_then(|i| self.sys.sessions.get(i))
            .cloned()
        else {
            self.info(
                "Select Session",
                "Please select a session from the list to undo.",
            );
            return;
        };
        if s.status == "ROLLED_BACK" {
            self.dialogs.push(Dialog::warn(
                "Notice",
                "This session has already been rolled back.",
            ));
            return;
        }
        if s.dry_run {
            self.info("Notice", "Cannot rollback a Dry-Run session.");
            return;
        }
        self.dialogs.push(Dialog::confirm(
            "Confirm Undo",
            format!(
                "Are you sure you want to rollback session:\n{}?\n\nAll moved files will be restored to their exact original locations!",
                s.session_id
            ),
            Confirmed::Undo(s.session_id),
        ));
    }

    pub fn run_undo(&mut self, id: String) {
        let undo = self.undo.clone();
        self.spawn("Rolling back session...", move || {
            Done::Undone(match undo {
                Some(u) => u.rollback_session(Some(&id)).map_err(|e| e.to_string()),
                None => Err("history database unavailable".into()),
            })
        });
    }

    // ------------------------------------------------------------ Settings
    pub fn refresh_settings(&mut self) {
        self.sys.autostart = startup::is_autostart_enabled();
        self.sys.shell_status = if !platform::is_supported() {
            "Shell integration: Windows only.".into()
        } else if shell::is_installed() {
            "✅ Status: Context menu is registered in Windows Explorer.".into()
        } else {
            "⚪ Status: Context menu is not registered.".into()
        };
        self.sys.category_rows = self
            .config
            .categories()
            .into_iter()
            .map(|(c, e)| vec![c, e.join(", ")])
            .collect();
    }

    fn report(&mut self, r: ActionResult) {
        if r.success {
            self.info("Success", r.message)
        } else {
            self.error("Error", r.message)
        }
    }

    pub fn settings_ui(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.label(theme::header("Enterprise System Preferences"));
            ui.horizontal(|ui| {
                ui.label(i18n::t("language_select"));
                let current = i18n::language();
                let label = |c: &str| {
                    if c == "vi" {
                        "Tiếng Việt (vi)"
                    } else {
                        "English (en)"
                    }
                };
                let mut chosen = current.clone();
                egui::ComboBox::from_id_salt("lang")
                    .selected_text(label(&current))
                    .width(150.0)
                    .show_ui(ui, |ui| {
                        for code in ["en", "vi"] {
                            ui.selectable_value(&mut chosen, code.to_string(), label(code));
                        }
                    });
                if chosen != current {
                    i18n::set_language(&chosen);
                    self.status = format!("Language set to: {}", label(&chosen));
                }
                ui.add_space(12.0);
                let before = self.sys.autostart;
                ui.add_enabled_ui(platform::is_supported(), |ui| {
                    ui.checkbox(&mut self.sys.autostart, i18n::t("autostart_label"))
                });
                if self.sys.autostart != before {
                    let r = if self.sys.autostart {
                        startup::enable_autostart(None)
                    } else {
                        startup::disable_autostart()
                    };
                    self.sys.autostart = startup::is_autostart_enabled();
                    self.info("Windows Startup", r.message);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, Kind::Secondary, "🔒 Verify SHA-256 Audit Log").clicked()
                    {
                        match AuditLogChain::new(None).map(|c| c.verify_integrity()) {
                            Ok(v) if v.valid => self.info(
                                "Audit Integrity Validated",
                                format!("{}\n\nThe SHA-256 chain shows no tampering.", v.message),
                            ),
                            Ok(v) => self.error("Integrity Violation", v.message),
                            Err(e) => self.error("Error", e.to_string()),
                        }
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::card(ui, |ui| {
            ui.label(theme::header(&i18n::t("context_menu_label")));
            ui.label(theme::muted(
                "Adds right-click shortcuts ('Smart Organize Here', 'Find Duplicates Here', 'Open in GUI') directly into Windows Explorer.",
            ));
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&self.sys.shell_status).font(theme::bold(theme::BODY)),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = platform::is_supported();
                    if widgets::button_enabled(ui, Kind::Danger, &i18n::t("remove_menu"), ok)
                        .clicked()
                    {
                        let r = shell::uninstall();
                        self.refresh_settings();
                        self.report(r);
                    }
                    if widgets::button_enabled(ui, Kind::Primary, &i18n::t("register_menu"), ok)
                        .clicked()
                    {
                        let r = shell::install(None);
                        self.refresh_settings();
                        self.report(r);
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::card(ui, |ui| {
            ui.label(theme::header("Category File Formats"));
            ui.label(theme::muted(&format!(
                "Extensions are mapped into category folders automatically.\nSettings are stored in {}.",
                sfo_core::config::Config::user_config_path().display()
            )));
        });
        ui.add_space(theme::GAP_ROW);
        let rows = self.sys.category_rows.clone();
        widgets::table(
            ui,
            "categories",
            &["Category Folder", "Supported File Extensions"],
            &rows,
            None,
        );
    }

    // --------------------------------------------------------- Job results
    pub fn on_done(&mut self, done: Done) {
        match done {
            Done::OrganizePreview {
                files,
                records,
                then_execute,
            } => self.on_organize_preview(files, records, then_execute),
            Done::Session {
                kind,
                session,
                saved,
            } => {
                self.load_history();
                if let Err(e) = saved {
                    self.dialogs.push(Dialog::warn(
                        "Undo unavailable",
                        format!("The session ran but could not be saved for undo:\n{e}"),
                    ));
                }
                let failed: Vec<String> = session
                    .records
                    .iter()
                    .filter(|r| r.status == sfo_core::models::RecordStatus::Failed)
                    .take(5)
                    .map(|r| r.message.clone())
                    .collect();
                let failures = if failed.is_empty() {
                    String::new()
                } else {
                    format!("\n\nFailed:\n{}", failed.join("\n"))
                };
                match kind {
                    SessionKind::Organize => {
                        self.org.rows = session
                            .records
                            .iter()
                            .take(300)
                            .map(|r| {
                                let sub = r
                                    .destination_path
                                    .as_deref()
                                    .and_then(|p| p.parent())
                                    .map(sfo_core::fsutil::name_of)
                                    .unwrap_or_else(|| "-".into());
                                vec![
                                    crate::tabs_main::action_str(r),
                                    sfo_core::fsutil::name_of(&r.source_path),
                                    sub,
                                    fb(r.file_size),
                                    crate::tabs_main::status_str(r),
                                ]
                            })
                            .collect();
                        let mode = if session.dry_run {
                            "Dry-Run Simulation"
                        } else {
                            "Live Cleanup"
                        };
                        self.info(
                            "Session Finished",
                            format!(
                                "{mode} Complete!\n\nFiles Organized: {}\nFiles Skipped: {}\nSession ID: {}{failures}",
                                session.moved_count, session.skipped_count, session.session_id
                            ),
                        );
                    }
                    SessionKind::Dedupe => {
                        self.info(
                            "Complete",
                            format!(
                                "Successfully moved {} duplicates!{failures}",
                                session.duplicate_count
                            ),
                        );
                        self.scan_duplicates();
                    }
                    SessionKind::Rename => {
                        self.info(
                            "Rename Complete",
                            format!(
                                "Successfully renamed {} files! (Undoable anytime){failures}",
                                session.moved_count
                            ),
                        );
                        self.preview_rename(false);
                    }
                }
                if self.busy.is_none() {
                    self.status = format!(
                        "Session {} finished ({} moved).",
                        session.session_id, session.moved_count
                    );
                }
            }
            Done::Duplicates {
                exact,
                visual,
                visual_mode,
            } => self.on_duplicates(exact, visual, visual_mode),
            Done::RenamePreview {
                records,
                then_execute,
            } => self.on_rename_preview(records, then_execute),
            Done::Storage { root, largest } => self.on_storage(*root, largest),
            Done::Junk(items) => {
                let total: u64 = items.iter().map(|i| i.size_bytes).sum();
                self.sec.junk_rows = items
                    .iter()
                    .map(|i| {
                        vec![
                            i.category.clone(),
                            i.name.clone(),
                            fb(i.size_bytes),
                            i.path.display().to_string(),
                        ]
                    })
                    .collect();
                self.sec.junk_status = format!(
                    "Found {} junk items ({} reclaimable).",
                    items.len(),
                    fb(total)
                );
                self.status = format!("Discovered {} junk items.", items.len());
                self.sec.junk = items;
            }
            Done::JunkCleaned(r) => {
                let body = if r.errors.is_empty() {
                    r.message.clone()
                } else {
                    format!("{}\n\n{}", r.message, r.errors.join("\n"))
                };
                self.info("Cleanup Finished", body);
                self.scan_junk();
            }
            Done::Secrets(found) => {
                self.sec.secret_rows = found
                    .iter()
                    .map(|f| {
                        vec![
                            f.severity.clone(),
                            f.secret_type.clone(),
                            sfo_core::fsutil::name_of(&f.file_path),
                            f.line_number.to_string(),
                            f.masked_snippet.clone(),
                        ]
                    })
                    .collect();
                if found.is_empty() {
                    self.sec.secret_status = "Clean! No exposed secrets detected.".into();
                    self.info("Clean Scan", "No sensitive tokens or secrets were found.");
                } else {
                    self.sec.secret_status =
                        format!("ALERT: Found {} exposed secrets!", found.len());
                    self.dialogs.push(Dialog::warn(
                        "Security Alert",
                        format!("Detected {} sensitive credentials!", found.len()),
                    ));
                }
                self.status = self.sec.secret_status.clone();
                self.sec.secrets = found;
            }
            Done::Quarantined(r) => match r {
                Ok(q) => {
                    self.info("Quarantine Finished", q.message);
                    self.scan_secrets();
                }
                Err(e) => self.error("Error", e),
            },
            Done::Shredded(r) => match r {
                Ok(msg) => {
                    self.sec.shred_status = msg.clone();
                    self.sec.shred_path.clear();
                    self.info("Erasure Complete", msg);
                }
                Err(e) => self.error("Shred Failed", e),
            },
            Done::VaultLocked(r) => match r {
                Ok((path, shredded)) => {
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    self.sec.vault_status = format!(
                        "Vault locked: {} ({})",
                        sfo_core::fsutil::name_of(&path),
                        fb(size)
                    );
                    self.sec.vault_src.clear();
                    let extra = if shredded {
                        "\n\nOriginal source files were securely shredded."
                    } else {
                        ""
                    };
                    self.info(
                        "Vault Locked",
                        format!(
                            "Vault successfully created:\n{}\nSize: {}{extra}",
                            path.display(),
                            fb(size)
                        ),
                    );
                }
                Err(e) => self.error("Vault Error", format!("Failed to lock vault:\n{e}")),
            },
            Done::VaultUnlocked(r) => match r {
                Ok((n, dest)) => {
                    self.sec.vault_status = format!("Unlocked {n} files into: {}", dest.display());
                    self.info(
                        "Vault Unlocked",
                        format!(
                            "Successfully decrypted and extracted {n} file(s) into:\n{}",
                            dest.display()
                        ),
                    );
                }
                Err(e) => self.error("Authentication Failed", e),
            },
            Done::VaultInspected(r) => match r {
                Ok((path, m)) => {
                    let mut lines = vec![
                        format!(
                            "Vault Container: {} (format v{})",
                            sfo_core::fsutil::name_of(&path),
                            m.created_version
                        ),
                        format!("File Count: {}", m.file_count),
                        format!("Uncompressed Payload: {}", fb(m.total_uncompressed_bytes)),
                        String::new(),
                        "Files contained:".into(),
                    ];
                    lines.extend(
                        m.files
                            .iter()
                            .take(20)
                            .map(|(n, s, _)| format!("  • {n} ({})", fb(*s))),
                    );
                    if m.files.len() > 20 {
                        lines.push(format!("  ... and {} more files.", m.files.len() - 20));
                    }
                    if m.created_version == 1 {
                        lines.push(
                            "\nLegacy v1 vault: re-encrypt it with `sfo --vault-upgrade`.".into(),
                        );
                    }
                    self.info("Vault Inspection", lines.join("\n"));
                }
                Err(e) => self.error("Authentication Failed", e),
            },
            Done::UsbSynced(r) => self.on_sync_done(false, r),
            Done::CloudSynced(r) => self.on_sync_done(true, r),
            Done::Cold { files, days } => self.on_cold(files, days),
            Done::Archived {
                message,
                success,
                dest,
            } => {
                if success {
                    self.info(
                        "Archiving Complete",
                        format!("{message}\nSaved at: {}", dest.display()),
                    );
                    self.scan_cold();
                } else {
                    self.error("Error", message);
                }
            }
            Done::Doctor(r) => {
                self.sys.doctor_rows = r
                    .checks
                    .iter()
                    .map(|c| vec![c.name.clone(), c.status.as_str().into(), c.message.clone()])
                    .collect();
                self.sys.doctor_status = format!(
                    "{} {} ({} PASS, {} WARN, {} FAIL)",
                    i18n::t("overall_health"),
                    r.overall_status(),
                    r.pass_count(),
                    r.warn_count(),
                    r.fail_count()
                );
                self.status = "Doctor diagnosis complete.".into();
            }
            Done::Undone(r) => match r {
                Ok(res) if res.success => {
                    let extra = if res.errors.is_empty() {
                        String::new()
                    } else {
                        format!("\n\n{}", res.errors.join("\n"))
                    };
                    self.info(
                        "Rollback Successful",
                        format!(
                            "Restored {} files back to their original paths!{extra}",
                            res.restored_count
                        ),
                    );
                    self.load_history();
                    self.status = res.message;
                }
                Ok(res) => self.error("Rollback Error", res.message),
                Err(e) => self.error("Rollback Error", e),
            },
        }
        if self.busy.is_none() && self.status.ends_with("...") {
            self.status = i18n::t("status_ready");
        }
    }
}
