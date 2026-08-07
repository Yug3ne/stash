#!/usr/bin/env bash
set -euo pipefail

REPO="Yug3ne/appimage-installer"
BIN_NAME="appimage-install-tui"
CLI_NAME="appimage-install"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
TARGET="$INSTALL_DIR/$BIN_NAME"
CLI_TARGET="$INSTALL_DIR/$CLI_NAME"
TUI_URL="https://raw.githubusercontent.com/$REPO/main/tui/dist/cli.js"
CLI_URL="https://raw.githubusercontent.com/$REPO/main/appimage-install"

cmd_exists() {
  command -v "$1" >/dev/null 2>&1
}

echo "==> Installing $BIN_NAME"

# 1. Ensure Bun is available
if ! cmd_exists bun; then
  echo "==> Bun not found. Installing Bun..."
  curl -fsSL https://bun.sh/install | bash

  # Try to load Bun into this shell session
  if [ -f "$HOME/.bashrc" ]; then
    # shellcheck source=/dev/null
    . "$HOME/.bashrc"
  fi
  if [ -f "$HOME/.zshrc" ]; then
    # shellcheck source=/dev/null
    . "$HOME/.zshrc"
  fi

  if ! cmd_exists bun; then
    export PATH="$HOME/.bun/bin:$PATH"
  fi

  if ! cmd_exists bun; then
    echo "ERROR: Bun installation failed or is not on PATH." >&2
    echo "Please open a new terminal and try again." >&2
    exit 1
  fi
fi

echo "==> Using Bun $(bun --version)"

# 2. Prepare install directory
mkdir -p "$INSTALL_DIR"

# 3. Download the TUI binary
echo "==> Downloading $BIN_NAME from $REPO..."
curl -fsSL "$TUI_URL" -o "$TARGET"
chmod +x "$TARGET"

# 4. Download the CLI backend (required by the TUI)
echo "==> Downloading $CLI_NAME from $REPO..."
curl -fsSL "$CLI_URL" -o "$CLI_TARGET"
chmod +x "$CLI_TARGET"

# 5. Verify the downloads
if [ ! -s "$TARGET" ]; then
  echo "ERROR: Downloaded $BIN_NAME is empty or missing." >&2
  exit 1
fi

if ! head -1 "$TARGET" | grep -q "bun"; then
  echo "ERROR: Downloaded $BIN_NAME does not have a Bun shebang." >&2
  exit 1
fi

if [ ! -s "$CLI_TARGET" ]; then
  echo "ERROR: Downloaded $CLI_NAME is empty or missing." >&2
  exit 1
fi

# 6. Warn if install dir is not on PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo
    echo "WARNING: $INSTALL_DIR is not on your PATH." >&2
    echo "Add this to your shell profile (e.g. ~/.bashrc or ~/.zshrc):" >&2
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" >&2
    ;;
esac

echo
echo "==> Installed to $INSTALL_DIR"
echo "   TUI: $BIN_NAME"
echo "   CLI: $CLI_NAME"
echo
echo "   Run the TUI:    $BIN_NAME"
echo "   Run the CLI:    $CLI_NAME --help"
