//! Window shell (header, tab bar, status bar), shared state and background
//! jobs. Long operations run on a worker thread so the window never freezes
//! (the Tkinter build ran them on the UI thread).

use crate::dialog::{self, Confirmed, Dialog};
use crate::theme;
use egui::{Margin, RichText};
use sfo_core::config::Config;
use sfo_core::dedup::DedupAction;
use sfo_core::doctor::DoctorReport;
use sfo_core::i18n;
use sfo_core::junk::{CleanResult, JunkItem};
use sfo_core::lan_share::LanShareServer;
use sfo_core::models::{ActionRecord, DuplicateGroup, FileInfo, SessionLog, VisualDuplicateGroup};
use sfo_core::progress::Control;
use sfo_core::secrets::{QuarantineResult, SecretFinding};
use sfo_core::syncer::SyncResult;
use sfo_core::treemap::{DiskNode, LargeFile};
use sfo_core::undo::{RollbackResult, SessionSummary, UndoManager};
use sfo_core::vault::VaultMetadata;
use sfo_core::watcher::WatcherHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Which session a finished execution belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionKind {
    Organize,
    Dedupe,
    Rename,
}

/// Results sent back from worker threads.
pub enum Done {
    OrganizePreview {
        files: Vec<FileInfo>,
        records: Vec<ActionRecord>,
        then_execute: bool,
    },
    Session {
        kind: SessionKind,
        session: Box<SessionLog>,
        saved: Result<(), String>,
    },
    Duplicates {
        exact: Vec<DuplicateGroup>,
        visual: Vec<VisualDuplicateGroup>,
        visual_mode: bool,
    },
    RenamePreview {
        records: Vec<ActionRecord>,
        then_execute: bool,
    },
    Storage {
        root: Box<DiskNode>,
        largest: Vec<LargeFile>,
    },
    Junk(Vec<JunkItem>),
    JunkCleaned(CleanResult),
    Secrets(Vec<SecretFinding>),
    Quarantined(Result<QuarantineResult, String>),
    Shredded(Result<String, String>),
    VaultLocked(Result<(PathBuf, bool), String>),
    VaultUnlocked(Result<(usize, PathBuf), String>),
    VaultInspected(Result<(PathBuf, VaultMetadata), String>),
    UsbSynced(SyncResult),
    CloudSynced(SyncResult),
    Cold {
        files: Vec<FileInfo>,
        days: u64,
    },
    Archived {
        message: String,
        success: bool,
        dest: PathBuf,
    },
    Doctor(DoctorReport),
    Undone(Result<RollbackResult, String>),
    /// A cancellable scan was stopped before it finished.
    Cancelled,
}

#[derive(Default)]
pub struct OrganizeTab {
    pub source: String,
    pub dest: String,
    pub strategy: String,
    pub dry_run: bool,
    pub smart_receipts: bool,
    pub sub_date: bool,
    /// Include files in subfolders of the source (CLI `-r`).
    pub recursive: bool,
    pub files: usize,
    pub total_bytes: u64,
    pub planned_moves: usize,
    pub mode_label: String,
    pub records: Vec<ActionRecord>,
    pub rows: Vec<Vec<String>>,
    /// Running folder watcher, if any.
    pub watcher: Option<WatcherHandle>,
    /// "file -> folder" lines sent by the watcher thread.
    pub watch_rx: Option<Receiver<String>>,
    pub watch_log: Vec<String>,
    pub watch_count: usize,
    pub watch_target: String,
}

#[derive(Default)]
pub struct DedupeTab {
    pub folder: String,
    pub visual: bool,
    pub similarity: u32,
    pub keep: String,
    pub exact: Vec<DuplicateGroup>,
    pub visual_groups: Vec<VisualDuplicateGroup>,
    pub stats: (usize, usize, u64),
    pub rows: Vec<Vec<String>>,
}

#[derive(Default)]
pub struct RenameTab {
    pub folder: String,
    pub template: String,
    pub find: String,
    pub replace: String,
    pub case: String,
    pub records: Vec<ActionRecord>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Default)]
pub struct StorageTab {
    pub folder: String,
    pub summary: String,
    pub root: Option<DiskNode>,
    pub largest_rows: Vec<Vec<String>>,
}

#[derive(Default)]
pub struct SyncTab {
    pub sub: usize,
    pub usb_label: String,
    pub usb_source: String,
    pub usb_dest: String,
    pub usb_mode: String,
    pub usb_log: String,
    pub clouds: Vec<sfo_core::cloud::CloudProvider>,
    pub cloud_idx: usize,
    pub cloud_source: String,
    pub cloud_sub: String,
    pub cloud_log: String,
    pub lan_folder: String,
    pub lan_port: String,
    pub lan_pin: String,
    pub lan: Option<LanShareServer>,
    pub lan_url: String,
    pub cold_folder: String,
    pub cold_days: String,
    pub cold_delete: bool,
    pub cold_files: Vec<FileInfo>,
    pub cold_rows: Vec<Vec<String>>,
}

#[derive(Default)]
pub struct SecurityTab {
    pub sub: usize,
    pub junk_folder: String,
    pub junk_status: String,
    pub junk: Vec<JunkItem>,
    pub junk_rows: Vec<Vec<String>>,
    pub secret_folder: String,
    pub secret_status: String,
    pub secrets: Vec<SecretFinding>,
    pub secret_rows: Vec<Vec<String>>,
    pub shred_path: String,
    pub shred_method: usize,
    pub shred_status: String,
    pub vault_src: String,
    pub vault_pw: String,
    pub vault_shred: bool,
    pub vault_file: String,
    pub vault_unlock_pw: String,
    pub vault_status: String,
}

#[derive(Default)]
pub struct SystemTab {
    pub doctor_status: String,
    pub doctor_rows: Vec<Vec<String>>,
    pub sessions: Vec<SessionSummary>,
    pub history_rows: Vec<Vec<String>>,
    pub history_sel: Option<usize>,
    pub autostart: bool,
    pub shell_status: String,
    pub category_rows: Vec<Vec<String>>,
    /// Regex rules being edited: `(pattern, folder)`.
    pub rules: Vec<(String, String)>,
    pub rules_dirty: bool,
    pub rule_test: String,
}

/// The running background job.
pub struct Job {
    pub ctl: Arc<Control>,
    pub cancellable: bool,
}

pub struct App {
    pub tab: usize,
    pub status: String,
    pub busy: Option<String>,
    pub job: Option<Job>,
    pub dialogs: Vec<Dialog>,
    pub config: Config,
    pub undo: Option<UndoManager>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    pub org: OrganizeTab,
    pub dup: DedupeTab,
    pub ren: RenameTab,
    pub store: StorageTab,
    pub sync: SyncTab,
    pub sec: SecurityTab,
    pub sys: SystemTab,
    capture: Option<Capture>,
    /// Path returned by the next file dialog in tests.
    #[cfg(test)]
    pub test_pick: Option<PathBuf>,
}

fn home_sub(name: &str) -> String {
    dirs::home_dir()
        .map(|h| h.join(name).display().to_string())
        .unwrap_or_default()
}

impl App {
    pub fn new(ctx: &egui::Context, source: Option<String>) -> Self {
        theme::apply(ctx);
        let (tx, rx) = channel();
        let config = Config::load(None).unwrap_or_default();
        let undo = UndoManager::new(None).ok();
        let downloads = home_sub("Downloads");
        let documents = home_sub("Documents");
        let start = source.unwrap_or_else(|| downloads.clone());

        let mut app = Self {
            tab: 0,
            status: i18n::t("status_ready"),
            busy: None,
            job: None,
            dialogs: Vec::new(),
            config,
            undo,
            tx,
            rx,
            org: OrganizeTab {
                source: start.clone(),
                dest: start.clone(),
                strategy: "extension".into(),
                dry_run: true,
                smart_receipts: true,
                mode_label: "DRY RUN".into(),
                ..Default::default()
            },
            dup: DedupeTab { folder: start.clone(), similarity: 90, keep: "oldest".into(), ..Default::default() },
            ren: RenameTab { folder: start.clone(), template: "{counter:03d}_{name}".into(), case: "None".into(), ..Default::default() },
            store: StorageTab {
                folder: start.clone(),
                summary: "Click 'Analyze Storage' to compute space distribution.".into(),
                ..Default::default()
            },
            sync: SyncTab {
                usb_source: documents.clone(),
                usb_dest: "D:/Backup".into(),
                usb_mode: "incremental".into(),
                cloud_source: documents.clone(),
                cloud_sub: "SFO_Cloud_Sync".into(),
                lan_folder: start.clone(),
                lan_port: "8080".into(),
                cold_folder: start,
                cold_days: "90".into(),
                ..Default::default()
            },
            sec: SecurityTab {
                junk_folder: documents.clone(),
                junk_status: "Scan for node_modules, .venv, and build caches.".into(),
                secret_folder: documents,
                secret_status: "Scan for exposed API keys, tokens, and SSH private keys.".into(),
                shred_status: "WARNING: Shredded files CANNOT be recovered by any undelete or recovery software.".into(),
                vault_status: "Authenticated encryption. Forgotten passwords cannot be recovered.".into(),
                ..Default::default()
            },
            capture: None,
            #[cfg(test)]
            test_pick: None,
            sys: SystemTab { doctor_status: "System Status: Ready to diagnose".into(), ..Default::default() },
        };
        if app.undo.is_none() {
            app.dialogs.push(Dialog::error(
                "History unavailable",
                "Could not open the session history database. Undo is disabled.",
            ));
        }
        app.load_history();
        app.refresh_usb();
        app.refresh_clouds();
        app.refresh_settings();
        app.capture = Capture::from_env(&mut app);
        app
    }

    /// Runs `work` on a worker thread unless a job is already running.
    pub fn spawn(&mut self, label: &str, work: impl FnOnce(Arc<Control>) -> Done + Send + 'static) {
        self.spawn_job(label, false, work);
    }

    /// Like [`Self::spawn`], with a Cancel button in the status bar. A
    /// cancelled scan reports [`Done::Cancelled`]; a cancelled session still
    /// reports what it did, so it can be undone.
    pub fn spawn_cancellable(
        &mut self,
        label: &str,
        work: impl FnOnce(Arc<Control>) -> Done + Send + 'static,
    ) {
        self.spawn_job(label, true, work);
    }

    fn spawn_job(
        &mut self,
        label: &str,
        cancellable: bool,
        work: impl FnOnce(Arc<Control>) -> Done + Send + 'static,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(label.to_string());
        self.status = label.to_string();
        let ctl = Control::new();
        self.job = Some(Job {
            ctl: Arc::clone(&ctl),
            cancellable,
        });
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let done = work(Arc::clone(&ctl));
            let done = if cancellable && ctl.is_cancelled() && !matches!(done, Done::Session { .. })
            {
                Done::Cancelled
            } else {
                done
            };
            let _ = tx.send(done);
        });
    }

    /// Blast-radius warning for a live operation, from the config limits.
    pub fn blast_warning(&self, count: u64, bytes: u64) -> Option<String> {
        let (max_files, max_bytes) = self.config.blast_radius();
        sfo_core::guardrails::check_blast_radius(count, bytes, max_files, max_bytes)
    }

    /// Queues a confirmation; over the blast-radius limit it becomes a red
    /// "danger" confirmation that leads with the warning.
    #[allow(clippy::too_many_arguments)]
    pub fn confirm_live(
        &mut self,
        title: &str,
        body: String,
        action: Confirmed,
        danger: bool,
        count: u64,
        bytes: u64,
    ) {
        let dialog = match self.blast_warning(count, bytes) {
            Some(w) => Dialog::confirm_danger(title, format!("⚠ {w}\n\n{body}"), action),
            None if danger => Dialog::confirm_danger(title, body, action),
            None => Dialog::confirm(title, body, action),
        };
        self.dialogs.push(dialog);
    }

    pub fn idle(&self) -> bool {
        self.busy.is_none()
    }

    pub fn info(&mut self, title: &str, body: impl Into<String>) {
        self.dialogs.push(Dialog::info(title, body));
    }

    pub fn error(&mut self, title: &str, body: impl Into<String>) {
        self.dialogs.push(Dialog::error(title, body));
    }

    /// `Some(path)` when `p` is an existing folder, else an error dialog.
    pub fn existing_dir(&mut self, p: &str) -> Option<PathBuf> {
        let path = PathBuf::from(p.trim());
        if p.trim().is_empty() || !path.is_dir() {
            self.error(
                "Error",
                format!("Folder does not exist:\n{}", path.display()),
            );
            return None;
        }
        Some(path)
    }

    /// [`Self::existing_dir`] plus the guardrail the CLI applies: drive roots
    /// and system folders are refused for anything that moves or deletes files.
    pub fn allowed_dir(&mut self, p: &str) -> Option<PathBuf> {
        let dir = self.existing_dir(p)?;
        if let Some(reason) = sfo_core::guardrails::protected_reason(&dir) {
            self.error(
                &sfo_core::i18n::t("guardrail_alert"),
                format!(
                    "{}

{reason}",
                    sfo_core::i18n::t("protected_path_warning")
                ),
            );
            return None;
        }
        Some(dir)
    }

    /// Native "save as" dialog (replaced by `test_pick` in tests).
    pub fn choose_save(&mut self, dialog: rfd::FileDialog) -> Option<PathBuf> {
        #[cfg(test)]
        {
            let _ = dialog;
            self.test_pick.take()
        }
        #[cfg(not(test))]
        dialog.save_file()
    }

    /// Native folder dialog (replaced by `test_pick` in tests).
    pub fn choose_folder(&mut self, dialog: rfd::FileDialog) -> Option<PathBuf> {
        #[cfg(test)]
        {
            let _ = dialog;
            self.test_pick.take()
        }
        #[cfg(not(test))]
        dialog.pick_folder()
    }

    fn poll_jobs(&mut self) {
        while let Ok(done) = self.rx.try_recv() {
            self.busy = None;
            self.job = None;
            self.on_done(done);
        }
    }

    fn dispatch(&mut self, c: Confirmed) {
        match c {
            Confirmed::ExecuteOrganize => self.run_organize(),
            Confirmed::MoveDuplicates => self.run_move_duplicates(DedupAction::MoveToDuplicates),
            Confirmed::RenameFiles => self.run_rename(),
            Confirmed::CleanJunk => self.run_clean_junk(),
            Confirmed::QuarantineSecrets => self.run_quarantine(),
            Confirmed::Shred => self.run_shred(),
            Confirmed::ArchiveCold => self.run_archive(),
            Confirmed::Undo(id) => self.run_undo(id),
        }
    }

    /// Status text, job progress (bar when the total is known, spinner
    /// otherwise) and the Cancel button for cancellable jobs.
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let small = |t: &str| RichText::new(t).font(egui::FontId::proportional(theme::SMALL));
        ui.horizontal(|ui| {
            let Some(job) = &self.job else {
                ui.label(small(&self.status));
                return;
            };
            let v = job.ctl.view();
            ui.label(small(&self.status));
            if v.total > 0 {
                let frac = (v.done as f32 / v.total as f32).clamp(0.0, 1.0);
                ui.add(
                    egui::ProgressBar::new(frac)
                        .desired_width(260.0)
                        .fill(theme::ACCENT)
                        .text(small(&format!("{} {}/{}", v.stage, v.done, v.total))),
                );
            } else {
                ui.add(egui::Spinner::new().size(12.0).color(theme::ACCENT));
                if !v.stage.is_empty() {
                    ui.label(small(&format!("{} — {} items", v.stage, v.done)));
                }
            }
            if job.cancellable {
                let ctl = Arc::clone(&job.ctl);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ctl.is_cancelled() {
                        ui.label(theme::muted("Cancelling..."));
                    } else if crate::widgets::button(
                        ui,
                        crate::widgets::Kind::Secondary,
                        "✖ Cancel",
                    )
                    .clicked()
                    {
                        ctl.cancel();
                    }
                });
            }
        });
    }

    /// Drains watcher events into the log and refreshes history.
    fn poll_watcher(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.org.watch_rx else {
            return;
        };
        let mut got = Vec::new();
        while let Ok(line) = rx.try_recv() {
            got.push(line);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        if got.is_empty() {
            return;
        }
        self.org.watch_count += got.len();
        for line in got {
            self.org.watch_log.push(format!(
                "{}  {line}",
                chrono::Local::now().format("%H:%M:%S")
            ));
        }
        let extra = self.org.watch_log.len().saturating_sub(200);
        self.org.watch_log.drain(..extra);
        self.load_history();
    }

    fn header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(theme::title("📂 Smart File Organizer"));
            ui.add_space(16.0);
            ui.label(theme::muted(
                "Visual pHash • Renamer • Treemap • Cloud Sync • LAN Drop • Vault • 100% Rollback",
            ));
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let keys = [
            "tab_organize",
            "tab_dedupe",
            "tab_renamer",
            "tab_storage",
            "tab_sync",
            "tab_security",
            "tab_doctor",
            "tab_history",
            "tab_settings",
        ];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.spacing_mut().button_padding = egui::vec2(8.0, 6.0);
            for (i, k) in keys.iter().enumerate() {
                let on = self.tab == i;
                let text = RichText::new(crate::widgets::plain_emoji(&i18n::t(k))).color(if on {
                    theme::ACCENT
                } else {
                    theme::TEXT_MUTED
                });
                let b = egui::Button::new(text)
                    .fill(if on { theme::BG_DARK } else { theme::CARD_BG })
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        if on {
                            theme::ACCENT
                        } else {
                            theme::CARD_BORDER
                        },
                    ))
                    .min_size(egui::vec2(0.0, 32.0));
                if ui.add(b).clicked() {
                    self.tab = i;
                }
            }
        });
    }
}

/// Self-capture for visual checks without touching the user's desktop:
/// `SFO_GUI_SCREENSHOT=<png>` renders a few frames, saves the window to that
/// file and exits. `SFO_GUI_TAB=<n>[.<sub>]` picks the tab (and sub-tab).
pub struct Capture {
    path: std::path::PathBuf,
    frames: u32,
    requested: bool,
}

impl Capture {
    pub fn from_env(app: &mut App) -> Option<Self> {
        let path = std::env::var_os("SFO_GUI_SCREENSHOT")?;
        if let Ok(t) = std::env::var("SFO_GUI_TAB") {
            let mut parts = t.split('.');
            app.tab = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let sub = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            app.sync.sub = sub;
            app.sec.sub = sub;
        }
        Some(Self {
            path: path.into(),
            frames: 0,
            requested: false,
        })
    }

    fn step(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        ctx.request_repaint();
        if self.frames == 8 && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                let _ = buf.save(&self.path);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl App {
    /// One UI frame. eframe calls it through `update`; the headless tests
    /// drive it directly.
    pub fn frame(&mut self, ctx: &egui::Context) {
        // Closing mid-operation could cut a move short before its session is
        // journaled, leaving files that undo cannot find.
        if self.busy.is_some() && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self
                .dialogs
                .iter()
                .any(|d| d.title == "Operation in progress")
            {
                self.dialogs.push(Dialog::warn(
                    "Operation in progress",
                    "Please wait until the current operation finishes before closing Smart File Organizer.",
                ));
            }
        }
        if let Some(mut c) = self.capture.take() {
            c.step(ctx);
            self.capture = Some(c);
        }
        self.poll_jobs();
        self.poll_watcher(ctx);
        if self.busy.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_DARK)
                    .inner_margin(Margin {
                        left: 20,
                        right: 20,
                        top: 14,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| {
                self.header(ui);
                ui.add_space(8.0);
                self.tab_bar(ui);
            });

        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::CARD_BG)
                    .stroke(egui::Stroke::new(1.0_f32, theme::CARD_BORDER))
                    .inner_margin(Margin {
                        left: 20,
                        right: 20,
                        top: 6,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| self.status_bar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG_DARK)
                    .inner_margin(Margin::symmetric(20, theme::PAD_TAB)),
            )
            .show(ctx, |ui| match self.tab {
                0 => self.organize_ui(ui),
                1 => self.dedupe_ui(ui),
                2 => self.rename_ui(ui),
                3 => self.storage_ui(ui),
                4 => self.sync_ui(ui),
                5 => self.security_ui(ui),
                6 => self.doctor_ui(ui),
                7 => self.history_ui(ui),
                _ => self.settings_ui(ui),
            });

        if let Some(c) = dialog::show(ctx, &mut self.dialogs) {
            self.dispatch(c);
        }
    }
}

// The LAN server stops in `LanShareServer::drop` when the app closes.

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}
