//! Arch Linux packages: the official repositories, Omarchy's own repository
//! ([omarchy] in pacman.conf, pkgs.omarchy.org) and the AUR (yay).
//!
//! Reading (search, lists, updates) runs directly. Anything that changes the
//! system needs root, so it runs in a terminal window; see terminal.rs.

use crate::icons::desktop_file_icon;
use crate::terminal::{CANCELLED, run_in_terminal};
use crate::util::{command_exists, output, output_lenient};
use anyhow::{Result, bail};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A package in search results or the installed list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    /// "repo" (official Arch), "omarchy" (Omarchy's repository) or "aur"
    pub source: String,
    /// Repository name: "core", "extra", "omarchy", "aur", …
    pub repo: String,
    pub installed: bool,
    /// Ships a desktop app (so "Open" makes sense).
    pub has_app: bool,
    /// App icon as a data URL.
    pub icon: Option<String>,
    pub votes: u32,
    pub out_of_date: bool,
    /// Installed size, e.g. "19.86 MiB" (installed list only).
    pub size: String,
}

/// Valid Arch package names. Everything passed to pacman/yay is checked against
/// this, which also makes names safe to put into a terminal script.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['-', '.'])
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "@._+-".contains(c))
}

fn check_names(names: &[String]) -> Result<()> {
    if names.is_empty() {
        bail!("No packages given");
    }
    if let Some(bad) = names.iter().find(|name| !is_valid_name(name)) {
        bail!("Invalid package name: {bad}");
    }
    Ok(())
}

fn lines(text: &str) -> HashSet<String> {
    text.lines().filter(|l| !l.is_empty()).map(String::from).collect()
}

fn installed_names() -> Result<HashSet<String>> {
    Ok(lines(&output("pacman", &["-Qq"])?))
}

/// Which repository each package comes from: name → "core", "extra",
/// "omarchy", … When a name is in several, the first repo in pacman.conf wins,
/// like it does for pacman (Omarchy lists its own repo first).
fn sync_repos() -> HashMap<String, String> {
    let mut repos = HashMap::new();
    for line in output_lenient("pacman", &["-Sl"]).lines() {
        let mut words = line.split_whitespace();
        if let (Some(repo), Some(name)) = (words.next(), words.next()) {
            repos.entry(name.to_string()).or_insert_with(|| repo.to_string());
        }
    }
    repos
}

/// "omarchy" for Omarchy's repository, "aur" for the AUR, "repo" for official Arch.
fn source_of(repo: &str) -> &'static str {
    match repo {
        "omarchy" => "omarchy",
        "aur" => "aur",
        _ => "repo",
    }
}

/// Packages not in any repository: installed from the AUR (or by hand).
fn foreign_names() -> HashSet<String> {
    lines(&output_lenient("pacman", &["-Qqm"]))
}

/// yay builds AUR packages with whatever Go is on PATH. mise's GOROOT/GOBIN
/// (Omarchy sets mise up) would make Go packages fail to build, so drop them,
/// like omarchy-update does.
const AUR_ENV: &str = "unset GOROOT GOBIN; ";

// ─── Searching ──────────────────────────────────────────────────────────────

/// Search the repos (official and Omarchy's) and the AUR at the same time.
pub fn search(query: &str) -> Result<Vec<Package>> {
    let query = query.trim().to_lowercase();
    if query.len() < 2 {
        return Ok(Vec::new());
    }

    let (repo, aur) = rayon::join(
        || search_repos(&query),
        || search_aur(&query).unwrap_or_default(), // offline or AUR down: repo results still work
    );
    let mut results = repo?;

    // A package in both places is the repo one (that's what pacman would install).
    let repo_names: HashSet<String> = results.iter().map(|p| p.name.clone()).collect();
    results.extend(aur.into_iter().filter(|p| !repo_names.contains(&p.name)));
    results.sort_by_key(|p| std::cmp::Reverse(rank(p, &query)));
    results.truncate(80);

    mark_installed(&mut results)?;
    Ok(results)
}

/// Higher is better: exact name, then name match, then repos over AUR, then votes.
fn rank(pkg: &Package, query: &str) -> u32 {
    let mut score = 0;
    if pkg.name == query {
        score += 1000;
    } else if pkg.name.starts_with(query) {
        score += 400;
    } else if pkg.name.contains(query) {
        score += 200;
    }
    if pkg.source != "aur" {
        score += 100;
    }
    score + pkg.votes.min(5000) / 50
}

fn search_repos(query: &str) -> Result<Vec<Package>> {
    // pacman -Ss takes a regex; escape it so queries like "c++" work.
    let escaped: String = query
        .chars()
        .flat_map(|c| if ".*+?^${}()|[]\\".contains(c) { vec!['\\', c] } else { vec![c] })
        .collect();
    let text = output_lenient("pacman", &["-Ss", &escaped]);

    // Output comes in pairs of lines:
    //   extra/firefox 131.0-1 [installed]
    //       Fast, Private & Safe Web Browser
    let mut results = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let mut words = line.split_whitespace();
        let (Some(full_name), Some(version)) = (words.next(), words.next()) else { continue };
        let Some((repo, name)) = full_name.split_once('/') else { continue };
        let description = lines.next_if(|l| l.starts_with(' ')).unwrap_or("").trim();
        results.push(Package {
            name: name.into(),
            version: version.into(),
            description: description.into(),
            source: source_of(repo).into(),
            repo: repo.into(),
            ..empty_package()
        });
    }
    Ok(results)
}

#[derive(Deserialize)]
struct AurResponse {
    results: Vec<AurPackage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AurPackage {
    name: String,
    version: String,
    description: Option<String>,
    num_votes: u32,
    popularity: f64,
    out_of_date: Option<u64>,
}

fn search_aur(query: &str) -> Result<Vec<Package>> {
    let url = format!("https://aur.archlinux.org/rpc/v5/search/{}?by=name-desc", encode(query));
    let mut response: AurResponse = crate::http::get_json(&url, Duration::from_secs(10))?;
    response.results.sort_by(|a, b| b.popularity.total_cmp(&a.popularity));
    Ok(response
        .results
        .into_iter()
        .take(50)
        .map(|p| Package {
            name: p.name,
            version: p.version,
            description: p.description.unwrap_or_default(),
            source: "aur".into(),
            repo: "aur".into(),
            votes: p.num_votes,
            out_of_date: p.out_of_date.is_some(),
            ..empty_package()
        })
        .collect())
}

/// Percent-encode a search term for a URL path.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn empty_package() -> Package {
    Package {
        name: String::new(),
        version: String::new(),
        description: String::new(),
        source: String::new(),
        repo: String::new(),
        installed: false,
        has_app: false,
        icon: None,
        votes: 0,
        out_of_date: false,
        size: String::new(),
    }
}

// ─── What's installed ───────────────────────────────────────────────────────

/// Set `installed`, `has_app` and `icon` on a list of packages.
fn mark_installed(packages: &mut [Package]) -> Result<()> {
    let installed = installed_names()?;
    let names: Vec<String> = packages.iter().map(|p| p.name.clone()).filter(|n| installed.contains(n)).collect();
    let apps = app_icons(&names);
    for pkg in packages {
        pkg.installed = installed.contains(&pkg.name);
        if let Some(icon) = apps.get(&pkg.name) {
            pkg.has_app = true;
            pkg.icon = icon.clone();
        }
    }
    Ok(())
}

/// Of these installed packages, which ship a desktop app (a .desktop file)?
/// Command-line tools like docker don't, so the GUI hides "Open" for them.
/// Returns package name → icon.
fn app_icons(names: &[String]) -> HashMap<String, Option<String>> {
    if names.is_empty() {
        return HashMap::new();
    }
    let args: Vec<&str> = std::iter::once("-Ql").chain(names.iter().map(String::as_str)).collect();
    let text = output_lenient("pacman", &args);

    // "firefox /usr/share/applications/firefox.desktop"
    let mut desktop_files: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if let Some((name, file)) = line.split_once(' ')
            && file.starts_with("/usr/share/applications/")
            && file.ends_with(".desktop")
            && !file[24..].contains('/')
        {
            desktop_files.entry(name.to_string()).or_insert_with(|| file.to_string());
        }
    }
    desktop_files
        .into_par_iter()
        .map(|(name, file)| (name, desktop_file_icon(Path::new(&file))))
        .collect()
}

/// Install state for the Discover catalog: name → (installed, has_app, icon).
pub fn status(names: &[String]) -> Result<HashMap<String, Package>> {
    let mut packages: Vec<Package> =
        names.iter().map(|name| Package { name: name.clone(), ..empty_package() }).collect();
    mark_installed(&mut packages)?;
    Ok(packages.into_iter().map(|p| (p.name.clone(), p)).collect())
}

/// Packages you installed on purpose (not pulled in as dependencies).
pub fn list_installed() -> Result<Vec<Package>> {
    let text = output("pacman", &["-Qei"])?;
    let repos = sync_repos();

    // `pacman -Qi` prints "Key   : value" blocks separated by blank lines.
    let mut packages: Vec<Package> = text
        .split("\n\n")
        .filter_map(|block| {
            let info = parse_info_block(block);
            let name = info.get("Name")?.clone();
            let field = |key: &str| info.get(key).cloned().unwrap_or_default();
            // Not in any repo: it came from the AUR.
            let repo = repos.get(&name).map_or("aur", String::as_str);
            Some(Package {
                source: source_of(repo).into(),
                repo: repo.into(),
                version: field("Version"),
                description: field("Description"),
                size: field("Installed Size"),
                installed: true,
                name,
                ..empty_package()
            })
        })
        .collect();

    let names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    let apps = app_icons(&names);
    for pkg in &mut packages {
        if let Some(icon) = apps.get(&pkg.name) {
            pkg.has_app = true;
            pkg.icon = icon.clone();
        }
    }
    Ok(packages)
}

fn parse_info_block(block: &str) -> HashMap<String, String> {
    let mut info = HashMap::new();
    let mut last_key = String::new();
    for line in block.lines() {
        if let Some((key, value)) = line.split_once(" : ")
            && !line.starts_with(' ')
        {
            last_key = key.trim().to_string();
            info.insert(last_key.clone(), value.trim().to_string());
        } else if let Some(value) = info.get_mut(&last_key) {
            // A long value wrapped onto the next line.
            value.push(' ');
            value.push_str(line.trim());
        }
    }
    info
}

// ─── Updates & maintenance ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct PackageUpdate {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub source: String,
}

/// The last check, reused for a little while: the window asks from several
/// pages at once (sidebar badge, Installed, Updates).
static LAST_CHECK: Mutex<Option<(Instant, Vec<PackageUpdate>)>> = Mutex::new(None);
const REUSE_CHECK_FOR: Duration = Duration::from_secs(60);

/// Forget the last check, after something was installed, updated or removed.
pub fn forget_last_check() {
    *LAST_CHECK.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Pending system updates. `checkupdates` (from pacman-contrib) syncs a private
/// copy of the package database, so this needs no root and changes nothing.
///
/// Checks run one at a time: two `checkupdates` at once share that private
/// database and one of them fails, which would look like "no updates".
pub fn check_updates() -> Vec<PackageUpdate> {
    let mut last = LAST_CHECK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((when, updates)) = last.as_ref()
        && when.elapsed() < REUSE_CHECK_FOR
    {
        return updates.clone();
    }
    let updates = find_updates();
    *last = Some((Instant::now(), updates.clone()));
    updates
}

/// Run `checkupdates`. It exits with 2 when there are no updates and 1 when it
/// failed, for example because something else (like Omarchy's own update
/// check) was using its database at that moment; then try again.
fn run_checkupdates() -> String {
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(2));
        }
        match Command::new("checkupdates").arg("--nocolor").stdin(Stdio::null()).stderr(Stdio::null()).output() {
            Ok(out) if out.status.code() != Some(1) => return String::from_utf8_lossy(&out.stdout).into_owned(),
            Ok(_) => continue,
            Err(_) => return String::new(), // pacman-contrib not installed
        }
    }
    String::new()
}

fn find_updates() -> Vec<PackageUpdate> {
    let parse = |text: String, source: &str| -> Vec<PackageUpdate> {
        text.lines()
            .filter_map(|line| {
                // "firefox 131.0-1 -> 132.0-1"
                let parts: Vec<&str> = line.split_whitespace().collect();
                match parts.as_slice() {
                    [name, current, "->", latest, ..] => Some(PackageUpdate {
                        name: name.to_string(),
                        current: current.to_string(),
                        latest: latest.to_string(),
                        source: source.into(),
                    }),
                    _ => None,
                }
            })
            .collect()
    };
    let (repo, aur) = rayon::join(
        run_checkupdates,
        || if command_exists("yay") { output_lenient("yay", &["-Qua"]) } else { String::new() },
    );
    let repos = sync_repos();
    let mut updates = parse(repo, "repo");
    for update in &mut updates {
        update.source = repos.get(&update.name).map_or("repo", |r| source_of(r)).into();
    }
    updates.extend(parse(aur, "aur"));
    updates
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub orphans: Vec<String>,
    pub cache_bytes: u64,
    pub explicit_count: usize,
    pub total_count: usize,
    pub has_yay: bool,
    /// Running Omarchy: system updates go through omarchy-update.
    pub is_omarchy: bool,
}

pub fn system_info() -> SystemInfo {
    let count = |text: String| text.lines().filter(|l| !l.is_empty()).count();
    SystemInfo {
        orphans: output_lenient("pacman", &["-Qdtq"]).lines().map(String::from).collect(),
        cache_bytes: crate::util::dir_size(Path::new("/var/cache/pacman/pkg")),
        explicit_count: count(output_lenient("pacman", &["-Qqe"])),
        total_count: count(output_lenient("pacman", &["-Qq"])),
        has_yay: command_exists("yay"),
        is_omarchy: command_exists("omarchy-update"),
    }
}

// ─── Changing the system (in a terminal) ────────────────────────────────────

/// Run in a terminal and turn the exit code into a Result.
fn in_terminal(title: &str, script: &str) -> Result<()> {
    let result = run_in_terminal(title, script);
    forget_last_check(); // even a failed run may have changed something
    match result? {
        0 => Ok(()),
        CANCELLED => bail!("Cancelled"),
        code => bail!("Failed (exit code {code}), see the terminal for details"),
    }
}

/// Install packages. Names in the official repos go to pacman; the rest are
/// assumed to be AUR packages and go to yay (which handles repo ones too).
pub fn install(names: &[String]) -> Result<()> {
    check_names(names)?;
    let in_repos = lines(&output("pacman", &["-Slq"])?);
    let from_aur: Vec<&String> = names.iter().filter(|n| !in_repos.contains(*n)).collect();
    let list = names.join(" ");

    if from_aur.is_empty() {
        return in_terminal(&format!("Installing {list}"), &format!("sudo pacman -S --needed --noconfirm {list}"));
    }
    if !command_exists("yay") {
        bail!("{} is in the AUR, which needs yay", from_aur[0]);
    }
    in_terminal(&format!("Installing {list}"), &format!("{AUR_ENV}yay -S --needed --noconfirm {list}"))
}

/// Update individual packages.
///
/// Only AUR packages can be updated one at a time. Packages from the official
/// and Omarchy repositories are built against each other, and Arch doesn't
/// support upgrading just some of them ("partial upgrades" can leave apps
/// unable to start), so those go through a full system update instead.
pub fn update(names: &[String]) -> Result<()> {
    check_updatable(names)?;
    let list = names.join(" ");
    // -S without --needed rebuilds even if pacman thinks it's current, which is
    // what "Update" should do for -git packages too.
    in_terminal(&format!("Updating {list}"), &format!("{AUR_ENV}yay -S --noconfirm {list}"))
}

/// Errors unless every package is an installed AUR package and yay is there.
pub fn check_updatable(names: &[String]) -> Result<()> {
    check_names(names)?;
    let foreign = foreign_names();
    if let Some(name) = names.iter().find(|n| !foreign.contains(*n)) {
        let installed = installed_names()?;
        if !installed.contains(name) {
            bail!("{name} is not installed");
        }
        bail!("{name} comes from a repository, so it's updated with the rest of the system (Update system)");
    }
    if !command_exists("yay") {
        bail!("Updating AUR packages needs yay");
    }
    Ok(())
}

/// Remove packages, their config backups and dependencies nothing else uses.
pub fn remove(names: &[String]) -> Result<()> {
    check_names(names)?;
    let list = names.join(" ");
    in_terminal(&format!("Removing {list}"), &format!("sudo pacman -Rns --noconfirm {list}"))
}

/// Full system upgrade. On Omarchy use its updater (snapshot first, migrations…).
pub fn upgrade_system() -> Result<()> {
    if command_exists("omarchy-update") {
        in_terminal("Updating Omarchy", "omarchy-update")
    } else if command_exists("yay") {
        in_terminal("Updating system", &format!("{AUR_ENV}yay -Syu"))
    } else {
        in_terminal("Updating system", "sudo pacman -Syu")
    }
}

pub fn remove_orphans() -> Result<()> {
    in_terminal(
        "Removing unused dependencies",
        r#"orphans=$(pacman -Qdtq); [[ -z "$orphans" ]] && echo "Nothing to remove" || sudo pacman -Rns --noconfirm $orphans"#,
    )
}

/// Keep the two newest versions of installed packages; drop the rest.
pub fn clean_cache() -> Result<()> {
    in_terminal(
        "Cleaning package cache",
        "sudo paccache -rk2 && sudo paccache -ruk0 && { command -v yay >/dev/null && yay -Sc --aur --noconfirm || true; }",
    )
}

/// Open an installed package's app through the .desktop file it ships.
pub fn launch(name: &str) -> Result<()> {
    check_names(&[name.to_string()])?;
    let files = output("pacman", &["-Qlq", name])?;
    let Some(desktop) = files.lines().find(|f| f.starts_with("/usr/share/applications/") && f.ends_with(".desktop"))
    else {
        bail!("{name} has no app to open (it may be a command-line tool)");
    };
    let id = desktop.rsplit('/').next().unwrap_or(desktop);
    crate::util::launch_detached("gtk-launch", &[id])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_names() {
        assert!(is_valid_name("visual-studio-code-bin"));
        assert!(is_valid_name("libc++"));
        assert!(!is_valid_name("rm -rf"));
        assert!(!is_valid_name("--noconfirm"));
        assert!(!is_valid_name("$(evil)"));
    }

    #[test]
    fn sources() {
        assert_eq!(source_of("omarchy"), "omarchy");
        assert_eq!(source_of("extra"), "repo");
        assert_eq!(source_of("multilib"), "repo");
        assert_eq!(source_of("aur"), "aur");
    }

    #[test]
    fn info_block() {
        let info = parse_info_block("Name            : firefox\nDescription     : Fast\n                  browser\n");
        assert_eq!(info["Name"], "firefox");
        assert_eq!(info["Description"], "Fast browser");
    }
}
