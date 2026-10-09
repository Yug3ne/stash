//! Small helpers used everywhere: running commands, progress messages,
//! version comparison and formatting.

use anyhow::{Context, Result, bail};
use std::cmp::Ordering;
use std::process::{Command, Stdio};

// ─── Progress messages ──────────────────────────────────────────────────────

/// A progress message from a long-running operation. The CLI prints these;
/// the GUI forwards them to the page's activity log.
/// Sent to the page as {"kind": "step", "value": "…"}.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum Msg {
    /// A step that's starting: "Extracting MarkText…"
    Step(String),
    /// A step that finished well: "Installed to ~/.local/opt/marktext"
    Done(String),
    /// Something worth knowing that isn't an error.
    Warn(String),
    /// Download progress, 0–100.
    Progress(u8),
}

/// Where operations send their progress. `Sync` so parallel work can report too.
pub type Log<'a> = &'a (dyn Fn(Msg) + Sync);

// ─── Running other programs ─────────────────────────────────────────────────

/// Run a program and return its stdout. Fails with its stderr if it fails.
pub fn output(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .env("LC_ALL", "C") // English, parseable output
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("could not run {program}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("{program} failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run a program and return its stdout even if it exits with an error code
/// (some tools, like `checkupdates`, use exit codes to mean "nothing found").
pub fn output_lenient(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// Is `program` on the PATH?
pub fn command_exists(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
    })
}

/// Start a GUI program that keeps running after we exit.
pub fn launch_detached(program: &str, args: &[&str]) -> Result<()> {
    Command::new("setsid")
        .arg(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("could not start {program}"))?;
    Ok(())
}

// ─── Versions ───────────────────────────────────────────────────────────────

/// Is `new` a newer version than `old`? Unknown old versions count as older.
pub fn is_newer(new: &str, old: &str) -> bool {
    let (new, old) = (new.trim_start_matches('v'), old.trim_start_matches('v'));
    if old.is_empty() || old == "unknown" {
        return !new.is_empty();
    }
    compare_versions(new, old) == Ordering::Greater
}

/// Compare versions like "1.10.2" and "1.9", or "2.0.0-beta.3": numbers are
/// compared as numbers, everything else as text.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> Vec<String> {
        v.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    };
    let (a, b) = (parts(a), parts(b));
    for (x, y) in a.iter().zip(&b) {
        let order = match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

// ─── Formatting ─────────────────────────────────────────────────────────────

/// 1536 → "1.5 KB"
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{size:.1} {}", UNITS[unit]) }
}

/// "T3 Code (Alpha)" → "t3-code-alpha"
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// Total size of a directory tree, in bytes (symlinks not followed).
pub fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => dir_size(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("v2.0.0", "1.99"));
        assert!(!is_newer("1.18.2", "1.18.2"));
        assert!(!is_newer("0.17.0", "0.21.1"));
        assert!(is_newer("0.0.45", "unknown"));
        assert!(is_newer("1.2.1", "1.2"));
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("T3 Code (Alpha)"), "t3-code-alpha");
        assert_eq!(slugify("MarkText"), "marktext");
    }
}
