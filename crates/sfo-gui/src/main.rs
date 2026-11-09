//! `sfo-gui` — Smart File Organizer desktop app (egui/eframe).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod dialog;
#[cfg(test)]
mod gui_tests;
mod tabs_main;
mod tabs_system;
mod tabs_tools;
mod theme;
mod widgets;

/// `--source <DIR>` pre-fills every folder field (used by the Explorer menu
/// and `sfo --gui`). Other arguments are ignored.
fn source_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "--source" || a == "-s")
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() -> eframe::Result<()> {
    let source = source_arg();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Smart File Organizer v{}", sfo_core::VERSION))
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([920.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Smart File Organizer",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(&cc.egui_ctx, source)))),
    )
}
