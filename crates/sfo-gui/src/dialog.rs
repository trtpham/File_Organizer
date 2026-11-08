//! The shared message / confirmation dialog (Tk `messagebox`).

use crate::theme;
use crate::widgets::{self, Kind};
use egui::RichText;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// What a confirmed dialog asks the app to run.
#[derive(Clone, Debug, PartialEq)]
pub enum Confirmed {
    ExecuteOrganize,
    MoveDuplicates,
    RenameFiles,
    CleanJunk,
    QuarantineSecrets,
    Shred,
    ArchiveCold,
    Undo(String),
}

#[derive(Clone, Debug)]
pub struct Dialog {
    pub title: String,
    pub body: String,
    pub level: Level,
    /// `Some` = Yes/No dialog; Yes dispatches this action.
    pub confirm: Option<Confirmed>,
    pub danger: bool,
}

impl Dialog {
    pub fn info(title: &str, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            level: Level::Info,
            confirm: None,
            danger: false,
        }
    }
    pub fn warn(title: &str, body: impl Into<String>) -> Self {
        Self {
            level: Level::Warning,
            ..Self::info(title, body)
        }
    }
    pub fn error(title: &str, body: impl Into<String>) -> Self {
        Self {
            level: Level::Error,
            ..Self::info(title, body)
        }
    }
    pub fn confirm(title: &str, body: impl Into<String>, action: Confirmed) -> Self {
        Self {
            confirm: Some(action),
            ..Self::info(title, body)
        }
    }
    pub fn confirm_danger(title: &str, body: impl Into<String>, action: Confirmed) -> Self {
        Self {
            danger: true,
            level: Level::Warning,
            ..Self::confirm(title, body, action)
        }
    }
}

/// Shows the front dialog; returns the confirmed action, if any.
pub fn show(ctx: &egui::Context, queue: &mut Vec<Dialog>) -> Option<Confirmed> {
    let d = queue.first()?.clone();
    let mut close = false;
    let mut result = None;
    egui::Modal::new(egui::Id::new("sfo_dialog")).show(ctx, |ui| {
        ui.set_max_width(460.0);
        let (icon, color) = match d.level {
            Level::Info => ("ℹ", theme::ACCENT),
            Level::Warning => ("⚠", theme::WARNING),
            Level::Error => ("✖", theme::DANGER),
        };
        ui.label(
            RichText::new(format!("{icon}  {}", d.title))
                .font(theme::bold(theme::HEADER))
                .color(color),
        );
        ui.add_space(theme::GAP_ROW);
        ui.label(&d.body);
        ui.add_space(theme::GAP_SECTION);
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| match &d.confirm {
                Some(action) => {
                    if widgets::button(ui, Kind::Secondary, "No").clicked() {
                        close = true;
                    }
                    let kind = if d.danger {
                        Kind::Danger
                    } else {
                        Kind::Primary
                    };
                    if widgets::button(ui, kind, "Yes").clicked() {
                        result = Some(action.clone());
                        close = true;
                    }
                }
                None => {
                    if widgets::button(ui, Kind::Primary, "OK").clicked() {
                        close = true;
                    }
                }
            },
        );
    });
    if close {
        queue.remove(0);
    }
    result
}
