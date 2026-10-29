#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use neochat::app::NeoChat;
use neochat::storage::Store;

fn load_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/neochat.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    let store = Store::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NeoChat")
        .with_inner_size([960.0, 720.0])
        .with_min_inner_size([560.0, 420.0]);

    if let Some([width, height]) = store.config.window_size {
        viewport = viewport.with_inner_size([width.max(560.0), height.max(420.0)]);
    }

    if let Some(icon) = load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "NeoChat",
        options,
        Box::new(move |cc| Ok(Box::new(NeoChat::new(cc, store)))),
    )
}
