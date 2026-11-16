# Stash

Install apps on [Omarchy](https://omarchy.org) (or any Arch Linux) from one place. Stash is a single Rust program: run `stash` to open the app, or `stash <command>` in a terminal.

- **AppImages**: installed as real apps. Each one is extracted once, with a menu entry, an icon, a terminal command and one-click updates from GitHub.
- **Arch packages**: search the official Arch repos, Omarchy's own package repository and the AUR, then install, open, update or remove.
- **Essentials**: a hand-picked catalog of popular apps (browsers, editors, chat, media, games…).
- **Updates**: system packages, AUR packages and AppImages in one list, each AUR package and AppImage updatable on its own.
- **Maintenance**: remove unused dependencies and clean the package cache.

## Install

```bash
git clone https://github.com/Yug3ne/stash.git
cd stash
./install.sh             # add --notify for a daily AppImage update check
```

This builds Stash, puts `stash` in `~/.local/bin`, and adds **Stash** to your app launcher. To update Stash itself: `git pull && ./install.sh`.

Building needs `rust` and `webkit2gtk-4.1` (`sudo pacman -S rust webkit2gtk-4.1`). Omarchy already has WebKitGTK. Stash itself has no runtime dependencies beyond what Arch ships: no Electron, Node, FUSE or squashfs-tools.

## Why extract AppImages?

A normal AppImage mounts itself through FUSE on every launch and decompresses its files as they're read. Stash extracts it once instead:

| | AppImage (FUSE) | Stash |
|---|---|---|
| Launch speed | Slower (mount + decompress) | Fast (plain files on disk) |
| Menu entry & icon | Only with extra tools | Always |
| Terminal command | No | `~/.local/bin/<name>` |
| Updates | Manual | From GitHub releases |
| Electron apps on Wayland | Often blurry (XWayland) | Native Wayland flags added |

Extraction reads the AppImage's squashfs image directly in Rust, on all CPU cores, without running the AppImage. A 63 MB AppImage extracts in about half a second.

## The app

Run `stash` or open it from the launcher.

| Page | What it does |
|---|---|
| **Discover** | Curated essentials, plus search across the Arch repos, the Omarchy repo and the AUR. |
| **AppImages** | Drop an `.AppImage` on the window, pick one, or paste a GitHub repo (`owner/repo`) or a download link. Lists installed AppImages with Open, Update, Files and Remove. |
| **Installed** | Everything you installed yourself: packages (not dependencies) and AppImages. Filter by Official, Omarchy, AUR, AppImages or Updates. Every row has Open, Update (when there is one) and Remove. |
| **Updates** | Everything with an update. AUR packages and AppImages each have their own Update button. "Update system" runs `omarchy-update` on Omarchy (snapshot first), otherwise `yay -Syu`. |
| **Maintenance** | Unused dependencies, and the package cache size with a cleanup button. |

Anything that needs root (installing or removing packages, cleaning the cache) opens a terminal window, like Omarchy's own menus do. You type your password into the real `sudo` and can see everything pacman prints. Stash waits for the terminal to finish, then refreshes.

The app uses the colors of your current Omarchy theme.

### On Omarchy

Omarchy adds its own pacman repository, `[omarchy]` (pkgs.omarchy.org), with apps like Brave, Cursor, Zen, Typora and 1Password. Stash labels these **omarchy**, installs them with pacman (no AUR build), and lists them under their own tab. The AUR is still available through yay, which Omarchy installs.

Packages from a repository (official Arch or Omarchy) can't be updated one at a time: Arch only supports upgrading them all together, and updating one alone can stop other apps from starting. Their Update button explains this and offers `omarchy-update`, which is what Omarchy's own menu runs. AUR packages and AppImages update individually.

## The command line

```bash
stash install ~/Downloads/MyApp.AppImage          # install a file
stash install https://example.com/MyApp.AppImage  # download and install
stash install marktext/marktext                   # latest release from GitHub

stash list                       # installed AppImages (--json too)
stash updates                    # which have updates (--json, --notify)
stash update marktext            # update one
stash update                     # update all
stash remove marktext            # uninstall

stash pkg search obsidian        # search Arch + Omarchy repos and the AUR
stash pkg install obsidian       # pacman, or yay for AUR packages
stash pkg updates                # packages with updates (official, omarchy, aur)
stash pkg update android-studio  # update one AUR package
stash pkg remove obsidian
```

Install options:

| Option | Meaning |
|---|---|
| `--name <name>` | Command and folder name (default: detected from the app) |
| `--repo <owner/repo>` | GitHub repo to update from. Detected automatically for most Electron apps. |
| `--optimize` | Strip debug symbols and remove unused languages/docs (smaller, faster to load) |
| `--no-sandbox` | Launch with `--no-sandbox` (a few Electron apps need it) |
| `--no-wayland` | Don't add native-Wayland flags to Electron apps |
| `--keep` | Keep the original `.AppImage` (it's deleted by default) |

Settings like `--optimize` and `--no-sandbox` are remembered and reapplied on every update. Downloads that stall resume where they stopped. Set `GITHUB_TOKEN` if you hit GitHub's rate limit of 60 requests per hour.

## Where things go

| What | Where |
|---|---|
| Extracted app | `~/.local/opt/<name>/` |
| Menu entry (and Stash's settings for the app) | `~/.local/share/applications/<name>.desktop` |
| Command | `~/.local/bin/<name>` |

Apps installed by the older `appimage-install` script are recognized and can be updated and removed by Stash.

## How the code is organized

```
src/
  main.rs              `stash` → app window, `stash <command>` → command line
  cli.rs               Command-line commands and output
  gui.rs               The window, and every action its pages can call
  appimage/
    mod.rs             Install, list, update, remove AppImages
    extract.rs         Unpack the squashfs image inside an AppImage
    desktop.rs         Read/write .desktop menu entries
    optimize.rs        --optimize: strip binaries, drop unused languages
    github.rs          Find the latest AppImage in a GitHub release
  packages.rs          pacman (Arch + Omarchy repos) / AUR: search, lists, install, updates
  terminal.rs          Run sudo commands in a terminal window
  http.rs              JSON requests and resumable downloads
  icons.rs, theme.rs   App icons, Omarchy theme colors
  util.rs              Small shared helpers
  catalog.json         The "Essentials" list (easy to edit)
ui/                    The pages: plain HTML/CSS/JS, no framework or build step
  api.js               What the pages can ask Rust to do
  app.js, ui.js        Sidebar, shared UI helpers
  views/*.js           One file per page
```

The window is [Tauri](https://tauri.app): the pages render in the system's WebKitGTK, and they can only call the functions in `src/gui.rs`, which validate every package and app name before running anything. The command line and the app call the same Rust functions.

Development: `cargo run` opens the app, `cargo run -- list` runs a command, `cargo test` runs the tests. The pages are compiled into the binary, so rebuild after editing `ui/`.

## License

MIT
