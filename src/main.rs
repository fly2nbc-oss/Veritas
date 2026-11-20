#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::ExitCode;

use veritas::screenshot::{self, Command};

fn main() -> ExitCode {
    // `veritas <folder or file>` opens the folder, like `Exec=veritas %f` in the desktop entry;
    // `veritas --screenshot …` takes the README picture.
    let command = match screenshot::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("veritas: {message}\n{}", screenshot::USAGE);
            return ExitCode::from(2);
        }
    };
    match command {
        // Without panic hook and run log: both would write into the user's data folder.
        Command::Screenshot(args) => screenshot::run(args),
        Command::Window(start) => window(start),
    }
}

fn window(start: Option<std::path::PathBuf>) -> ExitCode {
    // Before anything else: a panic should leave a trace in `crash.log`.
    veritas::crash::install();
    veritas::crash::install_run_log();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(veritas::ui::START_SIZE)
            .with_min_inner_size(veritas::ui::MIN_SIZE),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Veritas",
        native_options,
        Box::new(|cc| Ok(Box::new(veritas::ui::VeritasApp::new(cc, start)))),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("veritas: {err}");
            ExitCode::FAILURE
        }
    }
}
