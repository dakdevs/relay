use std::io::Write;

use relay_desktop::cli::{self, Cli};

fn main() {
    // Flags are handled before any GPUI, tray, proxy or file access, so
    // `relay --version` is safe on headless machines.
    match cli::parse(std::env::args()) {
        Cli::Print(text) => {
            let _ = std::io::stdout().write_all(text.as_bytes());
        }
        Cli::Error(text) => {
            let _ = std::io::stderr().write_all(text.as_bytes());
            std::process::exit(2);
        }
        Cli::Run => relay_desktop::app::run(),
    }
}
