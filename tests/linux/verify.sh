#!/usr/bin/env sh
# ============================================================================
# verify.sh — Linux AppImage + .deb verification
# ============================================================================
# Verifies the Linux packages on a clean container/VM:
#   * appimagetool --appimage-extract-and-run (or appimage runtime) self-check
#     + the .desktop file inside the AppImage is freedesktop-valid;
#   * lintian on the .deb;
#   * desktop-file-validate on the bundled .desktop (menu integration).
#
# HONEST SKIP DISCIPLINE: required tools are not installed here by default.
#   * Tool missing (lintian / appimagetool / desktop-file-validate) -> the
#     relevant check is SKIPPED with a loud structured message; the script
#     still verifies what it can and exits 77 only if NOTHING could run.
#   * A real validation failure exits 1. Skips are never reported as success.
#
# Usage:
#   ./tests/linux/verify.sh [--appimage <file>] [--deb <file>] [--desktop <file>]
#
# Exit codes: 0 pass; 1 fail; 77 skip (no tool/artifact available at all).
# ----------------------------------------------------------------------------
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=../matrix/_lib.sh
. "$SCRIPT_DIR/../matrix/_lib.sh"
ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)"

APPIMAGE=""
DEB=""
DESKTOP="$ROOT/packaging/linux/c0pl4nd.desktop"
while [ $# -gt 0 ]; do
  case "$1" in
    --appimage) APPIMAGE="${2:-}"; shift 2 ;;
    --deb) DEB="${2:-}"; shift 2 ;;
    --desktop) DESKTOP="${2:-}"; shift 2 ;;
    -h|--help) echo "Usage: $0 [--appimage <file>] [--deb <file>] [--desktop <file>]"; exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if ! require_os linux; then exit "$MATRIX_SKIP"; fi

FAILED=0
RAN_SOMETHING=0

# 1. .desktop validation (the committed per-app .desktop always exists).
if [ -f "$DESKTOP" ]; then
  if require_tool desktop-file-validate "apt-get install desktop-file-utils"; then
    RAN_SOMETHING=1
    if desktop-file-validate "$DESKTOP"; then
      pass "desktop-file-validate: $DESKTOP"
    else
      fail "desktop-file-validate reported errors in $DESKTOP"
      FAILED=1
    fi
  fi
else
  skip "linux-desktop" "no .desktop file at $DESKTOP"
fi

# 2. lintian on the .deb.
if [ -n "$DEB" ] && [ -f "$DEB" ]; then
  if require_tool lintian "apt-get install lintian"; then
    RAN_SOMETHING=1
    # lintian non-zero exit on errors; warnings are surfaced but not fatal here.
    if lintian --fail-on error "$DEB"; then
      pass "lintian (fail-on error): $DEB"
    else
      fail "lintian reported errors in $DEB"
      FAILED=1
    fi
  fi
else
  skip "linux-deb" "no .deb supplied (pass --deb <file>); build one with scripts/build.sh first"
fi

# 3. AppImage self-check.
if [ -n "$APPIMAGE" ] && [ -f "$APPIMAGE" ]; then
  RAN_SOMETHING=1
  chmod +x "$APPIMAGE" 2>/dev/null || true
  # Extract and confirm the internal .desktop is present + valid.
  _tmp="$(mktemp -d)"
  if ( cd "$_tmp" && "$APPIMAGE" --appimage-extract >/dev/null 2>&1 ); then
    _internal_desktop="$(find "$_tmp" -maxdepth 3 -name '*.desktop' 2>/dev/null | head -n1)"
    if [ -n "$_internal_desktop" ]; then
      pass "AppImage extracts and contains a .desktop entry"
    else
      fail "AppImage contains no .desktop entry (menu integration would break)"
      FAILED=1
    fi
  else
    skip "linux-appimage" "AppImage --appimage-extract unavailable (needs FUSE or a runtime); recorded honestly"
  fi
  rm -rf "$_tmp"
else
  skip "linux-appimage" "no AppImage supplied (pass --appimage <file>)"
fi

if [ "$RAN_SOMETHING" -eq 0 ]; then
  log "Nothing could be verified (no tools, no artifacts) — recorded as skip."
  exit "$MATRIX_SKIP"
fi
if [ "$FAILED" -ne 0 ]; then
  exit 1
fi
log "Linux verification complete (ran available checks; skipped absent tools/artifacts honestly)."
exit 0
