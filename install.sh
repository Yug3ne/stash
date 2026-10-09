#!/usr/bin/env bash
# Builds Stash and installs it for the current user. Run from the repo:
#
#   ./install.sh            build + install
#   ./install.sh --notify   also check for AppImage updates daily (notification)
set -euo pipefail

repo="$(dirname "$(readlink -f "$0")")"
bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/scalable/apps"

# Build requirements: Rust, and WebKitGTK for the window (Omarchy has it).
missing=()
command -v cargo >/dev/null || missing+=(rust)
pacman -Q webkit2gtk-4.1 >/dev/null 2>&1 || missing+=(webkit2gtk-4.1)
if (( ${#missing[@]} )); then
    echo "Please install first:  sudo pacman -S ${missing[*]}"
    exit 1
fi

echo ":: Building Stash (the first build takes a few minutes)…"
cargo build --release --manifest-path "$repo/Cargo.toml"

install -Dm755 "$repo/target/release/stash" "$bin/stash"
install -Dm644 "$repo/icons/icon.svg" "$icons/stash.svg"
cat >"$apps/stash.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Stash
Comment=Install AppImages and Arch Linux packages
Exec=$bin/stash
Icon=stash
Terminal=false
Categories=Settings;PackageManager;
Keywords=appimage;install;package;aur;pacman;software;store;
StartupWMClass=stash
DESKTOP
update-desktop-database -q "$apps" 2>/dev/null || true
echo "✓ Installed: run 'stash' or open Stash from your app launcher"

# Clean up what older versions of this project installed.
rm -f "$bin/appimage-install-tui" "$bin/app-installer" "$apps/omarchy-app-installer.desktop"
if [[ -L "$bin/appimage-install" ]] || grep -qs "appimage-install — " "$bin/appimage-install"; then
    rm -f "$bin/appimage-install"
    echo "✓ Replaced the old appimage-install script (use 'stash' now)"
fi
if systemctl --user is-enabled appimage-update-check.timer >/dev/null 2>&1; then
    systemctl --user disable --now appimage-update-check.timer >/dev/null 2>&1 || true
    rm -f "$HOME/.config/systemd/user/appimage-update-check".{service,timer}
    set -- --notify # keep the daily check, now done by stash
fi

if [[ "${1:-}" == "--notify" ]]; then
    units="$HOME/.config/systemd/user"
    mkdir -p "$units"
    cat >"$units/stash-update-check.service" <<UNIT
[Unit]
Description=Check AppImages installed by Stash for updates

[Service]
Type=oneshot
ExecStart=$bin/stash updates --notify
UNIT
    cat >"$units/stash-update-check.timer" <<UNIT
[Unit]
Description=Check AppImages for updates daily

[Timer]
OnCalendar=daily
OnBootSec=10min
Persistent=true

[Install]
WantedBy=timers.target
UNIT
    systemctl --user daemon-reload
    systemctl --user enable --now stash-update-check.timer
    echo "✓ Daily AppImage update notifications enabled"
fi
