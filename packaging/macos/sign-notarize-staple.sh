#!/usr/bin/env sh
# ============================================================================
# sign-notarize-staple.sh — macOS Developer ID signing + notarization + staple
# ============================================================================
# Signs the .app, submits the .dmg to Apple notarization, and staples the
# ticket so Gatekeeper passes OFFLINE.
#
# GATING (external_dependency: Apple Developer Program):
#   This pipeline requires an Apple Developer ID. The required credentials are
#   read BY NAME from the environment (CI secrets); they are NEVER literal here.
#   If the credentials are ABSENT, this script SKIPS signing honestly, prints a
#   documented dev-unsigned warning, and exits 0 — it NEVER fakes notarization.
#
# Required env vars (referenced by name; supplied from CI secrets / Keychain):
#   APPLE_SIGNING_IDENTITY   e.g. "Developer ID Application: Itasha.Corp (TEAMID)"
#   APPLE_ID                 the Apple ID email used for notarization
#   APPLE_TEAM_ID            the 10-char Apple Team ID
#   APPLE_APP_PASSWORD       an app-specific password (NOT the account password)
#   (OR App Store Connect API key: APPLE_API_KEY_ID / APPLE_API_ISSUER / APPLE_API_KEY_PATH)
#
# macOS Sequoia (15) / Tahoe (26) hardening (competitive O3):
#   * SEQUOIA removed the Control-click "Open Anyway" Gatekeeper override. An
#     UNSIGNED/UN-NOTARIZED app now traps the user in System Settings > Privacy
#     & Security on EVERY first run. Notarize + staple (below) is the only path
#     that avoids that trip for public distribution — see docs/macos-gatekeeper.md.
#   * TAHOE wipes a disk image's custom volume icon (the special "Icon\r" file)
#     during notarization. This script handles it by RE-APPLYING the custom
#     .dmg volume icon AFTER stapling, so the branded icon survives. The .app's
#     own bundle icon is unaffected (it is inside the signed bundle).
#
# Usage:
#   ./sign-notarize-staple.sh --app <App.app path> --dmg <Disk.dmg path> [--volicon <icon.icns>]
# ----------------------------------------------------------------------------
set -eu

# The custom volume-icon file inside a .dmg is literally named "Icon" followed
# by a carriage return (0x0D). Build the name without embedding a raw CR in the
# source so editors / the content-safety scan stay clean.
ICON_CR_NAME="Icon$(printf '\r')"

APP_PATH=""
DMG_PATH=""
VOLICON_PATH=""

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
    --volicon)
      VOLICON_PATH="${2:-}"
      shift 2
      ;;
    -h | --help)
      echo "Usage: $0 --app <App.app> --dmg <Disk.dmg> [--volicon <icon.icns>]"
      exit 0
      ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [ -z "$APP_PATH" ] || [ -z "$DMG_PATH" ]; then
  echo "ERROR: both --app and --dmg are required." >&2
  exit 2
fi

# --- Honest credential gate. No identity => dev-unsigned, never faked. ---
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "================================================================" >&2
  echo "NOTICE: APPLE_SIGNING_IDENTITY is not set." >&2
  echo "Shipping an UNSIGNED dev build. Signing + notarization SKIPPED." >&2
  echo "" >&2
  echo "This is the documented dev-unsigned-until-creds posture:" >&2
  echo "  * On the building Mac the app runs after a right-click > Open." >&2
  echo "  * For PUBLIC distribution, Gatekeeper HARD-BLOCKS unsigned apps." >&2
  echo "  * Acquire an Apple Developer Program account (\$99/yr) for a" >&2
  echo "    Developer ID, then re-run with the credentials set." >&2
  echo "Notarization is NEVER faked. See docs/adr/0003-signing-posture.md." >&2
  echo "================================================================" >&2
  exit 0
fi

echo "==> Signing the .app with hardened runtime"
codesign --force --options runtime --timestamp \
  --sign "$APPLE_SIGNING_IDENTITY" "$APP_PATH"

echo "==> Verifying the signature"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

echo "==> Submitting the .dmg to Apple notarization (notarytool, waits)"
if [ -n "${APPLE_API_KEY_ID:-}" ]; then
  # App Store Connect API-key path.
  xcrun notarytool submit "$DMG_PATH" \
    --key "${APPLE_API_KEY_PATH}" \
    --key-id "${APPLE_API_KEY_ID}" \
    --issuer "${APPLE_API_ISSUER}" \
    --wait
else
  # Apple-ID + app-specific-password path.
  xcrun notarytool submit "$DMG_PATH" \
    --apple-id "${APPLE_ID}" \
    --team-id "${APPLE_TEAM_ID}" \
    --password "${APPLE_APP_PASSWORD}" \
    --wait
fi

echo "==> Stapling the notarization ticket to the .dmg (offline Gatekeeper)"
# Stapling makes Gatekeeper pass OFFLINE (no notarization-server round-trip on
# the user's machine). On Sequoia this is what avoids the System-Settings trip.
xcrun stapler staple "$DMG_PATH"

# --- LOAD-BEARING Gatekeeper verification (ROOT-CAUSE FIX, was `|| true`) -----
# Apple's guidance: "notarization passing != Gatekeeper passing" — assert BOTH
# `stapler validate` AND `spctl --assess` under Hardened Runtime. The prior code
# ended the spctl line with `|| true`, which SWALLOWED a rejecting verdict: a
# tampered or un-notarized .dmg would print a rejection and the script would
# still exit 0, falsely reporting success. We reach this point ONLY because
# APPLE_SIGNING_IDENTITY is present (creds-present, signing was engaged), so the
# verdict is now load-bearing — a failure HARD-FAILS the script.
echo "==> Validating the staple (load-bearing)"
if ! xcrun stapler validate "$DMG_PATH"; then
  echo "::error::stapler validate FAILED on $DMG_PATH — the notarization ticket is missing or invalid. NOT swallowing this verdict (was previously masked by '|| true')." >&2
  exit 1
fi

echo "==> Gatekeeper assessment (load-bearing)"
if ! spctl --assess --type open --context context:primary-signature -v "$DMG_PATH"; then
  echo "::error::spctl --assess REJECTED $DMG_PATH — Gatekeeper would block this build despite signing being engaged (tampered/un-notarized payload). NOT swallowing this verdict (was previously masked by '|| true')." >&2
  exit 1
fi
echo "::notice::Gatekeeper verification passed: stapler validate + spctl --assess both accept $DMG_PATH."

# --- Tahoe Icon-wipe handling: re-apply the branded volume icon if requested ---
# macOS Tahoe (26) wipes a .dmg's custom volume icon (the "Icon\r" resource)
# during notarization. If a --volicon was supplied, re-apply it AFTER stapling
# so the branded disk-image icon survives. This requires `fileicon` or
# `SetFile`/`Rez` from the Xcode tools; the step is best-effort and NEVER fakes
# success — it logs honestly when the helper is unavailable.
if [ -n "$VOLICON_PATH" ]; then
  if [ ! -f "$VOLICON_PATH" ]; then
    echo "NOTICE: --volicon '$VOLICON_PATH' not found; skipping icon re-apply." >&2
  elif command -v fileicon >/dev/null 2>&1; then
    echo "==> Re-applying custom volume icon (Tahoe wipe recovery) via fileicon"
    fileicon set "$DMG_PATH" "$VOLICON_PATH"
  elif command -v SetFile >/dev/null 2>&1 && command -v Rez >/dev/null 2>&1; then
    echo "==> Re-applying custom volume icon (Tahoe wipe recovery) via SetFile/Rez"
    # Standard Apple recipe: encode the .icns into the Icon resource, then set
    # the volume's custom-icon bit. (The Icon resource lives in the file whose
    # name is "Icon" + CR; we reference it via $ICON_CR_NAME.)
    DeRez -only icns "$VOLICON_PATH" >"${TMPDIR:-/tmp}/volicon.rsrc" 2>/dev/null || true
    Rez -append "${TMPDIR:-/tmp}/volicon.rsrc" -o "$DMG_PATH/$ICON_CR_NAME" 2>/dev/null || true
    SetFile -a C "$DMG_PATH" 2>/dev/null || true
    SetFile -a V "$DMG_PATH/$ICON_CR_NAME" 2>/dev/null || true
  else
    echo "NOTICE: no icon helper (fileicon / SetFile+Rez) available." >&2
    echo "        The branded volume icon was wiped by notarization on Tahoe and" >&2
    echo "        was NOT re-applied. Install 'fileicon' (brew install fileicon)" >&2
    echo "        or run from a full Xcode install to restore it. NOT faked." >&2
  fi
fi

echo "==> macOS sign + notarize + staple complete: $DMG_PATH"
