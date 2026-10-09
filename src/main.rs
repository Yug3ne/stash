//! Stash: install AppImages and Arch Linux packages on Omarchy.
//!
//! `stash` opens the app; `stash <command>` runs on the command line.
//! Both use the same code below, so they always behave the same.

mod appimage; // extract, integrate, update and remove AppImages
mod cli; // command line interface
mod gui; // the app window and the actions its pages can call
mod http; // GitHub/AUR requests and resumable downloads
mod icons; // app icons for the pages
mod packages; // pacman and AUR
mod terminal; // run sudo commands in a terminal window
mod theme; // Omarchy theme colors
mod util; // shared helpers

use clap::Parser;

fn main() {
    let args = cli::Cli::parse();
    match args.command {
        None => gui::run(),
        Some(command) => {
            if let Err(error) = cli::run(command) {
                eprintln!("✗ {error:#}");
                std::process::exit(1);
            }
        }
    }
}
