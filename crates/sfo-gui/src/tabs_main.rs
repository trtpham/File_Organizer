//! Tabs 1–4: Organize, Duplicate Center, Smart Renamer, Storage Treemap.

use crate::app::{App, Done, SessionKind};
use crate::dialog::{Confirmed, Dialog};
use crate::theme;
use crate::widgets::{self, Kind};
use egui::{Align, Layout, Ui};
use sfo_core::dedup::{DedupAction, Deduplicator};
use sfo_core::executor::TransactionExecutor;
use sfo_core::models::{ActionRecord, ActionType, OrganizeStrategy};
use sfo_core::organizer::{Organizer, PlanOptions};
use sfo_core::paths;
use sfo_core::renamer::{self, RenameOptions};
use sfo_core::reporter::format_bytes;
use sfo_core::scanner::{FileScanner, ScanOptions};
use sfo_core::treemap;
use std::path::PathBuf;

pub fn fb(n: u64) -> String {
    format_bytes(n as f64)
}

pub fn action_str(r: &ActionRecord) -> String {
    serde_json::to_value(r.action_type)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

pub fn status_str(r: &ActionRecord) -> String {
    serde_json::to_value(r.status)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn name(p: &std::path::Path) -> String {
    sfo_core::fsutil::name_of(p)
}

fn record_rows(records: &[ActionRecord]) -> Vec<Vec<String>> {
    records
        .iter()
        .take(300)
        .map(|r| {
            let sub = r
                .destination_path
                .as_deref()
                .and_then(|p| p.parent())
                .map(name)
                .unwrap_or_else(|| "-".into());
            vec![
                action_str(r),
                name(&r.source_path),
                sub,
                fb(r.file_size),
                status_str(r),
            ]
        })
        .collect()
}

pub fn combo(ui: &mut Ui, id: &str, value: &mut String, options: &[&str], width: f32) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.as_str())
        .width(width)
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(value, (*o).to_string(), *o);
            }
        });
}

impl App {
    // ---------------------------------------------------------------- Organize
    pub fn organize_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(
                ui,
                sfo_core::i18n::t("source_folder").trim(),
                &mut self.org.source,
            );
            widgets::folder_picker(
                ui,
                sfo_core::i18n::t("dest_folder").trim(),
                &mut self.org.dest,
            );
            ui.horizontal(|ui| {
                ui.label(sfo_core::i18n::t("strategy"));
                combo(
                    ui,
                    "org_strategy",
                    &mut self.org.strategy,
                    &["extension", "date", "size", "regex"],
                    110.0,
                );
                ui.checkbox(&mut self.org.dry_run, sfo_core::i18n::t("dry_run"));
                ui.checkbox(
                    &mut self.org.smart_receipts,
                    sfo_core::i18n::t("smart_receipts"),
                );
                ui.checkbox(&mut self.org.sub_date, "Sub-date");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(
                        ui,
                        Kind::Secondary,
                        &sfo_core::i18n::t("preview_actions"),
                        idle,
                    )
                    .clicked()
                    {
                        self.preview_organize(false);
                    }
                    if widgets::button_enabled(
                        ui,
                        Kind::Primary,
                        &sfo_core::i18n::t("execute_now"),
                        idle,
                    )
                    .clicked()
                    {
                        self.execute_organize_clicked();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::stat_row(
            ui,
            &[
                ("Files Scanned", self.org.files.to_string(), None),
                ("Total Size", fb(self.org.total_bytes), None),
                (
                    "Planned Moves",
                    self.org.planned_moves.to_string(),
                    Some(theme::ACCENT),
                ),
                (
                    "Current Mode",
                    self.org.mode_label.clone(),
                    Some(theme::WARNING),
                ),
            ],
        );
        ui.add_space(theme::GAP_ROW);
        widgets::table(
            ui,
            "org_table",
            &["Action", "File Name", "Target Folder", "Size", "Status"],
            &self.org.rows,
            None,
        );
    }

    pub fn preview_organize(&mut self, then_execute: bool) {
        let Some(source) = self.allowed_dir(&self.org.source.clone()) else {
            return;
        };
        // An empty destination means "same as source", as in the CLI.
        if self.org.dest.trim().is_empty() {
            self.org.dest = self.org.source.clone();
        }
        let dest = PathBuf::from(self.org.dest.trim());
        if let Some(reason) = sfo_core::guardrails::protected_reason(&dest) {
            self.error(
                &sfo_core::i18n::t("guardrail_alert"),
                format!(
                    "{}

{reason}",
                    sfo_core::i18n::t("protected_path_warning")
                ),
            );
            return;
        }
        let config = self.config.clone();
        let strategy = OrganizeStrategy::parse(&self.org.strategy).unwrap_or_default();
        let opts = PlanOptions {
            sub_organize_by_date: self.org.sub_date,
            smart_receipts: self.org.smart_receipts,
        };
        self.spawn("Scanning directory files...", move || {
            let exclude = if paths::path_eq(&paths::resolve(&dest), &paths::resolve(&source)) {
                vec![]
            } else {
                vec![dest.clone()]
            };
            let scan = ScanOptions {
                recursive: false,
                exclude_dirs: exclude,
                max_depth: None,
            };
            let files = FileScanner::new(&config)
                .scan(&source, &scan, None)
                .unwrap_or_default();
            let records = Organizer::new(&config).plan(&files, &dest, strategy, opts);
            Done::OrganizePreview {
                files,
                records,
                then_execute,
            }
        });
    }

    fn execute_organize_clicked(&mut self) {
        if self.org.records.is_empty() {
            self.preview_organize(true);
        } else {
            self.confirm_organize();
        }
    }

    pub fn confirm_organize(&mut self) {
        if self.org.dry_run {
            self.run_organize();
            return;
        }
        self.dialogs.push(Dialog::confirm(
            "Confirm Execution",
            format!(
                "Are you sure you want to organize {} files in LIVE mode?\n\nSource: {}\nDestination: {}\n\n\
                 (Note: You can undo this operation anytime in the Undo tab!)",
                self.org.records.len(),
                self.org.source,
                self.org.dest
            ),
            Confirmed::ExecuteOrganize,
        ));
    }

    pub fn run_organize(&mut self) {
        let records = std::mem::take(&mut self.org.records);
        let (source, dest) = (
            PathBuf::from(&self.org.source),
            PathBuf::from(&self.org.dest),
        );
        let (config, undo, dry) = (self.config.clone(), self.undo.clone(), self.org.dry_run);
        let strategy = self.org.strategy.clone();
        self.spawn("Executing file operations...", move || {
            let session = TransactionExecutor::new(&config)
                .execute_session(records, &source, &dest, &strategy, dry, None);
            let saved = save(undo.as_ref(), &session);
            Done::Session {
                kind: SessionKind::Organize,
                session: Box::new(session),
                saved,
            }
        });
    }

    pub fn on_organize_preview(
        &mut self,
        files: Vec<sfo_core::models::FileInfo>,
        records: Vec<ActionRecord>,
        then_execute: bool,
    ) {
        if files.is_empty() {
            self.org.records.clear();
            self.info("Result", "No files found to organize.");
            self.status = "No files found to organize.".into();
            return;
        }
        let moves = records
            .iter()
            .filter(|r| r.action_type == ActionType::Move)
            .count();
        self.org.files = files.len();
        self.org.total_bytes = files.iter().map(|f| f.size_bytes).sum();
        self.org.planned_moves = moves;
        self.org.mode_label = if self.org.dry_run { "DRY RUN" } else { "LIVE" }.into();
        self.org.rows = record_rows(&records);
        self.org.records = records;
        self.status = format!(
            "Preview ready: {moves} planned moves out of {} files.",
            files.len()
        );
        if then_execute {
            self.confirm_organize();
        }
    }

    // --------------------------------------------------------------- Dedupe
    pub fn dedupe_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(ui, "Target Folder:", &mut self.dup.folder);
            ui.horizontal(|ui| {
                ui.label("Deduplication Mode:");
                let mut mode = if self.dup.visual {
                    "visual_phash"
                } else {
                    "exact"
                }
                .to_string();
                combo(ui, "dup_mode", &mut mode, &["exact", "visual_phash"], 120.0);
                self.dup.visual = mode == "visual_phash";
                ui.label("Similarity:");
                let mut sim = format!("{}%", self.dup.similarity);
                combo(
                    ui,
                    "dup_sim",
                    &mut sim,
                    &["80%", "85%", "90%", "95%", "100%"],
                    60.0,
                );
                self.dup.similarity = sim.trim_end_matches('%').parse().unwrap_or(90);
                ui.label("Keep:");
                combo(
                    ui,
                    "dup_keep",
                    &mut self.dup.keep,
                    &["oldest", "newest", "shortest_path"],
                    110.0,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Secondary, "🔍 Scan Duplicates", idle)
                        .clicked()
                    {
                        self.scan_duplicates();
                    }
                    if widgets::button_enabled(ui, Kind::Warning, "📦 Move Duplicates", idle)
                        .clicked()
                    {
                        self.move_duplicates_clicked();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        let (groups, files, wasted) = self.dup.stats;
        widgets::stat_row(
            ui,
            &[
                ("Duplicate Clusters", groups.to_string(), None),
                ("Redundant Files", files.to_string(), Some(theme::WARNING)),
                ("Wasted Disk Space", fb(wasted), Some(theme::DANGER)),
            ],
        );
        ui.add_space(theme::GAP_ROW);
        widgets::table(
            ui,
            "dup_table",
            &["Status", "File Name", "Size", "Folder", "Hash / Similarity"],
            &self.dup.rows,
            None,
        );
    }

    pub fn scan_duplicates(&mut self) {
        let Some(dir) = self.allowed_dir(&self.dup.folder.clone()) else {
            return;
        };
        let mut config = self.config.clone();
        let mut dedup = config.deduplication_settings();
        dedup.insert("keep_policy".into(), self.dup.keep.clone().into());
        config.set("deduplication", serde_json::Value::Object(dedup));
        let (visual, sim) = (self.dup.visual, f64::from(self.dup.similarity) / 100.0);
        self.spawn("Scanning for duplicates...", move || {
            let opts = ScanOptions {
                recursive: true,
                ..Default::default()
            };
            let files = FileScanner::new(&config)
                .scan(&dir, &opts, None)
                .unwrap_or_default();
            let dd = Deduplicator::new(&config);
            if visual {
                Done::Duplicates {
                    exact: vec![],
                    visual: dd.find_visual_duplicates(&files, sim),
                    visual_mode: true,
                }
            } else {
                Done::Duplicates {
                    exact: dd.find_duplicates(&files, None),
                    visual: vec![],
                    visual_mode: false,
                }
            }
        });
    }

    pub fn on_duplicates(
        &mut self,
        exact: Vec<sfo_core::models::DuplicateGroup>,
        visual: Vec<sfo_core::models::VisualDuplicateGroup>,
        visual_mode: bool,
    ) {
        let mut rows = Vec::new();
        if visual_mode {
            let redundant: usize = visual.iter().map(|g| g.files.len() - 1).sum();
            let wasted: u64 = visual
                .iter()
                .flat_map(|g| {
                    g.files
                        .iter()
                        .filter(move |f| g.suggested_keep.as_ref().is_none_or(|k| k.path != f.path))
                })
                .map(|f| f.size_bytes)
                .sum();
            for g in &visual {
                for f in &g.files {
                    let keep = g.suggested_keep.as_ref().is_some_and(|k| k.path == f.path);
                    rows.push(vec![
                        if keep {
                            "BEST RESOLUTION (Keep)"
                        } else {
                            "VISUAL DUPLICATE"
                        }
                        .into(),
                        f.filename.clone(),
                        fb(f.size_bytes),
                        f.path.parent().map(name).unwrap_or_default(),
                        if keep {
                            "100% (Original)".into()
                        } else {
                            format!("{:.0}% Similarity", g.similarity_score * 100.0)
                        },
                    ]);
                }
            }
            self.dup.stats = (visual.len(), redundant, wasted);
        } else {
            let redundant: usize = exact.iter().map(|g| g.files.len() - 1).sum();
            let wasted: u64 = exact.iter().map(|g| g.wasted_space_bytes()).sum();
            for g in &exact {
                for f in &g.files {
                    let keep = g.suggested_keep.as_ref().is_some_and(|k| k.path == f.path);
                    rows.push(vec![
                        if keep { "ORIGINAL (Keep)" } else { "DUPLICATE" }.into(),
                        f.filename.clone(),
                        fb(f.size_bytes),
                        f.path.parent().map(name).unwrap_or_default(),
                        format!("{}...", &g.file_hash[..16.min(g.file_hash.len())]),
                    ]);
                }
            }
            self.dup.stats = (exact.len(), redundant, wasted);
        }
        let n = if visual_mode {
            visual.len()
        } else {
            exact.len()
        };
        self.dup.rows = rows;
        self.dup.exact = exact;
        self.dup.visual_groups = visual;
        if n == 0 {
            self.info("Duplicates Scan", "No duplicate files were detected.");
            self.status = "Scan complete: No duplicate files found.".into();
        } else {
            self.status = format!("Found {n} duplicate clusters.");
        }
    }

    fn dedupe_records(&self, mode: DedupAction) -> Vec<ActionRecord> {
        let dir = PathBuf::from(self.dup.folder.trim());
        let dd = Deduplicator::new(&self.config);
        if self.dup.visual {
            dd.plan_visual_deduplication_actions(&self.dup.visual_groups, &dir, mode)
        } else {
            dd.plan_deduplication_actions(&self.dup.exact, &dir, mode)
        }
    }

    fn move_duplicates_clicked(&mut self) {
        let empty = if self.dup.visual {
            self.dup.visual_groups.is_empty()
        } else {
            self.dup.exact.is_empty()
        };
        if empty {
            let what = if self.dup.visual {
                "No visual duplicates to move."
            } else {
                "No duplicates to move."
            };
            self.info("Notice", what);
            return;
        }
        let n = self.dedupe_records(DedupAction::MoveToDuplicates).len();
        self.dialogs.push(Dialog::confirm(
            "Confirm Move",
            format!("Move {n} duplicate files to '_Duplicates' folder?"),
            Confirmed::MoveDuplicates,
        ));
    }

    pub fn run_move_duplicates(&mut self, mode: DedupAction) {
        let records = self.dedupe_records(mode);
        let dir = PathBuf::from(self.dup.folder.trim());
        let (config, undo) = (self.config.clone(), self.undo.clone());
        self.spawn("Moving duplicates...", move || {
            let session = TransactionExecutor::new(&config).execute_session(
                records,
                &dir,
                &dir,
                "deduplication",
                false,
                None,
            );
            let saved = save(undo.as_ref(), &session);
            Done::Session {
                kind: SessionKind::Dedupe,
                session: Box::new(session),
                saved,
            }
        });
    }

    // --------------------------------------------------------------- Rename
    pub fn rename_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(
                ui,
                sfo_core::i18n::t("source_folder").trim(),
                &mut self.ren.folder,
            );
            ui.horizontal(|ui| {
                ui.label("Template:");
                ui.add_sized(
                    [260.0, theme::BUTTON_HEIGHT],
                    egui::TextEdit::singleline(&mut self.ren.template),
                );
                ui.label(theme::muted(
                    "Tokens: {name}, {counter:03d}, {date}, {time}, {camera}, {artist}, {title}",
                ));
            });
            ui.horizontal(|ui| {
                ui.label("Find:");
                ui.add_sized(
                    [120.0, theme::BUTTON_HEIGHT],
                    egui::TextEdit::singleline(&mut self.ren.find),
                );
                ui.label("Replace:");
                ui.add_sized(
                    [120.0, theme::BUTTON_HEIGHT],
                    egui::TextEdit::singleline(&mut self.ren.replace),
                );
                ui.label("Casing:");
                combo(
                    ui,
                    "ren_case",
                    &mut self.ren.case,
                    &[
                        "None",
                        "lowercase",
                        "uppercase",
                        "title_case",
                        "snake_case",
                        "kebab_case",
                        "slugify",
                    ],
                    110.0,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Secondary, "🔍 Preview Rename", idle)
                        .clicked()
                    {
                        self.preview_rename(false);
                    }
                    if widgets::button_enabled(ui, Kind::Primary, "🏷️ Rename Files", idle).clicked()
                    {
                        if self.ren.records.is_empty() {
                            self.preview_rename(true);
                        } else {
                            self.confirm_rename();
                        }
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        widgets::table(
            ui,
            "ren_table",
            &["Action", "Original Filename", "New Filename", "Status"],
            &self.ren.rows,
            None,
        );
    }

    pub fn preview_rename(&mut self, then_execute: bool) {
        let Some(dir) = self.allowed_dir(&self.ren.folder.clone()) else {
            return;
        };
        let config = self.config.clone();
        let opts = RenameOptions {
            template: self.ren.template.clone(),
            find_text: self.ren.find.clone(),
            replace_text: self.ren.replace.clone(),
            case_transform: (self.ren.case != "None").then(|| self.ren.case.clone()),
            ..Default::default()
        };
        self.spawn("Planning rename...", move || {
            let files = FileScanner::new(&config).scan_dir(&dir).unwrap_or_default();
            Done::RenamePreview {
                records: renamer::plan_rename(&files, &opts),
                then_execute,
            }
        });
    }

    pub fn on_rename_preview(&mut self, records: Vec<ActionRecord>, then_execute: bool) {
        self.ren.rows = records
            .iter()
            .take(300)
            .map(|r| {
                let target = r
                    .destination_path
                    .as_deref()
                    .map(name)
                    .unwrap_or_else(|| "-".into());
                vec![action_str(r), name(&r.source_path), target, status_str(r)]
            })
            .collect();
        self.status = format!("Rename preview: {} files.", records.len());
        self.ren.records = records;
        if then_execute && !self.ren.records.is_empty() {
            self.confirm_rename();
        }
    }

    fn confirm_rename(&mut self) {
        self.dialogs.push(Dialog::confirm(
            "Confirm Rename",
            format!(
                "Are you sure you want to rename {} files?",
                self.ren.records.len()
            ),
            Confirmed::RenameFiles,
        ));
    }

    pub fn run_rename(&mut self) {
        let records = std::mem::take(&mut self.ren.records);
        let dir = PathBuf::from(self.ren.folder.trim());
        let (config, undo) = (self.config.clone(), self.undo.clone());
        self.spawn("Renaming files...", move || {
            let session = TransactionExecutor::new(&config)
                .execute_session(records, &dir, &dir, "rename", false, None);
            let saved = save(undo.as_ref(), &session);
            Done::Session {
                kind: SessionKind::Rename,
                session: Box::new(session),
                saved,
            }
        });
    }

    // -------------------------------------------------------------- Storage
    pub fn storage_ui(&mut self, ui: &mut Ui) {
        let idle = self.idle();
        widgets::card(ui, |ui| {
            widgets::folder_picker(ui, "Analyze Folder:", &mut self.store.folder);
            ui.horizontal(|ui| {
                ui.label(theme::muted(&self.store.summary));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_enabled(ui, Kind::Primary, "📊 Analyze Storage", idle)
                        .clicked()
                    {
                        self.analyze_storage();
                    }
                });
            });
        });
        ui.add_space(theme::GAP_SECTION);
        let h = ui.available_height();
        ui.columns(2, |cols| {
            cols[0].set_min_height(h);
            widgets::card(&mut cols[0], |ui| {
                ui.label(theme::header("Storage Treemap Distribution"));
                let size = egui::vec2(ui.available_width(), (h - 60.0).max(160.0));
                let (resp, painter) = ui.allocate_painter(size, egui::Sense::hover());
                painter.rect_filled(resp.rect, 0.0, theme::CARD_BG);
                if let Some(root) = self.store.root.as_mut() {
                    let r = resp.rect.shrink(10.0);
                    treemap::layout_treemap(
                        root,
                        f64::from(r.min.x),
                        f64::from(r.min.y),
                        f64::from(r.max.x),
                        f64::from(r.max.y),
                        0,
                        true,
                    );
                    for c in &root.children {
                        let (x1, y1, x2, y2) = c.rect;
                        let rect = egui::Rect::from_min_max(
                            egui::pos2(x1 as f32, y1 as f32),
                            egui::pos2(x2 as f32, y2 as f32),
                        );
                        if rect.width() > 2.0 && rect.height() > 2.0 {
                            let color = egui::Color32::from_hex(c.color).unwrap_or(theme::ACCENT);
                            painter.rect_filled(rect, 0.0, color);
                            painter.rect_stroke(
                                rect,
                                0.0,
                                egui::Stroke::new(2.0_f32, theme::BG_DARK),
                                egui::StrokeKind::Inside,
                            );
                            if rect.width() > 40.0 && rect.height() > 20.0 {
                                let label: String = c.name.chars().take(12).collect();
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{label}\n{}", fb(c.size)),
                                    theme::bold(theme::SMALL),
                                    theme::ON_LIGHT,
                                );
                            }
                            if resp.hover_pos().is_some_and(|p| rect.contains(p)) {
                                resp.clone().on_hover_text(format!(
                                    "{}\n{}",
                                    c.path.display(),
                                    fb(c.size)
                                ));
                            }
                        }
                    }
                }
            });
            widgets::card(&mut cols[1], |ui| {
                ui.label(theme::header("Top Storage-Eating Files"));
                widgets::table(
                    ui,
                    "largest",
                    &["File Name", "Size"],
                    &self.store.largest_rows,
                    None,
                );
            });
        });
    }

    fn analyze_storage(&mut self) {
        let Some(dir) = self.existing_dir(&self.store.folder.clone()) else {
            return;
        };
        self.spawn("Analyzing disk storage...", move || {
            let root = treemap::analyze_directory(&dir, 2);
            let largest = treemap::find_largest_files(&dir, 15);
            Done::Storage {
                root: Box::new(root),
                largest,
            }
        });
    }

    pub fn on_storage(
        &mut self,
        root: sfo_core::treemap::DiskNode,
        largest: Vec<sfo_core::treemap::LargeFile>,
    ) {
        self.store.summary = format!(
            "Total Storage: {} ({} direct subfolders/files)",
            fb(root.size),
            root.children.len()
        );
        self.store.largest_rows = largest
            .iter()
            .map(|f| vec![f.name.clone(), fb(f.size)])
            .collect();
        self.status = format!("Storage analysis complete: {}.", fb(root.size));
        self.store.root = Some(root);
    }
}

pub fn save(
    undo: Option<&sfo_core::undo::UndoManager>,
    s: &sfo_core::models::SessionLog,
) -> Result<(), String> {
    match undo {
        Some(u) => u.save_session(s).map_err(|e| e.to_string()),
        None => Err("history database unavailable".into()),
    }
}
