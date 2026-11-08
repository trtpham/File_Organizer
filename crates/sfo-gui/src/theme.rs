//! Slate dark theme, ported from the Tkinter `gui/theme.py`.
//!
//! Colours are identical. Tk font sizes were points; egui uses logical
//! pixels, so sizes are converted ×4/3 (96 DPI). Tk paddings were already
//! pixels and are kept as-is.

use egui::{Color32, FontFamily, FontId, RichText};
use std::sync::Arc;

pub const BG_DARK: Color32 = Color32::from_rgb(0x0f, 0x17, 0x2a);
pub const CARD_BG: Color32 = Color32::from_rgb(0x1e, 0x29, 0x3b);
pub const CARD_BORDER: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
pub const ACCENT: Color32 = Color32::from_rgb(0x38, 0xbd, 0xf8);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x02, 0x84, 0xc7);
pub const SUCCESS: Color32 = Color32::from_rgb(0x22, 0xc5, 0x5e);
pub const WARNING: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);
pub const DANGER: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const TEXT_LIGHT: Color32 = Color32::from_rgb(0xf8, 0xfa, 0xfc);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x94, 0xa3, 0xb8);
pub const TREE_SELECT: Color32 = Color32::from_rgb(0x25, 0x63, 0xeb);
/// Table header background (`Treeview.Heading` in theme.py).
pub const TABLE_HEADING: Color32 = Color32::from_rgb(0x17, 0x20, 0x33);
pub const ON_LIGHT: Color32 = Color32::BLACK;
pub const ON_DARK: Color32 = Color32::WHITE;

// Type roles (Tk points → px).
pub const TITLE: f32 = 20.0; // 15 pt
pub const HEADER: f32 = 15.0; // 11 pt
pub const BODY: f32 = 12.0; // 9 pt
pub const SMALL: f32 = 11.0; // 8 pt
pub const STAT_VALUE: f32 = 24.0; // 18 pt
pub const STATUS_TITLE: f32 = 13.5; // 10 pt (status lines styled as titles)

// Spacing (px, as in the Tk code).
pub const PAD_TAB: i8 = 14;
pub const PAD_CARD: i8 = 14;
pub const GAP_SECTION: f32 = 10.0;
pub const GAP_ROW: f32 = 6.0;
pub const ROW_HEIGHT: f32 = 26.0;
pub const BUTTON_HEIGHT: f32 = 28.0;

const BOLD: &str = "bold";

pub fn body() -> FontId {
    FontId::proportional(BODY)
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

pub fn mono() -> FontId {
    FontId::monospace(BODY)
}

/// Page title (accent, bold).
pub fn title(text: &str) -> RichText {
    RichText::new(text).font(bold(TITLE)).color(ACCENT)
}

/// Section heading inside a tab or card.
pub fn header(text: &str) -> RichText {
    RichText::new(text).font(bold(HEADER)).color(TEXT_LIGHT)
}

/// Helper / secondary text.
pub fn muted(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::proportional(SMALL))
        .color(TEXT_MUTED)
}

/// Status line rendered in the title style at a smaller size.
pub fn status_title(text: &str, color: Color32) -> RichText {
    RichText::new(text).font(bold(STATUS_TITLE)).color(color)
}

fn load(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Segoe UI / Consolas on Windows (theme.py's fonts), egui defaults elsewhere.
/// Always defines the `bold` family so `bold()` never panics.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let candidates = [
        ("segoe", format!(r"{windir}\Fonts\segoeui.ttf")),
        ("segoe_bold", format!(r"{windir}\Fonts\segoeuib.ttf")),
        ("consolas", format!(r"{windir}\Fonts\consola.ttf")),
        ("segoe_emoji", format!(r"{windir}\Fonts\seguiemj.ttf")),
        ("segoe_symbol", format!(r"{windir}\Fonts\seguisym.ttf")),
    ];
    let mut have = Vec::new();
    for (name, path) in &candidates {
        if cfg!(windows)
            && let Some(bytes) = load(path)
        {
            fonts
                .font_data
                .insert((*name).into(), Arc::new(egui::FontData::from_owned(bytes)));
            have.push(*name);
        }
    }
    let defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let fallbacks: Vec<String> = ["segoe_emoji", "segoe_symbol"]
        .iter()
        .filter(|n| have.contains(n))
        .map(|n| n.to_string())
        .collect();

    let mut prop = Vec::new();
    if have.contains(&"segoe") {
        prop.push("segoe".to_string());
    }
    prop.extend(defaults.iter().cloned());
    prop.extend(fallbacks.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Proportional, prop.clone());

    let mut bold_family = Vec::new();
    if have.contains(&"segoe_bold") {
        bold_family.push("segoe_bold".to_string());
    }
    bold_family.extend(prop);
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), bold_family);

    if have.contains(&"consolas") {
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "consolas".into());
    }
    ctx.set_fonts(fonts);
}

/// Global visuals (`apply_theme`).
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG_DARK;
    v.window_fill = CARD_BG;
    v.window_stroke = egui::Stroke::new(1.0_f32, CARD_BORDER);
    v.extreme_bg_color = CARD_BG; // text fields
    v.faint_bg_color = Color32::from_rgb(0x24, 0x31, 0x45); // striped rows
    v.selection.bg_fill = TREE_SELECT;
    v.selection.stroke = egui::Stroke::new(1.0_f32, ON_DARK);
    v.hyperlink_color = ACCENT;
    v.override_text_color = Some(TEXT_LIGHT);
    v.widgets.inactive.weak_bg_fill = CARD_BG;
    v.widgets.inactive.bg_fill = CARD_BG;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, CARD_BORDER);
    v.widgets.hovered.weak_bg_fill = CARD_BORDER;
    v.widgets.hovered.bg_fill = CARD_BORDER;
    v.widgets.active.weak_bg_fill = ACCENT_HOVER;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, GAP_ROW);
        s.spacing.button_padding = egui::vec2(10.0, 6.0);
        s.spacing.interact_size.y = BUTTON_HEIGHT;
        s.text_styles.insert(egui::TextStyle::Body, body());
        s.text_styles.insert(egui::TextStyle::Button, body());
        s.text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(SMALL));
        s.text_styles.insert(egui::TextStyle::Monospace, mono());
        s.text_styles.insert(egui::TextStyle::Heading, bold(HEADER));
    });
}
