# appimage-install

Extract and install AppImages properly — no FUSE overhead, no tmpfs RAM waste, with full desktop integration.



## Why use this?

AppImages use FUSE to mount a compressed filesystem into `/tmp` (often tmpfs = RAM) every time you run them. On a 16GB laptop running heavy workloads, this wastes hundreds of MB of RAM per app and adds latency to every file read through the FUSE layer.

`appimage-install` extracts the AppImage once, sets up the desktop entry with the correct icon, and creates a terminal command — all in one step.

| | AppImage (FUSE) | appimage-install |
|---|---|---|
| Launch speed | Slow (decompress + FUSE mount) | Fast (direct disk read) |
| RAM overhead | ~150MB+ per app in tmpfs | Zero |
| Desktop icon | Depends on AppImageLauncher | Always works |
| Terminal command | None | `~/.local/bin/<name>` |

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/Yug3ne/appimage-installer/main/appimage-install -o ~/.local/bin/appimage-install
chmod +x ~/.local/bin/appimage-install
```

Or clone:

```bash
git clone https://github.com/Yug3ne/appimage-installer.git
ln -s "$(pwd)/appimage-installer/appimage-install" ~/.local/bin/appimage-install
```

Make sure `~/.local/bin` is in your `PATH`.

## Usage

```bash
# Install an AppImage
appimage-install ~/Downloads/MyApp.AppImage

# Electron apps need --no-sandbox
appimage-install ~/Downloads/VSCode.AppImage --no-sandbox

# Custom name
appimage-install ~/Downloads/MyApp.AppImage --name myapp

# List installed apps
appimage-install --list

#Updated  all updateable appimages
appimage-install --update-all

#Upadate a specific image
appimage-install --update appimage

# Remove an app
appimage-install --remove myapp
```

## What it does

1. Extracts the AppImage with `--appimage-extract`
2. Moves it to `~/.local/opt/<name>/`
3. Auto-detects the icon (PNG/SVG from embedded hicolor or root)
4. Creates a `.desktop` entry in `~/.local/share/applications/`
5. Symlinks `AppRun` to `~/.local/bin/<name>` for terminal use
6. Removes the original `.AppImage` file

## Requirements

- Bash 4+
- `update-desktop-database` (from `desktop-file-utils`, installed on most Linux desktops)

## License

MIT
