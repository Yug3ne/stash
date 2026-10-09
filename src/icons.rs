//! Finds app icons on disk and turns them into data URLs the page can show
//! (the page itself can't read files).

use base64::Engine;
use std::path::{Path, PathBuf};

/// An image file as a `data:` URL, or None if it can't be read.
pub fn data_url(file: &Path) -> Option<String> {
    let mime = match file.extension()?.to_str()? {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "xpm" => return None, // browsers can't show XPM
        _ => "image/png",
    };
    let bytes = std::fs::read(file).ok()?;
    // Skip huge icons; a 512 KB icon in every list row would slow the page.
    if bytes.len() > 512 * 1024 {
        return None;
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}

/// Resolve a `.desktop` file's `Icon=` value (an icon name or a path) to a file.
pub fn find_icon_file(icon: &str) -> Option<PathBuf> {
    if icon.starts_with('/') {
        return Some(PathBuf::from(icon));
    }
    // Where installed apps put their icons, best quality first.
    const DIRS: [&str; 8] = [
        "/usr/share/icons/hicolor/256x256/apps",
        "/usr/share/icons/hicolor/128x128/apps",
        "/usr/share/icons/hicolor/scalable/apps",
        "/usr/share/icons/hicolor/512x512/apps",
        "/usr/share/icons/hicolor/96x96/apps",
        "/usr/share/icons/hicolor/64x64/apps",
        "/usr/share/icons/hicolor/48x48/apps",
        "/usr/share/pixmaps",
    ];
    DIRS.iter()
        .flat_map(|dir| ["png", "svg"].map(|ext| Path::new(dir).join(format!("{icon}.{ext}"))))
        .find(|file| file.is_file())
}

/// The icon of the app described by a `.desktop` file, as a data URL.
pub fn desktop_file_icon(desktop_file: &Path) -> Option<String> {
    let entry = crate::appimage::desktop::DesktopEntry::read(desktop_file);
    data_url(&find_icon_file(entry.get("Icon")?)?)
}
