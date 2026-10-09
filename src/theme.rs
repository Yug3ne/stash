//! Reads the active Omarchy theme so Stash matches the rest of the desktop.

use std::collections::HashMap;
use std::path::PathBuf;

/// The theme's colors, e.g. {"background": "#000000", "accent": "#8d8d8d", "mode": "dark"}.
/// None when not running Omarchy; the app then uses its built-in colors.
pub fn read_theme() -> Option<HashMap<String, String>> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    // Newer Omarchy keeps the current theme in ~/.local/state, older in ~/.config.
    let text = [".local/state/omarchy/current/theme/colors.toml", ".config/omarchy/current/theme/colors.toml"]
        .iter()
        .find_map(|file| std::fs::read_to_string(home.join(file)).ok())?;

    // colors.toml is flat `key = "value"` lines, so no TOML library is needed.
    let colors = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let value = value.trim().strip_prefix('"')?.split('"').next()?;
            Some((key.trim().to_string(), value.to_string()))
        })
        .collect();
    Some(colors)
}
