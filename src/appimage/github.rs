//! Finding the newest AppImage in a GitHub repository's releases.

use anyhow::{Result, bail};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
}

pub struct Release {
    /// Version from the release tag, without a leading "v".
    pub version: String,
    /// Download link of the AppImage for this computer's CPU.
    pub url: String,
}

fn fetch_latest(repo: &str) -> Result<ApiRelease> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    crate::http::get_json(&url, Duration::from_secs(20))
}

/// Just the latest version number (used when checking many apps at once).
pub fn latest_version(repo: &str) -> Result<String> {
    Ok(fetch_latest(repo)?.tag_name.trim_start_matches('v').to_string())
}

/// The latest release's AppImage, preferring one built for this CPU.
pub fn latest_release(repo: &str) -> Result<Release> {
    let release = fetch_latest(repo)?;
    let appimages: Vec<&ApiAsset> =
        release.assets.iter().filter(|a| a.name.to_lowercase().ends_with(".appimage")).collect();

    let lower = |a: &&ApiAsset| a.name.to_lowercase();
    let (this_cpu, other_cpus): (&[&str], &[&str]) = if cfg!(target_arch = "aarch64") {
        (&["aarch64", "arm64"], &["x86_64", "x86-64", "amd64", "x64", "armv7", "i386"])
    } else {
        (&["x86_64", "x86-64", "amd64", "x64"], &["aarch64", "arm64", "armv7", "armhf", "i386", "i686"])
    };

    // Best: names that mention our CPU. Fallback: names that don't mention any CPU.
    let asset = appimages
        .iter()
        .find(|a| this_cpu.iter().any(|arch| lower(a).contains(arch)))
        .or_else(|| appimages.iter().find(|a| !other_cpus.iter().any(|arch| lower(a).contains(arch))));

    match asset {
        Some(asset) => Ok(Release {
            version: release.tag_name.trim_start_matches('v').to_string(),
            url: asset.browser_download_url.clone(),
        }),
        None => bail!("No AppImage for this computer in the latest release of {repo}"),
    }
}
