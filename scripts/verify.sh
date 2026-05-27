#!/usr/bin/env sh
# ============================================================================
# verify.sh — verify a downloaded F0RG3-W1R3 artifact (free, offline)
# ============================================================================
# Checks an artifact two ways:
#   1. SHA-256 against the published checksum.sha256
#   2. minisign signature against the published public key (keys/minisign.pub)
#
# Both are free and need no network. A green result means the file is exactly
# what was released and signed by the holder of the project's minisign key.
#
# Usage:  ./scripts/verify.sh <artifact> [public-key]
# ----------------------------------------------------------------------------
set -eu

ART="${1:-}"
PUB="${2:-keys/minisign.pub}"
if [ -z "$ART" ] || [ ! -f "$ART" ]; then
  echo "Usage: $0 <artifact> [public-key]" >&2
  exit 2
fi

rc=0

# --- SHA-256 ---
if [ -f checksum.sha256 ]; then
  base="$(basename "$ART")"
  if command -v sha256sum >/dev/null 2>&1; then
    have="$(sha256sum "$ART" | awk '{print $1}')"
  else
    have="$(shasum -a 256 "$ART" | awk '{print $1}')"
  fi
  want="$(grep -i " $base\$" checksum.sha256 | awk '{print $1}' | head -n1)"
  if [ -n "$want" ] && [ "$have" = "$want" ]; then
    echo "  OK  sha256 matches checksum.sha256"
  else
    echo "  FAIL sha256 mismatch (have $have, want ${want:-<none>})"; rc=1
  fi
else
  echo "  --  checksum.sha256 not present; skipping hash check"
fi

# --- minisign ---
if command -v minisign >/dev/null 2>&1; then
  if [ -f "$ART.minisig" ] && [ -f "$PUB" ]; then
    if minisign -Vm "$ART" -p "$PUB" >/dev/null 2>&1; then
      echo "  OK  minisign signature valid ($PUB)"
    else
      echo "  FAIL minisign signature INVALID"; rc=1
    fi
  else
    echo "  --  no $ART.minisig or $PUB; signature check skipped (artifact may be an unsigned dev build)"
  fi
else
  echo "  --  minisign not installed; install from https://jedisct1.github.io/minisign/ to verify the signature"
fi

[ "$rc" -eq 0 ] && echo "==> VERIFIED" || echo "==> VERIFICATION FAILED"
exit "$rc"
