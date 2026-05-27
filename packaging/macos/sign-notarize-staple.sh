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
# Usage:
#   ./sign-notarize-staple.sh --app <App.app path> --dmg <Disk.dmg path>
# ----------------------------------------------------------------------------
set -eu

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
xcrun stapler staple "$DMG_PATH"

echo "==> Validating the staple"
xcrun stapler validate "$DMG_PATH"
spctl --assess --type open --context context:primary-signature -v "$DMG_PATH" || true

echo "==> macOS sign + notarize + staple complete: $DMG_PATH"
