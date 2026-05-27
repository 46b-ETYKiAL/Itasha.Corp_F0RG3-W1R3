#!/usr/bin/env sh
# ============================================================================
# gen-minisign-key.sh — one-time minisign keypair generation (free, no CA)
# ============================================================================
# Generates a minisign Ed25519 keypair for signing F0RG3-W1R3 release
# artifacts. Run this ONCE per project.
#
#   - The PUBLIC key (keys/minisign.pub) is safe to commit and ships in the
#     public repo so downloaders can verify (see scripts/verify.sh).
#   - The SECRET key is PRIVATE: store its contents as the CI secret
#     MINISIGN_SECRET_KEY and DELETE the local copy. It must NEVER be committed
#     (the content-safety audit will reject it).
#
# Usage:  ./scripts/gen-minisign-key.sh [output-secret-path]
# ----------------------------------------------------------------------------
set -eu

if ! command -v minisign >/dev/null 2>&1; then
  echo "ERROR: minisign not installed." >&2
  echo "Install (free, BSD-2): https://jedisct1.github.io/minisign/" >&2
  echo "  Windows:  scoop install minisign   (or winget install jedisct1.minisign)" >&2
  echo "  macOS:    brew install minisign" >&2
  echo "  Linux:    apt install minisign      (or your distro's package)" >&2
  exit 127
fi

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
PUB="$ROOT/keys/minisign.pub"
SECRET_OUT="${1:-$ROOT/minisign.key}"

mkdir -p "$ROOT/keys"
if [ -f "$PUB" ] && grep -q '^untrusted comment' "$PUB" 2>/dev/null; then
  echo "ERROR: $PUB already exists — refusing to overwrite an in-use key." >&2
  echo "       Delete it deliberately if you intend to rotate." >&2
  exit 1
fi

echo "==> Generating minisign keypair"
minisign -G -p "$PUB" -s "$SECRET_OUT"

cat <<EOF

============================================================================
NEXT STEPS (do these now):
  1. Commit the PUBLIC key:        git add keys/minisign.pub && commit
  2. Store the SECRET key in CI:   copy the FULL contents of
                                     $SECRET_OUT
                                   into the GitHub Actions secret
                                     MINISIGN_SECRET_KEY
                                   (Settings → Secrets → Actions). If you set a
                                   password, also set MINISIGN_PASSWORD.
  3. DELETE the local secret:       rm "$SECRET_OUT"
The SECRET key must NEVER be committed — the content-safety audit blocks it.
============================================================================
EOF
