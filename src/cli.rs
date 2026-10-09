//! The command line: `stash install …`, `stash list`, …
//! Run `stash` with no arguments to open the app instead.

use crate::appimage::{self, InstallOptions, Source};
use crate::packages;
use crate::util::{Msg, human_size};
use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};
use std::io::{IsTerminal, Write};
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "stash",
    version,
    about = "Install AppImages and Arch Linux packages. Run without arguments to open the app."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Install an AppImage from a file, a link, or a GitHub repo (owner/repo)
    Install {
        /// Path to an .AppImage, an https link, or "owner/repo" on GitHub
        source: String,
        #[command(flatten)]
        options: InstallFlags,
    },
    /// List installed AppImages
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show AppImages that have updates (doesn't install anything)
    Updates {
        #[arg(long)]
        json: bool,
        /// Also show a desktop notification (for the daily timer)
        #[arg(long)]
        notify: bool,
    },
    /// Update an AppImage, or all of them if no name is given
    Update { name: Option<String> },
    /// Uninstall an AppImage
    Remove { name: String },
    /// Arch Linux packages (official repos and the AUR)
    #[command(subcommand)]
    Pkg(PkgCmd),
}

#[derive(Args)]
pub struct InstallFlags {
    /// Command/folder name (default: detected from the app)
    #[arg(long)]
    name: Option<String>,
    /// GitHub repo to update from (detected for most Electron apps)
    #[arg(long, value_name = "OWNER/REPO")]
    repo: Option<String>,
    /// Strip debug symbols and unused languages/docs: smaller and faster to load
    #[arg(long)]
    optimize: bool,
    /// Launch with --no-sandbox (a few Electron apps need it)
    #[arg(long)]
    no_sandbox: bool,
    /// Don't add native-Wayland flags to Electron apps
    #[arg(long)]
    no_wayland: bool,
    /// Keep the original .AppImage file (it's deleted by default)
    #[arg(long)]
    keep: bool,
}

#[derive(Subcommand)]
pub enum PkgCmd {
    /// Search the official repos and the AUR
    Search { query: Vec<String> },
    /// Install packages (pacman, or yay for AUR packages)
    Install { names: Vec<String> },
    /// Remove packages and the dependencies nothing else needs
    Remove { names: Vec<String> },
}

pub fn run(command: Cmd) -> Result<()> {
    match command {
        Cmd::Install { source, options } => {
            let options = InstallOptions {
                name: options.name,
                repo: options.repo,
                optimize: options.optimize,
                no_sandbox: options.no_sandbox,
                no_wayland: options.no_wayland,
                keep: options.keep,
            };
            appimage::install(Source::parse(&source)?, &options, &print)?;
        }
        Cmd::List { json } => {
            let apps = appimage::list();
            if json {
                println!("{}", serde_json::to_string_pretty(&apps)?);
            } else if apps.is_empty() {
                println!("No AppImages installed yet. Try: stash install marktext/marktext");
            } else {
                for app in apps {
                    let updatable = if app.repo.is_some() { "updatable" } else { "" };
                    println!(
                        "  {}  {:<28} {:<14} {:>9}  {updatable}",
                        green(&format!("{:<22}", app.name)),
                        app.display_name,
                        app.version,
                        human_size(app.size)
                    );
                }
            }
        }
        Cmd::Updates { json, notify } => {
            let updates = appimage::check_updates();
            if json {
                println!("{}", serde_json::to_string_pretty(&updates)?);
            } else if updates.is_empty() {
                print(Msg::Done("All AppImages are up to date".into()));
            } else {
                for update in &updates {
                    println!("  {}: {} → {}", update.name, update.current, update.latest);
                }
            }
            if notify && !updates.is_empty() {
                let names: Vec<&str> = updates.iter().map(|u| u.name.as_str()).collect();
                let _ = Command::new("notify-send")
                    .args(["-a", "Stash", "AppImage updates available", &names.join(", ")])
                    .status();
            }
        }
        Cmd::Update { name: Some(name) } => {
            appimage::update(&name, &print)?;
        }
        Cmd::Update { name: None } => appimage::update_all(&print)?,
        Cmd::Remove { name } => appimage::remove(&name, &print)?,
        Cmd::Pkg(command) => run_pkg(command)?,
    }
    Ok(())
}

fn run_pkg(command: PkgCmd) -> Result<()> {
    match command {
        PkgCmd::Search { query } => {
            for pkg in packages::search(&query.join(" "))?.iter().take(30) {
                let installed = if pkg.installed { " [installed]" } else { "" };
                println!("{} {}{}", green(&format!("{}/{}", pkg.repo, pkg.name)), pkg.version, installed);
                println!("    {}", pkg.description);
            }
        }
        // We're already in a terminal, so run pacman/yay right here.
        PkgCmd::Install { names } => {
            let in_repos = crate::util::output("pacman", &["-Slq"])?;
            let all_official = names.iter().all(|n| in_repos.lines().any(|l| l == n));
            let mut cmd = if all_official { Command::new("sudo") } else { Command::new("yay") };
            if all_official {
                cmd.args(["pacman", "-S", "--needed"]);
            } else {
                cmd.args(["-S", "--needed"]);
            }
            check_status(cmd.args(&names).status()?)?;
        }
        PkgCmd::Remove { names } => {
            check_status(Command::new("sudo").args(["pacman", "-Rns"]).args(&names).status()?)?;
        }
    }
    Ok(())
}

fn check_status(status: std::process::ExitStatus) -> Result<()> {
    if !status.success() {
        bail!("failed ({status})");
    }
    Ok(())
}

// ─── Output ─────────────────────────────────────────────────────────────────

fn color(code: &str, text: &str) -> String {
    if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn green(text: &str) -> String {
    color("32", text)
}

/// Print a progress message from an operation.
fn print(msg: Msg) {
    match msg {
        Msg::Step(text) => println!("{} {text}", color("34", "::")),
        Msg::Done(text) => println!("{} {text}", color("32", "✓")),
        Msg::Warn(text) => println!("{} {text}", color("33", "!")),
        Msg::Progress(percent) => {
            // Redraw one line in a terminal; skip it when output goes to a file.
            if std::io::stderr().is_terminal() {
                eprint!("\r   {percent:>3}%");
                if percent == 100 {
                    eprintln!();
                }
                let _ = std::io::stderr().flush();
            }
        }
    }
}
