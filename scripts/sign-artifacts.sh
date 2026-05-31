#!/usr/bin/env sh
# ============================================================================
# sign-artifacts.sh — free, cross-platform artifact signing (minisign)
# ============================================================================
# Signs every artifact in dist/ with minisign (Ed25519, no CA, no cost) to
# produce a detached `<artifact>.minisig` next to each file. minisign runs
# identically on Windows, macOS, and Linux, so this is the universal
# free-signing tier for ALL platforms' installers.
#
# This NEVER fakes a signature: if minisign is not installed, or the secret
# key is absent, it prints an honest warning and exits 0 WITHOUT writing any
# .minisig (downstream verification will simply find no signature, which is
# the truthful state).
#
# Inputs (env):
#   MINISIGN_SECRET_KEY   Path to the minisign secret key file, OR the key
#                         material itself (auto-detected; written to a 0600
#                         temp file when inline). Lives ONLY in a CI secret.
#   MINISIGN_PASSWORD     Password for the secret key ("" if the key is
#                         passwordless). Optional.
#   DIST_DIR              Artifact directory (default: dist).
#
# Usage:  ./scripts/sign-artifacts.sh
# ----------------------------------------------------------------------------
set -eu

DIST_DIR="${DIST_DIR:-dist}"

if ! command -v minisign >/dev/null 2>&1; then
  echo "::warning::minisign not installed — skipping free-signing tier. Install from https://jedisct1.github.io/minisign/ (BSD-2, no cost). Artifacts remain checksum-verified."
  exit 0
fi

if [ -z "${MINISIGN_SECRET_KEY:-}" ]; then
  echo "::warning::MINISIGN_SECRET_KEY absent — artifacts UNSIGNED (minisign tier skipped). Generate a key with scripts/gen-minisign-key.sh and store it as the MINISIGN_SECRET_KEY secret. NEVER faked."
  exit 0
fi

if [ ! -d "$DIST_DIR" ]; then
  echo "No $DIST_DIR/ directory — nothing to sign."
  exit 0
fi

# Resolve the secret key to a file path. Accept either a path or inline key
# material (CI secrets are commonly the material itself).
SECRET_FILE=""
CLEANUP=0
if [ -f "$MINISIGN_SECRET_KEY" ]; then
  SECRET_FILE="$MINISIGN_SECRET_KEY"
else
  SECRET_FILE="$(mktemp)"
  CLEANUP=1
  chmod 600 "$SECRET_FILE"
  printf '%s\n' "$MINISIGN_SECRET_KEY" >"$SECRET_FILE"
fi
# shellcheck disable=SC2064
trap '[ "$CLEANUP" -eq 1 ] && rm -f "$SECRET_FILE"' EXIT INT TERM

TRUSTED_COMMENT="Itasha.Corp F0RG3-W1R3 signed artifact $(date -u +%Y-%m-%dT%H:%M:%SZ)"
SIGNED=0
for f in "$DIST_DIR"/*; do
  [ -f "$f" ] || continue
  case "$f" in
    *.minisig | *checksum.sha256) continue ;;
  esac
  # minisign reads the key password from stdin when stdin is not a TTY; a
  # trailing newline submits an empty password for a passwordless CI key
  # (recommended). NEVER fakes — a failure aborts the release loudly.
  printf '%s\n' "${MINISIGN_PASSWORD:-}" | minisign -S -s "$SECRET_FILE" \
    -c "minisign signature" -t "$TRUSTED_COMMENT" -m "$f" >/dev/null 2>&1 || {
    echo "::error::minisign failed to sign $f (check MINISIGN_SECRET_KEY / MINISIGN_PASSWORD; CI keys should be passwordless)"
    exit 1
  }
  echo "  signed: $(basename "$f").minisig"
  SIGNED=$((SIGNED + 1))
done

echo "==> minisign: signed $SIGNED artifact(s) in $DIST_DIR/."
