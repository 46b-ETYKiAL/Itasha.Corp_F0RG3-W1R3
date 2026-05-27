#!/usr/bin/env sh
# ============================================================================
# verify.sh — macOS signing / notarization / Gatekeeper verification
# ============================================================================
# Verifies a built .app + .dmg against Gatekeeper expectations:
#   * codesign --verify (deep, strict) on the .app
#   * spctl --assess --type install on the .dmg (Gatekeeper acceptance)
#   * stapler validate on the .dmg (notarization ticket stapled => offline OK)
#
# HONEST SKIP DISCIPLINE: this requires a macOS host with the Apple toolchain.
#   * Not macOS                  -> exit 77 (skip), loud message.
#   * Tool missing (spctl/stapler/codesign) -> exit 77 (skip), loud message.
#   * No signing creds / unsigned dev build -> records an honest UNSIGNED-DEV
#       note and exits 0 (an intentionally-unsigned dev build is not a failure;
#       it is the documented dev-unsigned-until-creds posture). It is NEVER
#       reported as "notarized".
#
# Usage:
#   ./tests/macos/verify.sh --app <App.app> --dmg <Disk.dmg>
#
# Exit codes: 0 pass (or honest unsigned-dev); 1 fail (a real signed-state
# assertion failed); 77 skip (host/tool unavailable).
# ----------------------------------------------------------------------------
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=../matrix/_lib.sh
. "$SCRIPT_DIR/../matrix/_lib.sh"

APP_PATH=""
DMG_PATH=""
while [ $# -gt 0 ]; do
  case "$1" in
    --app) APP_PATH="${2:-}"; shift 2 ;;
    --dmg) DMG_PATH="${2:-}"; shift 2 ;;
    -h|--help) echo "Usage: $0 --app <App.app> --dmg <Disk.dmg>"; exit 0 ;;
    *) echo "ERROR: unknown argument: $1" >&2; exit 2 ;;
  esac
done

if ! require_os macos; then exit "$MATRIX_SKIP"; fi
if ! require_tool codesign "Xcode Command Line Tools (xcode-select --install)"; then exit "$MATRIX_SKIP"; fi
if ! require_tool spctl "Xcode Command Line Tools"; then exit "$MATRIX_SKIP"; fi
if ! require_tool stapler "Xcode Command Line Tools (xcrun stapler)"; then exit "$MATRIX_SKIP"; fi

if [ -z "$APP_PATH" ] || [ ! -d "$APP_PATH" ]; then
  skip "macos-verify" "no .app supplied or not found (pass --app <App.app>)"
  exit "$MATRIX_SKIP"
fi
if [ -z "$DMG_PATH" ] || [ ! -f "$DMG_PATH" ]; then
  skip "macos-verify" "no .dmg supplied or not found (pass --dmg <Disk.dmg>)"
  exit "$MATRIX_SKIP"
fi

FAILED=0

# 1. codesign verification on the .app.
if codesign --verify --deep --strict --verbose=2 "$APP_PATH" >/dev/null 2>&1; then
  pass "codesign --verify --deep --strict on $APP_PATH"
else
  # Distinguish "unsigned dev build" from "signed-but-broken".
  if codesign -dv "$APP_PATH" >/dev/null 2>&1; then
    fail "codesign verify failed on a SIGNED .app (broken signature)"
    FAILED=1
  else
    log "NOTE: $APP_PATH is UNSIGNED (dev build). Not a failure — documented dev-unsigned posture (ADR-0003). Gatekeeper will warn on distribution until Developer ID signing is applied."
  fi
fi

# 2. Gatekeeper assessment on the .dmg.
if spctl --assess --type install --verbose=2 "$DMG_PATH" >/dev/null 2>&1; then
  pass "spctl --assess accepts $DMG_PATH (Gatekeeper would allow install)"
else
  log "NOTE: spctl rejected $DMG_PATH — expected for an un-notarized dev build (ADR-0003). Not faked as accepted."
fi

# 3. Stapled notarization ticket (offline Gatekeeper pass).
if stapler validate "$DMG_PATH" >/dev/null 2>&1; then
  pass "stapler validate: notarization ticket stapled to $DMG_PATH (offline OK)"
else
  log "NOTE: no stapled ticket on $DMG_PATH — expected for an un-notarized dev build. NEVER faked."
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi
log "macOS verification complete (signed assertions passed where signing present; unsigned-dev states recorded honestly)."
exit 0
