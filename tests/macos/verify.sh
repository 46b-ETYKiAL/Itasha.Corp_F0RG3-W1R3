#!/usr/bin/env sh
# ============================================================================
# verify.sh — macOS signing / notarization / Gatekeeper verification
# ============================================================================
# Verifies a built .app + .dmg against Gatekeeper expectations:
#   * codesign --verify (deep, strict) on the .app
#   * spctl --assess --type install on the .dmg (Gatekeeper acceptance)
#   * stapler validate on the .dmg (notarization ticket stapled => offline OK)
#   * (Tahoe O3) optional custom-volume-icon presence check on the .dmg
#
# Sequoia/Tahoe context (competitive O3): on Sequoia (15) the Control-click
# "Open Anyway" override is GONE, so an un-stapled .dmg traps the user in
# System Settings on first run — stapler validate passing is what prevents that
# (see docs/macos-gatekeeper.md). On Tahoe (26) notarization wipes the custom
# volume icon; the optional --expect-volicon check asserts it was re-applied.
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

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
# shellcheck source=../matrix/_lib.sh
. "$SCRIPT_DIR/../matrix/_lib.sh"

# The custom volume-icon file is named "Icon" + carriage return (0x0D); build
# the name without embedding a raw CR in source.
ICON_CR_NAME="Icon$(printf '\r')"

APP_PATH=""
DMG_PATH=""
EXPECT_VOLICON=0
# --expect-signed makes the Gatekeeper chain LOAD-BEARING: when set (creds were
# present and signing was engaged in the build), a failing codesign / spctl /
# stapler check is a HARD FAILURE (exit 1) rather than an honest unsigned-dev
# NOTE. This is the test-side mirror of the sign-notarize-staple.sh root-cause
# fix (dropping the `|| true` swallow). The release-verify workflow passes it on
# a creds-present signed build; a deliberately un-stapled/tampered .dmg then
# FAILS, proving the verdict is no longer swallowed.
EXPECT_SIGNED=0
while [ $# -gt 0 ]; do
  case "$1" in
    --app)
      APP_PATH="${2:-}"
      shift 2
      ;;
    --dmg)
      DMG_PATH="${2:-}"
      shift 2
      ;;
    --expect-volicon)
      EXPECT_VOLICON=1
      shift
      ;;
    --expect-signed)
      EXPECT_SIGNED=1
      shift
      ;;
    -h | --help)
      echo "Usage: $0 --app <App.app> --dmg <Disk.dmg> [--expect-volicon] [--expect-signed]"
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      exit 2
      ;;
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

# Under --expect-signed every Gatekeeper check is LOAD-BEARING: a NOTE becomes a
# hard FAIL. This is what makes a deliberately un-stapled/tampered fixture fail
# the test (proving the verdict is no longer swallowed). Without the flag the
# honest unsigned-dev NOTE path is preserved (creds-absent posture, ADR-0003).
assert_or_note() {
  # $1 = human label of the failed check (already-failed context)
  if [ "$EXPECT_SIGNED" -eq 1 ]; then
    fail "$1 (load-bearing under --expect-signed; signing was expected)"
    FAILED=1
  else
    log "NOTE: $1 — expected for an un-notarized dev build (ADR-0003). NEVER faked."
  fi
}

# 1. codesign verification on the .app.
if codesign --verify --deep --strict --verbose=2 "$APP_PATH" >/dev/null 2>&1; then
  pass "codesign --verify --deep --strict on $APP_PATH"
else
  # Distinguish "unsigned dev build" from "signed-but-broken".
  if codesign -dv "$APP_PATH" >/dev/null 2>&1; then
    fail "codesign verify failed on a SIGNED .app (broken signature)"
    FAILED=1
  else
    assert_or_note "$APP_PATH is UNSIGNED (dev build); Gatekeeper would warn until Developer ID signing is applied"
  fi
fi

# 2. Gatekeeper assessment on the .dmg.
if spctl --assess --type install --verbose=2 "$DMG_PATH" >/dev/null 2>&1; then
  pass "spctl --assess accepts $DMG_PATH (Gatekeeper would allow install)"
else
  assert_or_note "spctl --assess REJECTED $DMG_PATH (Gatekeeper would block install)"
fi

# 3. Stapled notarization ticket (offline Gatekeeper pass).
if stapler validate "$DMG_PATH" >/dev/null 2>&1; then
  pass "stapler validate: notarization ticket stapled to $DMG_PATH (offline OK)"
else
  assert_or_note "no stapled notarization ticket on $DMG_PATH (Gatekeeper offline-block)"
fi

# 4. Tahoe custom-volume-icon survival (only when explicitly expected).
# A .dmg mounts to /Volumes/<name>; we inspect the mounted root for the custom
# "Icon\r" resource. Without --expect-volicon this is informational only.
if [ "$EXPECT_VOLICON" -eq 1 ]; then
  MOUNT_OUT="$(hdiutil attach -nobrowse -readonly "$DMG_PATH" 2>/dev/null || true)"
  VOL_DIR="$(printf '%s\n' "$MOUNT_OUT" | grep -o '/Volumes/.*' | head -n1 || true)"
  if [ -n "$VOL_DIR" ] && [ -d "$VOL_DIR" ]; then
    if [ -f "$VOL_DIR/$ICON_CR_NAME" ]; then
      pass "custom volume icon present on $DMG_PATH (survived Tahoe notarization re-apply)"
    else
      fail "custom volume icon MISSING on $DMG_PATH — Tahoe notarization wiped it and re-apply did not run (--volicon on sign-notarize-staple.sh)"
      FAILED=1
    fi
    hdiutil detach "$VOL_DIR" >/dev/null 2>&1 || true
  else
    log "NOTE: could not mount $DMG_PATH to verify the volume icon; skipping (not faked as present)."
  fi
fi

if [ "$FAILED" -ne 0 ]; then
  exit 1
fi
log "macOS verification complete (signed assertions passed where signing present; unsigned-dev states recorded honestly)."
exit 0
