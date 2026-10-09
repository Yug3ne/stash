//! Reading and writing freedesktop `.desktop` files (menu entries).

use std::path::Path;

/// The key/value pairs of a `.desktop` file's `[Desktop Entry]` section.
#[derive(Debug, Default, Clone)]
pub struct DesktopEntry {
    entries: Vec<(String, String)>,
}

impl DesktopEntry {
    /// Parse a file. A missing or unreadable file gives an empty entry.
    pub fn read(path: &Path) -> Self {
        std::fs::read_to_string(path).map(|text| Self::parse(&text)).unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        let mut in_main_section = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_main_section = line == "[Desktop Entry]";
            } else if in_main_section && let Some((key, value)) = line.split_once('=') {
                entries.push((key.trim().to_string(), value.trim().to_string()));
            }
        }
        Self { entries }
    }

    /// The value for `key`, if set and not empty.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    /// Set (or replace) a key. Keys keep the order they were first added in.
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    pub fn to_text(&self) -> String {
        let mut text = String::from("[Desktop Entry]\n");
        for (key, value) in &self.entries {
            text.push_str(&format!("{key}={value}\n"));
        }
        text
    }
}

/// Remove repeats from a `;`-separated list: "a;b;a;" → "a;b;"
pub fn dedupe_list(list: &str) -> String {
    let mut seen = Vec::new();
    for item in list.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    if seen.is_empty() { String::new() } else { format!("{};", seen.join(";")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_main_section_only() {
        let entry = DesktopEntry::parse(
            "[Desktop Entry]\nName=MarkText\nExec=marktext %U\n\n[Desktop Action new]\nName=New Window\n",
        );
        assert_eq!(entry.get("Name"), Some("MarkText"));
        assert_eq!(entry.get("Exec"), Some("marktext %U"));
    }

    #[test]
    fn dedupes() {
        assert_eq!(dedupe_list("text/markdown;text/markdown;"), "text/markdown;");
        assert_eq!(dedupe_list(""), "");
    }
}
