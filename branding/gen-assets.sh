#!/usr/bin/env sh
# ============================================================================
# gen-assets.sh — regenerate raster brand assets from the SVG sources
# ============================================================================
# Generates, from the committed SVG sources, the raster assets the installer
# configs reference:
#   * per-OS app icons: icon.ico (Windows), icon.icns (macOS), icon-256.png
#   * NSIS wizard art:  nsis-header.bmp (150x57), nsis-sidebar.bmp (164x314)
#   * dmg background:   dmg-background.png (660x400)
#
# Raster outputs land in git-ignored locations (branding/<app>/, branding/) and
# are regenerated on demand — only the SVG sources are committed.
#
# Tooling (free, OSS): rsvg-convert (librsvg) or ImageMagick `convert` for SVG
# rasterization; ImageMagick for .ico; png2icns / iconutil for .icns. If a tool
# is missing, the script prints the exact install command and SKIPS that output
# honestly (it never writes a corrupt or placeholder asset).
#
# Usage:
#   ./gen-assets.sh --app c0pl4nd
# ----------------------------------------------------------------------------
set -eu

APP=""
while [ $# -gt 0 ]; do
  case "$1" in
    --app)
      APP="${2:-}"
      shift 2
      ;;
    -h | --help)
      echo "Usage: $0 --app <name>"
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done
if [ -z "$APP" ]; then
  echo "ERROR: --app <name> is required." >&2
  exit 2
fi

BRANDING="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
ICON_SVG="$BRANDING/$APP/icon.svg"
DMG_SVG="$BRANDING/dmg-background.svg"

if [ ! -f "$ICON_SVG" ]; then
  echo "ERROR: icon source not found: $ICON_SVG" >&2
  exit 1
fi

# --- Pick an SVG rasterizer. ---
RASTER=""
if command -v rsvg-convert >/dev/null 2>&1; then
  RASTER="rsvg"
elif command -v convert >/dev/null 2>&1; then
  RASTER="im"
else
  echo "NOTICE: no SVG rasterizer found (need librsvg's rsvg-convert or" >&2
  echo "        ImageMagick's convert). Install one of:" >&2
  echo "          apt-get install librsvg2-bin      # rsvg-convert" >&2
  echo "          brew install librsvg              # rsvg-convert" >&2
  echo "          apt-get install imagemagick       # convert" >&2
  echo "Skipping raster generation (no asset faked)." >&2
  exit 0
fi

svg_to_png() {
  # $1 = src svg, $2 = dst png, $3 = width, $4 = height
  if [ "$RASTER" = "rsvg" ]; then
    rsvg-convert -w "$3" -h "$4" "$1" -o "$2"
  else
    convert -background none -resize "${3}x${4}" "$1" "$2"
  fi
}

echo "==> Generating PNG icon (256x256)"
svg_to_png "$ICON_SVG" "$BRANDING/$APP/icon-256.png" 256 256

# --- Windows .ico (multi-resolution) ---
if command -v convert >/dev/null 2>&1; then
  echo "==> Generating Windows .ico"
  TMPDIR_ICO="$(mktemp -d)"
  for sz in 16 32 48 64 128 256; do
    svg_to_png "$ICON_SVG" "$TMPDIR_ICO/icon-$sz.png" "$sz" "$sz"
  done
  convert "$TMPDIR_ICO"/icon-*.png "$BRANDING/$APP/icon.ico"
  rm -rf "$TMPDIR_ICO"
else
  echo "NOTICE: ImageMagick 'convert' absent — .ico skipped (install imagemagick)." >&2
fi

# --- macOS .icns ---
if command -v iconutil >/dev/null 2>&1; then
  echo "==> Generating macOS .icns (iconutil)"
  ICONSET="$(mktemp -d)/icon.iconset"
  mkdir -p "$ICONSET"
  for sz in 16 32 64 128 256 512; do
    svg_to_png "$ICON_SVG" "$ICONSET/icon_${sz}x${sz}.png" "$sz" "$sz"
    dbl=$((sz * 2))
    svg_to_png "$ICON_SVG" "$ICONSET/icon_${sz}x${sz}@2x.png" "$dbl" "$dbl"
  done
  iconutil -c icns "$ICONSET" -o "$BRANDING/$APP/icon.icns"
elif command -v png2icns >/dev/null 2>&1; then
  echo "==> Generating macOS .icns (png2icns)"
  TMPDIR_ICNS="$(mktemp -d)"
  for sz in 16 32 48 128 256 512; do
    svg_to_png "$ICON_SVG" "$TMPDIR_ICNS/icon-$sz.png" "$sz" "$sz"
  done
  png2icns "$BRANDING/$APP/icon.icns" "$TMPDIR_ICNS"/icon-*.png
  rm -rf "$TMPDIR_ICNS"
else
  echo "NOTICE: no .icns tool (iconutil on macOS / png2icns elsewhere) — .icns skipped." >&2
fi

# --- NSIS wizard art (BMP3). Per-app branded sidebar 164x314 + header 150x57. ---
# Rasterized from the committed per-app SVG sources (the fully-branded wordmark
# splashes), NOT a bare icon on a black plate. NSIS needs BMP3 (24-bit, no
# alpha), so each PNG is flattened onto the VOID-BLACK plate. See
# INSTALLER-BRANDING.md for the per-app splash standard.
SIDEBAR_SVG="$BRANDING/$APP/nsis-sidebar.svg"
HEADER_SVG="$BRANDING/$APP/nsis-header.svg"
if command -v convert >/dev/null 2>&1; then
  NSIS_TMP="$(mktemp -d)"
  if [ -f "$SIDEBAR_SVG" ]; then
    echo "==> Generating NSIS sidebar (164x314) from $APP/nsis-sidebar.svg"
    svg_to_png "$SIDEBAR_SVG" "$NSIS_TMP/sidebar.png" 164 314
    convert "$NSIS_TMP/sidebar.png" -background '#08060d' -flatten \
      "BMP3:$BRANDING/nsis-sidebar.bmp"
  else
    echo "NOTICE: $APP/nsis-sidebar.svg missing — sidebar skipped (no asset faked)." >&2
  fi
  if [ -f "$HEADER_SVG" ]; then
    echo "==> Generating NSIS header (150x57) from $APP/nsis-header.svg"
    svg_to_png "$HEADER_SVG" "$NSIS_TMP/header.png" 150 57
    convert "$NSIS_TMP/header.png" -background '#08060d' -flatten \
      "BMP3:$BRANDING/nsis-header.bmp"
  else
    echo "NOTICE: $APP/nsis-header.svg missing — header skipped (no asset faked)." >&2
  fi
  rm -rf "$NSIS_TMP"
else
  echo "NOTICE: ImageMagick 'convert' absent — NSIS bitmaps skipped." >&2
fi

# --- dmg background PNG ---
if [ -f "$DMG_SVG" ]; then
  echo "==> Generating dmg background PNG (660x400)"
  svg_to_png "$DMG_SVG" "$BRANDING/dmg-background.png" 660 400
fi

echo "==> Asset generation complete for '$APP'."
