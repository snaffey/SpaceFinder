#![cfg_attr(windows, windows_subsystem = "windows")]

mod ui;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let initial_path = args.next().map(PathBuf::from);
    let elevation_ready = args
        .next()
        .filter(|value| value == spacefinder::actions::ELEVATION_READY_ARG)
        .and_then(|_| args.next());
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("SpaceFinder")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1040.0, 680.0]),
        centered: true,
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "SpaceFinder",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::SpaceFinder::new(
                cc,
                initial_path,
                elevation_ready,
            )))
        }),
    ) {
        rfd::MessageDialog::new()
            .set_title("SpaceFinder")
            .set_description(format!("Não foi possível abrir a interface: {error}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}
