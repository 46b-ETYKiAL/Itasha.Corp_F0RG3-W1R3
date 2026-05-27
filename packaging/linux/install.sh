#!/usr/bin/env sh
# ============================================================================
# install.sh — one-line Linux installer for an Itasha.Corp AppImage
# ============================================================================
# Installs a downloaded AppImage into the user's local bin, registers the
# .desktop entry + icon for menu integration, and verifies the checksum if a
# .sha256 sidecar is present. User-scope (no root) by default.
#
# Usage:
#   ./install.sh --appimage <path-to.AppImage> [--name c0pl4nd]
#   (or fetched: curl -fsSL <url>/install.sh | sh -s -- --appimage ./C0PL4ND.AppImage)
# ----------------------------------------------------------------------------
set -eu

APPIMAGE=""
NAME="c0pl4nd"

while [ $# -gt 0 ]; do
  case "$1" in
    --appimage) APPIMAGE="${2:-}"; shift 2 ;;
    --name)     NAME="${2:-}"; shift 2 ;;
    -h|--help)
      echo "Usage: $0 --appimage <path.AppImage> [--name <binname>]"
      exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ -z "$APPIMAGE" ]; then
  echo "ERROR: --appimage <path> is required." >&2
  exit 2
fi
if [ ! -f "$APPIMAGE" ]; then
  echo "ERROR: AppImage not found: $APPIMAGE" >&2
  exit 1
fi

# --- Optional checksum verification (binary-as-artifact integrity). ---
if [ -f "$APPIMAGE.sha256" ]; then
  echo "==> Verifying checksum"
  if command -v sha256sum >/dev/null 2>&1; then
    ( cd "$(dirname "$APPIMAGE")" && sha256sum -c "$(basename "$APPIMAGE").sha256" )
  else
    echo "WARNING: sha256sum not available; skipping checksum verification." >&2
  fi
fi

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"

mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICON_DIR"

echo "==> Installing AppImage to $BIN_DIR/$NAME"
cp "$APPIMAGE" "$BIN_DIR/$NAME"
chmod +x "$BIN_DIR/$NAME"

# --- Install the .desktop entry (menu integration). ---
DESKTOP_SRC="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/$NAME.desktop"
if [ -f "$DESKTOP_SRC" ]; then
  echo "==> Installing desktop entry to $APPS_DIR/$NAME.desktop"
  # Point Exec at the installed binary path.
  sed "s|^Exec=.*|Exec=$BIN_DIR/$NAME|" "$DESKTOP_SRC" > "$APPS_DIR/$NAME.desktop"
fi

# --- Install the icon (256px, hicolor theme). ---
ICON_SRC="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/../../branding/$NAME/icon-256.png"
if [ -f "$ICON_SRC" ]; then
  echo "==> Installing icon to $ICON_DIR/$NAME.png"
  cp "$ICON_SRC" "$ICON_DIR/$NAME.png"
fi

# --- Refresh the desktop database / icon cache if the tools exist. ---
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "==> Installed. Ensure $BIN_DIR is on your PATH, then run: $NAME"
