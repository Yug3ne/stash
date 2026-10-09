//! The app window. The pages in ui/ are plain HTML/CSS/JS shown in a native
//! window (WebKitGTK, which Omarchy already has). They can't touch the system
//! themselves; they call the `#[tauri::command]` functions below, which use the
//! same code as the command line.

use crate::appimage::{self, InstallOptions, Source};
use crate::util::Msg;
use crate::{icons, packages, theme};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

/// Errors reach the page as plain text.
type CmdResult<T> = Result<T, String>;

/// Run slow work (commands, downloads, extraction) off the UI thread.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

/// Progress messages go to the page's activity log as "task" events.
fn logger(app: &AppHandle) -> impl Fn(Msg) + Sync + use<> {
    let app = app.clone();
    move |msg| {
        let _ = app.emit("task", msg);
    }
}

// ─── General ────────────────────────────────────────────────────────────────

#[tauri::command]
fn theme() -> Option<std::collections::HashMap<String, String>> {
    theme::read_theme()
}

#[tauri::command]
fn catalog() -> serde_json::Value {
    serde_json::from_str(include_str!("catalog.json")).expect("catalog.json is valid JSON")
}

/// Open a folder in the file manager.
#[tauri::command]
fn show_folder(path: String) -> CmdResult<()> {
    if !Path::new(&path).is_absolute() || !Path::new(&path).is_dir() {
        return Err("Not a folder".into());
    }
    crate::util::launch_detached("xdg-open", &[&path]).map_err(|e| e.to_string())
}

/// Open a web page in the browser.
#[tauri::command]
fn open_url(url: String) -> CmdResult<()> {
    if !url.starts_with("https://") {
        return Err("Only https links can be opened".into());
    }
    crate::util::launch_detached("xdg-open", &[&url]).map_err(|e| e.to_string())
}

// ─── AppImages ──────────────────────────────────────────────────────────────

#[tauri::command]
async fn appimages_list() -> CmdResult<Vec<appimage::InstalledApp>> {
    blocking(|| {
        // Swap icon paths for data URLs; the page can't read files.
        Ok(appimage::list()
            .into_iter()
            .map(|mut app| {
                app.icon = app.icon.and_then(|icon| icons::data_url(Path::new(&icon)));
                app
            })
            .collect())
    })
    .await
}

#[tauri::command]
async fn appimages_check_updates() -> CmdResult<Vec<appimage::AppUpdate>> {
    blocking(|| Ok(appimage::check_updates())).await
}

#[tauri::command]
async fn appimages_install(app: AppHandle, source: String, options: InstallOptions) -> CmdResult<String> {
    let log = logger(&app);
    blocking(move || appimage::install(Source::parse(&source)?, &options, &log)).await
}

#[tauri::command]
async fn appimages_update(app: AppHandle, name: String) -> CmdResult<Option<String>> {
    let log = logger(&app);
    blocking(move || appimage::update(&name, &log)).await
}

#[tauri::command]
async fn appimages_update_all(app: AppHandle) -> CmdResult<()> {
    let log = logger(&app);
    blocking(move || appimage::update_all(&log)).await
}

#[tauri::command]
async fn appimages_remove(app: AppHandle, name: String) -> CmdResult<()> {
    let log = logger(&app);
    blocking(move || appimage::remove(&name, &log)).await
}

#[tauri::command]
fn appimages_launch(name: String) -> CmdResult<()> {
    appimage::launch(&name).map_err(|e| e.to_string())
}

/// Show a file picker for .AppImage files. Returns the chosen path, if any.
#[tauri::command]
async fn pick_appimage(app: AppHandle) -> CmdResult<Option<String>> {
    blocking(move || {
        let downloads = app.path().download_dir().ok();
        let mut dialog = app.dialog().file().set_title("Choose an AppImage").add_filter("AppImage", &["AppImage", "appimage"]);
        if let Some(dir) = downloads {
            dialog = dialog.set_directory(dir);
        }
        Ok(dialog.blocking_pick_file().and_then(|file| file.into_path().ok()).map(|p| p.display().to_string()))
    })
    .await
}

// ─── Arch packages ──────────────────────────────────────────────────────────

#[tauri::command]
async fn packages_search(query: String) -> CmdResult<Vec<packages::Package>> {
    blocking(move || packages::search(&query)).await
}

#[tauri::command]
async fn packages_installed() -> CmdResult<Vec<packages::Package>> {
    blocking(packages::list_installed).await
}

#[tauri::command]
async fn packages_status(names: Vec<String>) -> CmdResult<std::collections::HashMap<String, packages::Package>> {
    blocking(move || packages::status(&names)).await
}

#[tauri::command]
async fn packages_check_updates() -> CmdResult<Vec<packages::PackageUpdate>> {
    blocking(|| Ok(packages::check_updates())).await
}

#[tauri::command]
async fn system_info() -> CmdResult<packages::SystemInfo> {
    blocking(|| Ok(packages::system_info())).await
}

#[tauri::command]
async fn packages_install(names: Vec<String>) -> CmdResult<()> {
    blocking(move || packages::install(&names)).await
}

#[tauri::command]
async fn packages_remove(names: Vec<String>) -> CmdResult<()> {
    blocking(move || packages::remove(&names)).await
}

#[tauri::command]
fn packages_launch(name: String) -> CmdResult<()> {
    packages::launch(&name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn packages_upgrade() -> CmdResult<()> {
    blocking(packages::upgrade_system).await
}

#[tauri::command]
async fn packages_remove_orphans() -> CmdResult<()> {
    blocking(packages::remove_orphans).await
}

#[tauri::command]
async fn packages_clean_cache() -> CmdResult<()> {
    blocking(packages::clean_cache).await
}

// ─── Window ─────────────────────────────────────────────────────────────────

/// "#1a1b26" → Color, for the window background shown before the page loads.
fn parse_color(hex: &str) -> Option<tauri::window::Color> {
    let hex = hex.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some(tauri::window::Color(byte(0)?, byte(2)?, byte(4)?, 255))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let background = theme::read_theme()
                .and_then(|colors| parse_color(colors.get("background")?))
                .unwrap_or(tauri::window::Color(16, 16, 20, 255));
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Stash")
                .inner_size(1180.0, 800.0)
                .min_inner_size(760.0, 520.0)
                .background_color(background)
                .build()?;
            Ok(())
        })
        // Development aid (debug builds only): STASH_UI_SCRIPT=test.js runs that
        // script once the page has loaded, to click through the UI automatically.
        .on_page_load(|_webview, _payload| {
            #[cfg(debug_assertions)]
            if _payload.event() == tauri::webview::PageLoadEvent::Finished
                && let Some(script) = std::env::var_os("STASH_UI_SCRIPT").and_then(|f| std::fs::read_to_string(f).ok())
            {
                let _ = _webview.eval(script);
            }
        })
        .invoke_handler(tauri::generate_handler![
            theme,
            catalog,
            show_folder,
            open_url,
            appimages_list,
            appimages_check_updates,
            appimages_install,
            appimages_update,
            appimages_update_all,
            appimages_remove,
            appimages_launch,
            pick_appimage,
            packages_search,
            packages_installed,
            packages_status,
            packages_check_updates,
            system_info,
            packages_install,
            packages_remove,
            packages_launch,
            packages_upgrade,
            packages_remove_orphans,
            packages_clean_cache,
        ])
        .run(tauri::generate_context!())
        .expect("could not start the Stash window");
}
