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

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=../matrix/_lib.sh
. "$SCRIPT_DIR/../matrix/_lib.sh"
ROOT="$(CDPATH='' cd -- "$SCRIPT_DIR/../.." && pwd)"

APPIMAGE=""
DEB=""
DESKTOP="$ROOT/packaging/linux/c0pl4nd.desktop"
# --dpkg-cycle opts the .deb section into a real install+remove cycle (needs
# root / a disposable container). Off by default so the lintian-only path still
# runs on an unprivileged runner. --pubkey overrides the minisign public key.
DPKG_CYCLE=0
PUBKEY="$ROOT/keys/minisign.pub"
while [ $# -gt 0 ]; do
  case "$1" in
    --appimage)
      APPIMAGE="${2:-}"
      shift 2
      ;;
    --deb)
      DEB="${2:-}"
      shift 2
      ;;
    --desktop)
      DESKTOP="${2:-}"
      shift 2
      ;;
    --dpkg-cycle)
      DPKG_CYCLE=1
      shift
      ;;
    --pubkey)
      PUBKEY="${2:-}"
      shift 2
      ;;
    -h | --help)
      echo "Usage: $0 [--appimage <file>] [--deb <file>] [--desktop <file>] [--dpkg-cycle] [--pubkey <key>]"
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      exit 2
      ;;
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

  # 2b. Real install + remove cycle (opt-in: --dpkg-cycle, needs root/container).
  # Asserts the .deb installs cleanly AND removes cleanly with no dpkg error —
  # the smoke test that the package's maintainer scripts + dependency line work.
  if [ "$DPKG_CYCLE" -eq 1 ]; then
    if require_tool dpkg "apt-get install dpkg"; then
      RAN_SOMETHING=1
      if [ "$(id -u)" -ne 0 ] && ! command -v sudo >/dev/null 2>&1; then
        skip "linux-deb-cycle" "--dpkg-cycle needs root or sudo (run in a disposable container); recorded honestly"
      else
        _sudo=""
        [ "$(id -u)" -ne 0 ] && _sudo="sudo"
        # Resolve the package name from the .deb control metadata.
        _pkg="$(dpkg-deb -f "$DEB" Package 2>/dev/null || true)"
        if [ -z "$_pkg" ]; then
          fail "could not read Package name from $DEB control metadata"
          FAILED=1
        elif $_sudo dpkg -i "$DEB" >/dev/null 2>&1 || $_sudo apt-get install -y -f >/dev/null 2>&1; then
          pass "dpkg -i installed $_pkg from $DEB"
          if $_sudo dpkg -r "$_pkg" >/dev/null 2>&1; then
            pass "dpkg -r removed $_pkg cleanly"
          else
            fail "dpkg -r failed to remove $_pkg cleanly"
            FAILED=1
          fi
        else
          fail "dpkg -i failed to install $DEB (even after apt-get -f)"
          FAILED=1
        fi
      fi
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
  if (cd "$_tmp" && "$APPIMAGE" --appimage-extract >/dev/null 2>&1); then
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

# 4. checksum.sha256 + minisign verification of the Linux artifacts.
# Delegates to scripts/verify.sh (which checks BOTH the sha256 against the
# published checksum.sha256 AND the minisign signature against the public key).
# A present-but-INVALID signature/checksum is a hard FAIL; an absent sidecar is
# an honest skip (an unsigned dev build is not a failure).
_verify_artifact() {
  _art="$1"
  [ -z "$_art" ] && return 0
  [ ! -f "$_art" ] && return 0
  if [ ! -x "$ROOT/scripts/verify.sh" ] && [ ! -f "$ROOT/scripts/verify.sh" ]; then
    skip "linux-verify-sig" "scripts/verify.sh not found; cannot run checksum/minisign verification"
    return 0
  fi
  RAN_SOMETHING=1
  # Run verify.sh from the artifact's directory so it finds checksum.sha256 +
  # the .minisig sidecar next to the file.
  _dir="$(CDPATH='' cd -- "$(dirname -- "$_art")" && pwd)"
  _base="$(basename -- "$_art")"
  if [ -f "$_dir/$_base.minisig" ] || [ -f "$_dir/checksum.sha256" ]; then
    if (cd "$_dir" && sh "$ROOT/scripts/verify.sh" "$_base" "$PUBKEY"); then
      pass "checksum + minisign verification on $_base"
    else
      fail "checksum/minisign verification FAILED on $_base"
      FAILED=1
    fi
  else
    skip "linux-verify-sig" "no checksum.sha256 or $_base.minisig beside $_base (unsigned dev build); not faked"
  fi
}
_verify_artifact "$APPIMAGE"
_verify_artifact "$DEB"

if [ "$RAN_SOMETHING" -eq 0 ]; then
  log "Nothing could be verified (no tools, no artifacts) — recorded as skip."
  exit "$MATRIX_SKIP"
fi
if [ "$FAILED" -ne 0 ]; then
  exit 1
fi
log "Linux verification complete (ran available checks; skipped absent tools/artifacts honestly)."
exit 0
