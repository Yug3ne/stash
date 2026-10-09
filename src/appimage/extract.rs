//! Unpacking an AppImage.
//!
//! A (type 2) AppImage is a small ELF program followed by a squashfs image
//! holding the app. We read that image directly with the `backhand` crate,
//! using all CPU cores, without running the AppImage or needing FUSE.

use anyhow::{Context, Result, bail};
use backhand::{FilesystemReader, InnerNode};
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::process::{Command, Stdio};

/// Extract `appimage` into the new directory `dest`.
pub fn extract(appimage: &Path, dest: &Path) -> Result<()> {
    let error = match extract_squashfs(appimage, dest) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    // Rare formats (old type-1 AppImages, unusual compression): let the
    // AppImage unpack itself.
    let _ = fs::remove_dir_all(dest);
    extract_by_running(appimage, dest).with_context(|| format!("could not read the AppImage ({error:#})"))
}

/// Where the squashfs image starts: right after the ELF program, whose size is
/// section-header offset + (section-header size × count), read from its header.
fn squashfs_offset(file: &mut File) -> Result<u64> {
    let mut header = [0u8; 64];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;
    if &header[..4] != b"\x7fELF" {
        bail!("not an AppImage (no ELF header)");
    }
    let u16_at = |at: usize| u16::from_le_bytes([header[at], header[at + 1]]) as u64;
    let u32_at = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap()) as u64;
    let u64_at = |at: usize| u64::from_le_bytes(header[at..at + 8].try_into().unwrap());

    Ok(match header[4] {
        2 => u64_at(0x28) + u16_at(0x3A) * u16_at(0x3C), // 64-bit ELF
        _ => u32_at(0x20) + u16_at(0x2E) * u16_at(0x30), // 32-bit ELF
    })
}

fn extract_squashfs(appimage: &Path, dest: &Path) -> Result<()> {
    let mut file = File::open(appimage)?;
    let offset = squashfs_offset(&mut file)?;
    let image = FilesystemReader::from_reader_with_offset(BufReader::new(file), offset)?;
    let nodes: Vec<_> = image.files().collect();

    // 1. Folders first, so files have somewhere to go.
    fs::create_dir_all(dest)?;
    for node in &nodes {
        if let InnerNode::Dir(_) = node.inner {
            fs::create_dir_all(dest.join(relative(&node.fullpath)))?;
        }
    }

    // 2. Files, decompressed in parallel.
    nodes.par_iter().try_for_each(|node| -> Result<()> {
        let path = dest.join(relative(&node.fullpath));
        match &node.inner {
            InnerNode::File(file) => {
                let mut out = File::create(&path)?;
                std::io::copy(&mut image.file(file).reader(), &mut out)
                    .with_context(|| format!("could not extract {}", path.display()))?;
                out.set_permissions(fs::Permissions::from_mode(node.header.permissions.into()))?;
            }
            InnerNode::Symlink(link) => symlink(&link.link, &path)?,
            _ => {} // folders done above; devices/pipes don't belong in apps
        }
        Ok(())
    })?;

    // 3. Folder permissions last, in case some are read-only.
    for node in &nodes {
        if let InnerNode::Dir(_) = node.inner {
            let mode = u32::from(node.header.permissions) | 0o700; // always keep it ours to manage
            fs::set_permissions(dest.join(relative(&node.fullpath)), fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(())
}

/// "/usr/bin/app" → "usr/bin/app" (so it's joined under the destination).
fn relative(path: &Path) -> &Path {
    path.strip_prefix("/").unwrap_or(path)
}

/// Fallback: run `<appimage> --appimage-extract`, which unpacks into
/// ./squashfs-root, then move that into place.
fn extract_by_running(appimage: &Path, dest: &Path) -> Result<()> {
    let work = dest.with_extension("work");
    fs::create_dir_all(&work)?;
    fs::set_permissions(appimage, fs::Permissions::from_mode(0o755))?;
    let status = Command::new(appimage)
        .arg("--appimage-extract")
        .current_dir(&work)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let result = match status {
        Ok(status) if status.success() => fs::rename(work.join("squashfs-root"), dest).map_err(Into::into),
        _ => Err(anyhow::anyhow!("extraction failed")),
    };
    let _ = fs::remove_dir_all(&work);
    result
}
