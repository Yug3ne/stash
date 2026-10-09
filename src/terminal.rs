//! Runs commands that need root (pacman, yay) in a real terminal window.
//!
//! Why not a password box in the app? This way sudo works exactly like always,
//! you see everything pacman prints and can answer its questions. It's how
//! Omarchy's own menus install packages too.

use crate::util::command_exists;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Exit code reported when the terminal window was closed before finishing.
pub const CANCELLED: i32 = 130;

/// Open a terminal running `script` (bash) and wait until it finishes.
/// Returns the script's exit code.
pub fn run_in_terminal(title: &str, script: &str) -> Result<i32> {
    let dir = std::env::temp_dir().join(format!("stash-{}-{}", std::process::id(), unique()));
    fs::create_dir_all(&dir)?;
    let script_path = dir.join("run.sh");
    let status_path = dir.join("status");
    fs::write(&script_path, wrap_script(title, script, &status_path))?;

    let result = open_terminal(title, &script_path).and_then(|()| wait_for_status(&status_path));
    let _ = fs::remove_dir_all(&dir);
    result
}

/// The script we actually run: the command, then report its exit code by
/// writing it to `status`. Terminals like Ghostty hand new windows over to an
/// already-running process, so waiting for the process we started doesn't work;
/// waiting for the status file does, with every terminal.
fn wrap_script(title: &str, script: &str, status: &Path) -> String {
    let status = status.display();
    format!(
        r#"#!/bin/bash
# If the window is closed early, still report back.
trap '[[ -f "{status}" ]] || echo {CANCELLED} > "{status}"' EXIT

printf '\e[1m%s\e[0m\n\n' {title}
(
{script}
)
code=$?
echo "$code" > "{status}"

if (( code == 0 )); then
  printf '\n\e[32m✓ Done\e[0m\n'
  sleep 1.5
else
  printf '\n\e[31m✗ Failed (exit code %s)\e[0m\n' "$code"
  read -rsn1 -p 'Press any key to close…'
fi
"#,
        title = shell_quote(title),
    )
}

/// Terminals to try, in order, and the arguments to run `bash <script>` in each.
fn terminal_command(title: &str, script: &str) -> Option<(&'static str, Vec<String>)> {
    let t = title.to_string();
    let s = script.to_string();
    let candidates: [(&str, Vec<String>); 8] = [
        // Omarchy (and modern distros) route this to your default terminal.
        ("xdg-terminal-exec", vec!["--app-id=org.omarchy.terminal".into(), format!("--title={t}"), "-e".into(), "bash".into(), s.clone()]),
        ("ghostty", vec![format!("--title={t}"), "-e".into(), "bash".into(), s.clone()]),
        ("alacritty", vec!["--title".into(), t.clone(), "-e".into(), "bash".into(), s.clone()]),
        ("kitty", vec!["--title".into(), t.clone(), "bash".into(), s.clone()]),
        ("foot", vec!["--title".into(), t.clone(), "bash".into(), s.clone()]),
        ("konsole", vec!["-e".into(), "bash".into(), s.clone()]),
        ("gnome-terminal", vec!["--title".into(), t.clone(), "--".into(), "bash".into(), s.clone()]),
        ("xterm", vec!["-T".into(), t, "-e".into(), "bash".into(), s]),
    ];
    candidates.into_iter().find(|(bin, _)| command_exists(bin))
}

fn open_terminal(title: &str, script: &Path) -> Result<()> {
    let Some((terminal, args)) = terminal_command(title, &script.to_string_lossy()) else {
        bail!("No terminal emulator found");
    };
    Command::new(terminal)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("could not open {terminal}"))?;
    Ok(())
}

fn wait_for_status(status: &Path) -> Result<i32> {
    loop {
        if let Ok(text) = fs::read_to_string(status)
            && let Ok(code) = text.trim().parse()
        {
            return Ok(code);
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

fn unique() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Quote text for safe use in a bash script: it'll always be one plain word.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}
