//! Shared pieces, ported from `gui/components.py` and the ttk styles:
//! card panel, stat card, folder picker, themed buttons, results table.

use crate::theme::{self, CARD_BG, CARD_BORDER};
use egui::{Color32, Margin, RichText, Ui};
use egui_extras::{Column, TableBuilder};

/// `Card.TFrame`: card fill, 1px border.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD_BG)
        .stroke(egui::Stroke::new(1.0_f32, CARD_BORDER))
        .inner_margin(Margin::same(theme::PAD_CARD))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Button variants from theme.py.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Warning,
    Danger,
}

pub fn button(ui: &mut Ui, kind: Kind, text: &str) -> egui::Response {
    button_enabled(ui, kind, text, true)
}

pub fn button_enabled(ui: &mut Ui, kind: Kind, text: &str, enabled: bool) -> egui::Response {
    let (fill, fg, strong) = match kind {
        Kind::Primary => (theme::ACCENT, theme::ON_LIGHT, true),
        Kind::Secondary => (CARD_BG, theme::TEXT_LIGHT, false),
        Kind::Warning => (theme::WARNING, theme::ON_LIGHT, true),
        Kind::Danger => (theme::DANGER, theme::ON_DARK, true),
    };
    let font = if strong {
        theme::bold(theme::BODY)
    } else {
        theme::body()
    };
    let mut b = egui::Button::new(RichText::new(text).font(font).color(fg))
        .fill(fill)
        .min_size(egui::vec2(0.0, theme::BUTTON_HEIGHT));
    if kind == Kind::Secondary {
        b = b.stroke(egui::Stroke::new(1.0_f32, CARD_BORDER));
    }
    ui.add_enabled(enabled, b)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// `StatCard`: muted uppercase title, large bold value.
pub fn stat_card(ui: &mut Ui, title: &str, value: &str, color: Option<Color32>) {
    egui::Frame::new()
        .fill(CARD_BG)
        .stroke(egui::Stroke::new(1.0_f32, CARD_BORDER))
        .inner_margin(Margin::same(12))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(theme::muted(&title.to_uppercase()));
            ui.add_space(4.0);
            ui.label(
                RichText::new(value)
                    .font(theme::bold(theme::STAT_VALUE))
                    .color(color.unwrap_or(theme::TEXT_LIGHT)),
            );
        });
}

/// A row of equally wide stat cards.
pub fn stat_row(ui: &mut Ui, cards: &[(&str, String, Option<Color32>)]) {
    ui.columns(cards.len(), |cols| {
        for (col, (title, value, color)) in cols.iter_mut().zip(cards) {
            stat_card(col, title, value, *color);
        }
    });
}

/// Which native dialog a picker opens.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Folder,
    File,
    /// "Folder..." and "File..." buttons (vault source, shredder target).
    Either,
}

/// Fixed-width, left-aligned field label (Tk `Label(width=...)` + `side=left`).
pub fn field_label(ui: &mut Ui, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, theme::BUTTON_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.label(text);
        },
    );
}

/// `FolderPicker`: label, path field, "Browse..." button. Returns `true`
/// when the path changed through the dialog.
pub fn path_picker(
    ui: &mut Ui,
    label: &str,
    path: &mut String,
    pick: Pick,
    label_width: f32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        field_label(ui, label, label_width);
        let browse_w = if pick == Pick::Either { 172.0 } else { 92.0 };
        let field_w = (ui.available_width() - browse_w - 8.0).max(80.0);
        ui.add_sized(
            [field_w, theme::BUTTON_HEIGHT],
            egui::TextEdit::singleline(path),
        );
        let start = if path.is_empty() {
            ".".to_string()
        } else {
            path.clone()
        };
        let mut chosen = None;
        match pick {
            Pick::Either => {
                if button(ui, Kind::Secondary, "Folder...").clicked() {
                    chosen = rfd::FileDialog::new().set_directory(&start).pick_folder();
                }
                if button(ui, Kind::Secondary, "File...").clicked() {
                    chosen = rfd::FileDialog::new().set_directory(&start).pick_file();
                }
            }
            Pick::Folder | Pick::File => {
                if button(ui, Kind::Secondary, "Browse...").clicked() {
                    let dialog = rfd::FileDialog::new().set_directory(&start);
                    chosen = if pick == Pick::Folder {
                        dialog.pick_folder()
                    } else {
                        dialog.pick_file()
                    };
                }
            }
        }
        if let Some(p) = chosen {
            *path = p.display().to_string();
            changed = true;
        }
    });
    changed
}

pub fn folder_picker(ui: &mut Ui, label: &str, path: &mut String) -> bool {
    path_picker(ui, label, path, Pick::Folder, 130.0)
}

/// Read-only results table (`ScrollableTreeview`). With `selected`, a click
/// on a row selects it.
pub fn table(
    ui: &mut Ui,
    id: &str,
    headings: &[&str],
    rows: &[Vec<String>],
    selected: Option<&mut Option<usize>>,
) {
    let height = ui.available_height().max(120.0);
    let mut sel_copy = selected.as_ref().and_then(|s| **s);
    let selectable = selected.is_some();
    ui.push_id(id, |ui| {
        let mut b = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .min_scrolled_height(height)
            .max_scroll_height(height)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        if selectable {
            b = b.sense(egui::Sense::click());
        }
        for i in 0..headings.len() {
            b = if i + 1 == headings.len() {
                b.column(Column::remainder().at_least(80.0))
            } else {
                b.column(Column::initial(150.0).at_least(60.0).clip(true))
            };
        }
        b.header(theme::ROW_HEIGHT, |mut h| {
            for head in headings {
                h.col(|ui| {
                    ui.painter()
                        .rect_filled(ui.max_rect(), 0.0, theme::TABLE_HEADING);
                    ui.label(RichText::new(*head).font(theme::bold(theme::BODY)));
                });
            }
        })
        .body(|body| {
            body.rows(theme::ROW_HEIGHT, rows.len(), |mut row| {
                let i = row.index();
                row.set_selected(sel_copy == Some(i));
                for cell in &rows[i] {
                    row.col(|ui| {
                        ui.add(egui::Label::new(cell).truncate());
                    });
                }
                if row.response().clicked() {
                    sel_copy = Some(i);
                }
            });
        });
    });
    if let Some(s) = selected {
        *s = sel_copy;
    }
}

/// Monospace log area (USB / cloud sync logs).
pub fn log_view(ui: &mut Ui, id: &str, text: &str) {
    egui::Frame::new()
        .fill(CARD_BG)
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt(id)
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(RichText::new(text).font(theme::mono()));
                });
        });
}

/// Drops emoji variation selectors (U+FE0F), which egui draws as a wide blank.
pub fn plain_emoji(s: &str) -> String {
    s.replace('\u{fe0f}', "")
}

/// Sub-tab strip used by the Sync and Security tabs.
pub fn sub_tabs(ui: &mut Ui, current: &mut usize, labels: &[String]) {
    ui.horizontal(|ui| {
        for (i, l) in labels.iter().enumerate() {
            let on = *current == i;
            let text = RichText::new(plain_emoji(l)).color(if on {
                theme::ACCENT
            } else {
                theme::TEXT_MUTED
            });
            if ui
                .add(egui::Button::new(text).fill(if on { theme::BG_DARK } else { CARD_BG }))
                .clicked()
            {
                *current = i;
            }
        }
    });
    ui.add_space(theme::GAP_ROW);
}
