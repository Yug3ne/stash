//! `--optimize`: make an extracted app smaller and quicker to load by removing
//! things it doesn't need at runtime.

use crate::util::{Log, Msg, command_exists, dir_size, human_size};
use rayon::prelude::*;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn optimize(app_dir: &Path, log: Log) {
    log(Msg::Step("Optimizing (stripping symbols, removing unused languages and docs)…".into()));
    let before = dir_size(app_dir);

    strip_binaries(app_dir);
    remove_unused_locales(app_dir);
    for docs in ["man", "info", "doc", "help", "gtk-doc"] {
        let _ = fs::remove_dir_all(app_dir.join("usr/share").join(docs));
    }

    let saved = before.saturating_sub(dir_size(app_dir));
    log(Msg::Done(format!("Saved {}", human_size(saved))));
}

/// Remove debug symbols from every ELF binary/library, using all CPU cores.
fn strip_binaries(app_dir: &Path) {
    if !command_exists("strip") {
        return;
    }
    let mut files = Vec::new();
    collect_files(app_dir, &mut files);
    files.par_iter().filter(|file| is_elf(file)).for_each(|file| {
        let _ = Command::new("strip")
            .arg("--strip-debug")
            .arg(file)
            .stderr(Stdio::null())
            .status();
    });
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect_files(&entry.path(), files);
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
}

/// ELF files start with the bytes 0x7F 'E' 'L' 'F'.
fn is_elf(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    fs::File::open(path).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && &magic == b"\x7fELF"
}

/// Languages to keep: English plus whatever $LANG says (e.g. "de_DE" → de, de_DE).
fn kept_languages() -> Vec<String> {
    let lang = std::env::var("LANG").unwrap_or_default();
    let lang = lang.split(['.', '@']).next().unwrap_or("").to_string();
    let primary = lang.split('_').next().unwrap_or("").to_string();
    let mut keep: Vec<String> = ["en", "en_US", "en_GB", "C", "POSIX"].map(String::from).to_vec();
    keep.extend([lang, primary].into_iter().filter(|s| !s.is_empty()));
    keep
}

fn remove_unused_locales(app_dir: &Path) {
    let keep = kept_languages();
    let is_kept = |name: &str| {
        let name = name.replace('-', "_");
        let base = name.split(['.', '@']).next().unwrap_or("");
        keep.iter().any(|k| k == base || k == base.split('_').next().unwrap_or(""))
    };

    // gettext translations: usr/share/locale/<lang>/
    for root in ["usr/share/locale", "usr/share/locales"] {
        for entry in fs::read_dir(app_dir.join(root)).into_iter().flatten().flatten() {
            if !is_kept(&entry.file_name().to_string_lossy()) {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    // Electron/Chromium: locales/<lang>.pak (about 50 of them, ~25 MB)
    for entry in fs::read_dir(app_dir.join("locales")).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(lang) = name.strip_suffix(".pak")
            && !is_kept(lang)
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}
