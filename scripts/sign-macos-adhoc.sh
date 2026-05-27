#!/usr/bin/env sh
# ============================================================================
# sign-macos-adhoc.sh — free ad-hoc codesign for macOS (local integrity)
# ============================================================================
# When no Apple Developer ID is configured, ad-hoc-sign the .app bundles under
# dist/ with `codesign --sign -`. This is FREE (no Apple Developer Program) and
# gives the binary a valid in-place signature that:
#   - lets the app run on the BUILD machine and pass local integrity checks,
#   - is required for some macOS APIs (e.g. hardened-runtime entitlements,
#     Apple-silicon execution of locally-built code).
#
# It does NOT satisfy Gatekeeper for DISTRIBUTED apps — downloaded ad-hoc apps
# still need a right-click → Open (or `xattr -d com.apple.quarantine`). Warning-
# free distribution requires Developer ID + notarization (paid; ADR-0003). The
# paid path lives in sign-notarize-staple.sh and takes precedence when creds
# are present.
#
# Usage:  ./scripts/sign-macos-adhoc.sh [dist-dir]
# ----------------------------------------------------------------------------
set -eu

DIST_DIR="${1:-dist}"

if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "Apple Developer ID present — skipping ad-hoc (use sign-notarize-staple.sh)."
  exit 0
fi
if ! command -v codesign >/dev/null 2>&1; then
  echo "::warning::codesign not available (not macOS?) — ad-hoc signing skipped."
  exit 0
fi
[ -d "$DIST_DIR" ] || { echo "No $DIST_DIR/ — nothing to sign."; exit 0; }

count=0
for app in "$DIST_DIR"/*.app; do
  [ -d "$app" ] || continue
  codesign --force --deep --sign - --options runtime "$app" 2>/dev/null \
    || codesign --force --deep --sign - "$app"
  count=$((count + 1))
  echo "  ad-hoc signed: $(basename "$app")"
done

if [ "$count" -eq 0 ]; then
  echo "No .app bundles in $DIST_DIR/ (dmg-only build) — ad-hoc signing skipped."
else
  echo "==> Ad-hoc signed $count bundle(s). NOT notarized — distributed users must"
  echo "    right-click → Open once, or run: xattr -d com.apple.quarantine <app>."
fi
