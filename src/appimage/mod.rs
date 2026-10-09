//! AppImages, installed properly: extracted once onto disk (no FUSE mount on
//! every launch), with a menu entry, an icon, a terminal command, and updates
//! from GitHub releases.
//!
//! Layout of an installed app called `marktext`:
//!   ~/.local/opt/marktext/                      the extracted files
//!   ~/.local/share/applications/marktext.desktop menu entry (+ our settings)
//!   ~/.local/bin/marktext                        terminal command

pub mod desktop;
mod extract;
pub mod github;
mod optimize;

use crate::util::{Log, Msg, dir_size, is_newer, launch_detached, slugify};
use anyhow::{Context, Result, bail};
use desktop::{DesktopEntry, dedupe_list};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Every menu entry we create carries this, so we only ever touch our own apps.
const MARKER: &str = "X-InstalledBy";
const MARKER_VALUE: &str = "stash";
/// Apps installed by the old bash version are still ours.
const OLD_MARKER_VALUE: &str = "appimage-install";

// ─── Where things go ────────────────────────────────────────────────────────

/// Install locations. Each can be overridden with an environment variable,
/// which the tests use to install into a scratch folder.
pub struct Dirs {
    pub opt: PathBuf,
    pub applications: PathBuf,
    pub bin: PathBuf,
}

impl Dirs {
    pub fn new() -> Self {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let dir = |var: &str, default: &str| std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| home.join(default));
        Dirs {
            opt: dir("STASH_INSTALL_DIR", ".local/opt"),
            applications: dir("STASH_DESKTOP_DIR", ".local/share/applications"),
            bin: dir("STASH_BIN_DIR", ".local/bin"),
        }
    }

    fn app_dir(&self, name: &str) -> PathBuf {
        self.opt.join(name)
    }

    fn desktop_file(&self, name: &str) -> PathBuf {
        self.applications.join(format!("{name}.desktop"))
    }
}

/// Names become folder, file and command names, so keep them simple.
fn check_name(name: &str) -> Result<()> {
    let valid = name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
    if !valid {
        bail!("Invalid app name '{name}': use lowercase letters, digits, '.', '_' and '-'");
    }
    Ok(())
}

fn check_repo(repo: &str) -> Result<()> {
    let valid = repo.split('/').count() == 2
        && repo.split('/').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        });
    if !valid {
        bail!("GitHub repo must look like owner/repo, got '{repo}'");
    }
    Ok(())
}

// ─── Installing ─────────────────────────────────────────────────────────────

/// What to install from.
pub enum Source {
    File(PathBuf),
    Url(String),
    GitHub(String),
}

impl Source {
    /// Understands a file path, an https link to an .AppImage, a GitHub page
    /// (https://github.com/owner/repo) or just "owner/repo".
    pub fn parse(text: &str) -> Result<Source> {
        let text = text.trim();
        if Path::new(text).is_file() {
            return Ok(Source::File(PathBuf::from(text)));
        }
        if let Some(rest) = text.strip_prefix("https://github.com/") {
            let parts: Vec<&str> = rest.trim_end_matches('/').trim_end_matches(".git").split('/').collect();
            if parts.len() == 2 {
                return Ok(Source::GitHub(parts.join("/")));
            }
        }
        if text.starts_with("https://") || text.starts_with("http://") {
            return Ok(Source::Url(text.to_string()));
        }
        if check_repo(text).is_ok() {
            return Ok(Source::GitHub(text.to_string()));
        }
        bail!("'{text}' is not a file, a link or a GitHub repo (owner/repo)")
    }
}

/// Install options, from the CLI flags or the GUI's checkboxes.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstallOptions {
    /// Command/folder name; detected from the app if not given.
    pub name: Option<String>,
    /// GitHub repo to update from; detected for most Electron apps.
    pub repo: Option<String>,
    /// Launch with --no-sandbox (a few Electron apps need it).
    pub no_sandbox: bool,
    /// Strip debug symbols and unused languages/docs.
    pub optimize: bool,
    /// Don't add native-Wayland flags to Electron apps.
    pub no_wayland: bool,
    /// Keep the original .AppImage file (it's deleted by default).
    pub keep: bool,
}

/// Install an AppImage. Returns the installed app's name.
pub fn install(source: Source, options: &InstallOptions, log: Log) -> Result<String> {
    let dirs = Dirs::new();
    fs::create_dir_all(&dirs.opt)?;
    if let Some(repo) = &options.repo {
        check_repo(repo)?;
    }

    match source {
        Source::File(file) => {
            let file = file.canonicalize()?;
            if !file.to_string_lossy().to_lowercase().ends_with(".appimage") {
                bail!("Not an AppImage: {}", file.display());
            }
            let name = install_file(&dirs, &file, options, None, log)?;
            if !options.keep {
                fs::remove_file(&file)?;
                log(Msg::Done(format!("Removed {}", file.display())));
            }
            Ok(name)
        }
        Source::Url(url) => {
            let download = Download::new(&dirs.opt, &url)?;
            crate::http::download(&url, &download.file, log)?;
            install_file(&dirs, &download.file, options, None, log)
        }
        Source::GitHub(repo) => {
            check_repo(&repo)?;
            log(Msg::Step(format!("Looking up the latest release of {repo}…")));
            let release = github::latest_release(&repo)?;
            let download = Download::new(&dirs.opt, &release.url)?;
            crate::http::download(&release.url, &download.file, log)?;
            let options = InstallOptions { repo: Some(repo), ..options.clone() };
            install_file(&dirs, &download.file, &options, Some(&release.version), log)
        }
    }
}

/// The core install, shared by install and update.
/// `version_hint` is used when the AppImage doesn't say its own version.
fn install_file(
    dirs: &Dirs,
    appimage: &Path,
    options: &InstallOptions,
    version_hint: Option<&str>,
    log: Log,
) -> Result<String> {
    // Work next to the final location: same disk, so no RAM-backed /tmp,
    // and moving into place is an instant rename.
    let staging = TempPath::new(&dirs.opt, "staging");
    fs::create_dir_all(&staging.0)?;
    let new_app = staging.0.join("app");

    let file_name = appimage.file_name().unwrap_or_default().to_string_lossy();
    log(Msg::Step(format!("Extracting {file_name}…")));
    let started = std::time::Instant::now();
    extract::extract(appimage, &new_app)?;
    log(Msg::Done(format!("Extracted in {:.1}s", started.elapsed().as_secs_f32())));

    patch_apprun(&new_app);

    let embedded = DesktopEntry::read(&embedded_desktop_file(&new_app).unwrap_or_default());
    let name = match &options.name {
        Some(name) => name.clone(),
        None => detect_name(&embedded, appimage),
    };
    check_name(&name)?;
    let repo = options.repo.clone().or_else(|| detect_repo(&new_app));
    let version = embedded
        .get("X-AppImage-Version")
        .or(version_hint)
        .unwrap_or("unknown")
        .trim_start_matches('v')
        .to_string();
    log(Msg::Step(format!(
        "Name: {name}   Version: {version}{}",
        repo.as_ref().map(|r| format!("   Updates from: {r}")).unwrap_or_default()
    )));

    if options.optimize {
        optimize::optimize(&new_app, log);
    }

    // Swap in the new version; the old one is deleted with the staging folder.
    let target = dirs.app_dir(&name);
    if target.exists() {
        fs::rename(&target, staging.0.join("old"))?;
        log(Msg::Step("Replacing the existing installation".into()));
    }
    fs::rename(&new_app, &target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    log(Msg::Done(format!("Installed to {}", target.display())));
    drop(staging);

    let settings = Settings {
        repo,
        no_sandbox: options.no_sandbox,
        optimized: options.optimize,
        wayland: !options.no_wayland,
    };
    integrate(dirs, &name, &target, &embedded, &version, &settings, log)?;
    log(Msg::Done(format!("Done! Open it from your app launcher or run: {name}")));
    Ok(name)
}

/// A temporary file or folder that's deleted when this value goes away,
/// even if installing fails halfway.
struct TempPath(PathBuf);

impl TempPath {
    fn new(dir: &Path, kind: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        TempPath(dir.join(format!(".stash-{kind}-{}-{nanos}", std::process::id())))
    }
}

/// A file being downloaded, in its own temporary folder so it keeps its real
/// name (messages then say "Extracting MarkText-1.0.AppImage"). The folder
/// and file are deleted when this goes away.
struct Download {
    _folder: TempPath,
    file: PathBuf,
}

impl Download {
    fn new(dir: &Path, url: &str) -> Result<Self> {
        let folder = TempPath::new(dir, "download");
        fs::create_dir_all(&folder.0)?;
        let name = url.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("download.AppImage");
        Ok(Download { file: folder.0.join(name), _folder: folder })
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0).or_else(|_| fs::remove_file(&self.0));
    }
}

/// Many AppRun scripts only set APPDIR when it's empty. Launched from another
/// AppImage (or a terminal that inherited its environment), the wrong APPDIR
/// leaks in and the app won't start. Make it always point at this app.
fn patch_apprun(app_dir: &Path) {
    let apprun = app_dir.join("AppRun");
    let Ok(script) = fs::read_to_string(&apprun) else { return }; // binary AppRun: nothing to do
    let mut lines: Vec<&str> = script.lines().collect();

    // Replace the whole `if [ -z "$APPDIR" ] … fi` block with one assignment.
    let Some(start) = lines.iter().position(|l| l.contains("if [ -z \"$APPDIR\" ]")) else { return };
    let Some(length) = lines[start..].iter().position(|l| l.trim() == "fi") else { return };
    lines.splice(start..=start + length, [r#"APPDIR="$(dirname "$(readlink -f "${THIS}")")""#]);
    let _ = fs::write(&apprun, lines.join("\n") + "\n");
}

// ─── Reading app metadata ───────────────────────────────────────────────────

/// The .desktop file shipped inside the AppImage.
fn embedded_desktop_file(app_dir: &Path) -> Option<PathBuf> {
    fs::read_dir(app_dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "desktop"))
}

/// Command name: StartupWMClass if it's a plain word, else the slugified Name,
/// else the file name without version and architecture.
fn detect_name(embedded: &DesktopEntry, appimage: &Path) -> String {
    if let Some(class) = embedded.get("StartupWMClass")
        && class.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return slugify(class);
    }
    if let Some(name) = embedded.get("Name") {
        return slugify(name);
    }
    // "MyApp-1.2.3-x86_64.AppImage" → "myapp"
    let stem = appimage.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let base = stem
        .split(['-', '_'])
        .take_while(|part| !part.starts_with(|c: char| c.is_ascii_digit()) && !part.starts_with('v'))
        .collect::<Vec<_>>()
        .join("-");
    slugify(if base.is_empty() { &stem } else { &base })
}

/// GitHub "owner/repo" from electron-builder's resources/app-update.yml.
fn detect_repo(app_dir: &Path) -> Option<String> {
    let yml = fs::read_to_string(app_dir.join("resources/app-update.yml")).ok()?;
    let field = |key: &str| {
        yml.lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|value| value.trim().trim_matches(['"', '\'']).to_string())
    };
    if field("provider:")? != "github" {
        return None;
    }
    Some(format!("{}/{}", field("owner:")?, field("repo:")?))
}

/// Electron apps can run natively on Wayland, which looks sharp on Hyprland.
fn is_electron(app_dir: &Path) -> bool {
    app_dir.join("resources/app.asar").exists() || app_dir.join("chrome-sandbox").exists()
}

/// The best icon: largest PNG named after the app's icon, then an SVG, then
/// any icon at the top level.
fn find_icon(app_dir: &Path, icon_name: Option<&str>) -> Option<PathBuf> {
    let matches = |path: &Path, ext: &str| {
        path.extension().is_some_and(|e| e == ext)
            && icon_name.is_none_or(|name| path.file_stem().is_some_and(|stem| stem == name))
    };

    let hicolor = app_dir.join("usr/share/icons/hicolor");
    let mut best_png: Option<(u32, PathBuf)> = None;
    let mut svg = None;
    for size_dir in fs::read_dir(&hicolor).into_iter().flatten().flatten() {
        // Folders are named "256x256", "scalable", …
        let size: u32 = size_dir.file_name().to_string_lossy().split('x').next().and_then(|s| s.parse().ok()).unwrap_or(0);
        for icon in fs::read_dir(size_dir.path().join("apps")).into_iter().flatten().flatten() {
            let path = icon.path();
            if matches(&path, "png") && best_png.as_ref().is_none_or(|(best, _)| size > *best) {
                best_png = Some((size, path));
            } else if matches(&path, "svg") {
                svg = Some(path);
            }
        }
    }
    let top_level = || {
        fs::read_dir(app_dir).ok()?.flatten().map(|e| e.path()).find(|p| matches(p, "png") || matches(p, "svg"))
    };
    let found = best_png.map(|(_, path)| path).or(svg).or_else(top_level);

    match (found, icon_name) {
        (Some(path), _) => fs::canonicalize(&path).ok().or(Some(path)),
        // The .desktop's icon name may not match any file: try any icon.
        (None, Some(_)) => find_icon(app_dir, None),
        (None, None) => {
            let dir_icon = app_dir.join(".DirIcon");
            fs::canonicalize(dir_icon).ok()
        }
    }
}

// ─── Desktop integration ────────────────────────────────────────────────────

/// Settings remembered in the menu entry (as X- keys), so updates reinstall
/// the app exactly the same way.
#[derive(Debug, Clone)]
struct Settings {
    repo: Option<String>,
    no_sandbox: bool,
    optimized: bool,
    wayland: bool,
}

impl Settings {
    fn read(entry: &DesktopEntry) -> Self {
        Settings {
            repo: entry.get("X-AppImage-Repo").map(String::from),
            no_sandbox: entry.get("X-AppImage-Flags").is_some_and(|f| f.contains("--no-sandbox")),
            optimized: entry.get("X-AppImage-Optimized") == Some("true"),
            wayland: entry.get("X-AppImage-Wayland") != Some("false"),
        }
    }
}

/// Write the menu entry and the terminal command.
fn integrate(
    dirs: &Dirs,
    name: &str,
    app_dir: &Path,
    embedded: &DesktopEntry,
    version: &str,
    settings: &Settings,
    log: Log,
) -> Result<()> {
    fs::create_dir_all(&dirs.applications)?;
    fs::create_dir_all(&dirs.bin)?;

    let mut args = Vec::new();
    if settings.no_sandbox {
        args.push("--no-sandbox");
    }
    if settings.wayland && is_electron(app_dir) {
        args.extend(["--ozone-platform-hint=auto", "--enable-wayland-ime"]);
    }
    let args = args.join(" ");
    let apprun = app_dir.join("AppRun");
    // `env -u` clears variables leaked from other AppImages that break launches.
    let launcher = format!("env -u APPDIR -u ELECTRON_RUN_AS_NODE -u DESKTOPINTEGRATION \"{}\"", apprun.display());
    let with_args = if args.is_empty() { launcher.clone() } else { format!("{launcher} {args}") };

    let icon = find_icon(app_dir, embedded.get("Icon"));

    let mut entry = DesktopEntry::default();
    entry.set("Type", "Application");
    entry.set("Name", embedded.get("Name").unwrap_or(name));
    if let Some(comment) = embedded.get("Comment") {
        entry.set("Comment", comment);
    }
    entry.set("Exec", format!("{with_args} %U"));
    entry.set("TryExec", apprun.display().to_string());
    if let Some(icon) = &icon {
        entry.set("Icon", icon.display().to_string());
    }
    entry.set("Terminal", "false");
    let categories = dedupe_list(embedded.get("Categories").unwrap_or(""));
    entry.set("Categories", if categories.is_empty() { "Utility;".into() } else { categories });
    if let Some(class) = embedded.get("StartupWMClass") {
        entry.set("StartupWMClass", class);
    }
    for key in ["MimeType", "Keywords"] {
        if let Some(list) = embedded.get(key) {
            entry.set(key, dedupe_list(list));
        }
    }
    entry.set("X-AppImage-Version", version);
    if let Some(repo) = &settings.repo {
        entry.set("X-AppImage-Repo", repo);
    }
    if settings.no_sandbox {
        entry.set("X-AppImage-Flags", "--no-sandbox");
    }
    if settings.optimized {
        entry.set("X-AppImage-Optimized", "true");
    }
    if !settings.wayland {
        entry.set("X-AppImage-Wayland", "false");
    }
    entry.set(MARKER, MARKER_VALUE);
    fs::write(dirs.desktop_file(name), entry.to_text())?;

    // Terminal command: a tiny wrapper so it launches just like the menu does.
    let command = dirs.bin.join(name);
    let _ = fs::remove_file(&command); // may be an old symlink into the app
    fs::write(&command, format!("#!/bin/sh\n# Generated by stash\nexec {with_args} \"$@\"\n"))?;
    fs::set_permissions(&command, fs::Permissions::from_mode(0o755))?;

    refresh_menu(dirs);
    match &icon {
        Some(icon) => log(Msg::Done(format!("Icon: {}", icon.display()))),
        None => log(Msg::Warn("No icon found".into())),
    }
    log(Msg::Done(format!("Command: {name}")));
    Ok(())
}

fn refresh_menu(dirs: &Dirs) {
    if crate::util::command_exists("update-desktop-database") {
        let _ = std::process::Command::new("update-desktop-database")
            .arg("-q")
            .arg(&dirs.applications)
            .status();
    }
}

// ─── Listing, updating, removing ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub name: String,
    pub display_name: String,
    pub version: String,
    /// Path to the icon file.
    pub icon: Option<String>,
    pub repo: Option<String>,
    pub optimized: bool,
    pub no_sandbox: bool,
    pub path: String,
    /// Size on disk, in bytes.
    pub size: u64,
}

/// Names of the apps we manage: menu entry with our marker + app folder exists.
fn managed_apps(dirs: &Dirs) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(&dirs.applications)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.strip_suffix(".desktop")?.to_string();
            let marker = DesktopEntry::read(&path).get(MARKER).map(String::from);
            let ours = matches!(marker.as_deref(), Some(MARKER_VALUE | OLD_MARKER_VALUE));
            (ours && dirs.app_dir(&name).is_dir()).then_some(name)
        })
        .collect();
    names.sort();
    names
}

/// Update source for an app: stored at install time, else re-detected
/// (apps installed by the old bash version didn't store it).
fn app_repo(dirs: &Dirs, name: &str, entry: &DesktopEntry) -> Option<String> {
    entry.get("X-AppImage-Repo").map(String::from).or_else(|| detect_repo(&dirs.app_dir(name)))
}

pub fn list() -> Vec<InstalledApp> {
    let dirs = Dirs::new();
    managed_apps(&dirs)
        .into_par_iter() // folder sizes take a moment; measure them all at once
        .map(|name| {
            let entry = DesktopEntry::read(&dirs.desktop_file(&name));
            let settings = Settings::read(&entry);
            let app_dir = dirs.app_dir(&name);
            InstalledApp {
                display_name: entry.get("Name").unwrap_or(&name).to_string(),
                version: entry.get("X-AppImage-Version").unwrap_or("unknown").to_string(),
                icon: entry.get("Icon").map(String::from),
                repo: app_repo(&dirs, &name, &entry),
                optimized: settings.optimized,
                no_sandbox: settings.no_sandbox,
                path: app_dir.display().to_string(),
                size: dir_size(&app_dir),
                name,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdate {
    pub name: String,
    pub current: String,
    pub latest: String,
}

/// Apps with a newer GitHub release. All apps are checked at the same time.
pub fn check_updates() -> Vec<AppUpdate> {
    list()
        .into_par_iter()
        .filter_map(|app| {
            let latest = github::latest_version(app.repo.as_deref()?).ok()?;
            is_newer(&latest, &app.version).then_some(AppUpdate { name: app.name, current: app.version, latest })
        })
        .collect()
}

/// Update one app. Returns the new version, or None if it was up to date.
pub fn update(name: &str, log: Log) -> Result<Option<String>> {
    check_name(name)?;
    let dirs = Dirs::new();
    let entry = DesktopEntry::read(&dirs.desktop_file(name));
    if !dirs.app_dir(name).is_dir() || entry.get(MARKER).is_none() {
        bail!("'{name}' is not installed");
    }
    let Some(repo) = app_repo(&dirs, name, &entry) else {
        bail!("{name} has no update source. Reinstall it with a GitHub repo to enable updates.");
    };
    let current = entry.get("X-AppImage-Version").unwrap_or("unknown").to_string();

    log(Msg::Step(format!("Checking {repo}…")));
    let release = github::latest_release(&repo)?;
    if !is_newer(&release.version, &current) {
        log(Msg::Done(format!("{name} is up to date ({current})")));
        return Ok(None);
    }

    log(Msg::Step(format!("Updating {name}: {current} → {}", release.version)));
    let download = Download::new(&dirs.opt, &release.url)?;
    crate::http::download(&release.url, &download.file, log)?;

    // Reinstall with the same settings as before.
    let settings = Settings::read(&entry);
    let options = InstallOptions {
        name: Some(name.to_string()),
        repo: Some(repo),
        no_sandbox: settings.no_sandbox,
        optimize: settings.optimized,
        no_wayland: !settings.wayland,
        keep: true,
    };
    install_file(&dirs, &download.file, &options, Some(&release.version), log)?;
    log(Msg::Done(format!("Updated {name} to {}", release.version)));
    Ok(Some(release.version))
}

/// Update every app that has an update. Fails if any update failed.
pub fn update_all(log: Log) -> Result<()> {
    log(Msg::Step("Checking for updates…".into()));
    let updates = check_updates();
    if updates.is_empty() {
        log(Msg::Done("All apps are up to date".into()));
        return Ok(());
    }
    let mut failed = Vec::new();
    for app in updates {
        if let Err(error) = update(&app.name, log) {
            log(Msg::Warn(format!("Failed to update {}: {error:#}", app.name)));
            failed.push(app.name);
        }
    }
    if !failed.is_empty() {
        bail!("Could not update: {}", failed.join(", "));
    }
    Ok(())
}

pub fn remove(name: &str, log: Log) -> Result<()> {
    check_name(name)?;
    let dirs = Dirs::new();
    let desktop = dirs.desktop_file(name);
    let app_dir = dirs.app_dir(name);
    if !app_dir.is_dir() && !desktop.is_file() {
        bail!("'{name}' is not installed");
    }
    if desktop.is_file() && DesktopEntry::read(&desktop).get(MARKER).is_none() {
        bail!("'{name}' wasn't installed by Stash, so it's left alone");
    }

    if app_dir.is_dir() {
        fs::remove_dir_all(&app_dir).with_context(|| format!("could not delete {}", app_dir.display()))?;
    }
    let _ = fs::remove_file(&desktop);
    // Only remove the command if it's ours (our wrapper, or an old symlink).
    let command = dirs.bin.join(name);
    let is_ours = command.is_symlink()
        || fs::read_to_string(&command).is_ok_and(|text| text.contains("Generated by stash") || text.contains("Generated by appimage-install"));
    if is_ours {
        let _ = fs::remove_file(&command);
    }
    refresh_menu(&dirs);
    log(Msg::Done(format!("Removed {name}")));
    Ok(())
}

/// Start an installed app through its menu entry.
pub fn launch(name: &str) -> Result<()> {
    check_name(name)?;
    launch_detached("gtk-launch", &[name])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources() {
        assert!(matches!(Source::parse("marktext/marktext").unwrap(), Source::GitHub(r) if r == "marktext/marktext"));
        assert!(matches!(Source::parse("https://github.com/a/b.git").unwrap(), Source::GitHub(r) if r == "a/b"));
        assert!(matches!(Source::parse("https://x.org/A.AppImage").unwrap(), Source::Url(_)));
        assert!(Source::parse("not a thing").is_err());
    }

    #[test]
    fn names() {
        assert!(check_name("ai.opencode.desktop").is_ok());
        assert!(check_name("../etc").is_err());
        assert!(check_name("Big Name").is_err());
    }

    #[test]
    fn name_from_file() {
        let empty = DesktopEntry::default();
        assert_eq!(detect_name(&empty, Path::new("/x/LocalSend-1.18.2-linux-x86-64.AppImage")), "localsend");
    }

    #[test]
    fn apprun_patch() {
        let dir = std::env::temp_dir().join(format!("stash-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("AppRun"), "#!/bin/sh\nif [ -z \"$APPDIR\" ] ; then\n  APPDIR=x\nfi\nexec app\n").unwrap();
        patch_apprun(&dir);
        let patched = fs::read_to_string(dir.join("AppRun")).unwrap();
        assert!(patched.contains("APPDIR=\"$(dirname"));
        assert!(patched.contains("exec app") && !patched.contains("if ["));
        fs::remove_dir_all(dir).unwrap();
    }
}
