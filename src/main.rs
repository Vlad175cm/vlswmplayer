mod app;
mod artwork;
mod input;
mod library;
mod cache;
mod config;
mod likes;
mod model;
mod mpv;
mod sc;
mod theme;
mod upload;
mod ui;
mod util;

use eframe::egui;

fn main() -> eframe::Result<()> {
    if std::env::var_os("VLSWMPLAYER_REINDEX").is_some() {
        let count = upload::rebuild_index();
        println!("indexed {count} files");
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("vlswmplayer")
            .with_fullscreen(true),
        ..Default::default()
    };

    eframe::run_native(
        "vlswmplayer",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
