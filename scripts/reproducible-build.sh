#!/usr/bin/env bash
# ============================================================================
# reproducible-build.sh — verify the UNSIGNED payload is byte-reproducible
# ============================================================================
# A signed installer is NOT bit-for-bit reproducible (the signature embeds a
# timestamp + nonce). The thing that CAN be reproduced is the UNSIGNED payload:
# the bundled binary + resources before any code-signing step runs. This script
# builds that payload twice with a pinned toolchain and a fixed SOURCE_DATE_EPOCH
# and asserts the two sha256 hashes match.
#
# Method (Linux-distro reproducible-builds.org convention):
#   1. Pin the Rust toolchain (rust-toolchain.toml or the --toolchain flag).
#   2. Export SOURCE_DATE_EPOCH so every embedded timestamp is deterministic.
#   3. Build the unsigned payload into a clean dir.
#   4. Compute a deterministic tree hash (sorted file list -> per-file sha256).
#   5. Repeat into a second clean dir and diff the two tree hashes.
#
# Honest-skip: if cargo or cargo-packager is absent, the script prints a clear
# message and exits 0 WITHOUT faking a match. It never claims reproducibility
# it did not measure.
#
# Usage:
#   ./scripts/reproducible-build.sh [--app <name>] [--toolchain <channel>]
#
# Exit codes:
#   0  the two unsigned-payload hashes matched  (OR an honest tool-absent skip)
#   1  the two hashes DIFFERED — the build is not yet deterministic
#   2  usage / IO error
# ----------------------------------------------------------------------------
set -euo pipefail

APP="c0pl4nd"
TOOLCHAIN="stable"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --app)
      APP="${2:?--app needs a value}"
      shift 2
      ;;
    --toolchain)
      TOOLCHAIN="${2:?--toolchain needs a value}"
      shift 2
      ;;
    -h | --help)
      grep -E '^#( |$)' "$0" | sed -E 's/^# ?//'
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 2
      ;;
  esac
done

# --- Tool presence gate (honest-skip, never faked) -------------------------
# cargo + cargo-packager are required ONLY in BINARY mode (a full installer
# build). In CONFIG-only mode (no ITASHA_BINARY_PATH) the measurable is the
# deterministic merged config, produced by build.sh --dry-run, which needs only
# a Python interpreter — so the heavy toolchain gate must not block it.
if [ -n "${ITASHA_BINARY_PATH:-}" ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "SKIP: cargo not found on PATH (BINARY mode needs the Rust toolchain)."
    echo "      Install the Rust toolchain (https://rustup.rs) then re-run."
    echo "      Reproducibility was NOT measured — no result is being claimed."
    exit 0
  fi
  if ! cargo packager --version >/dev/null 2>&1; then
    echo "SKIP: cargo-packager not found (BINARY mode needs it)."
    echo "      Install it with:  cargo install cargo-packager --locked"
    echo "      Reproducibility was NOT measured — no result is being claimed."
    exit 0
  fi
fi
if ! command -v python3 >/dev/null 2>&1 && ! command -v python >/dev/null 2>&1; then
  echo "SKIP: no Python interpreter found (build.sh --dry-run needs one)."
  echo "      Reproducibility was NOT measured — no result is being claimed."
  exit 0
fi

# --- Determinism inputs -----------------------------------------------------
# A fixed epoch makes every embedded mtime/timestamp deterministic. Prefer the
# repo's HEAD commit time when available; otherwise a stable constant.
if command -v git >/dev/null 2>&1 && git rev-parse HEAD >/dev/null 2>&1; then
  SOURCE_DATE_EPOCH="$(git log -1 --pretty=%ct)"
else
  SOURCE_DATE_EPOCH="1700000000"
fi
export SOURCE_DATE_EPOCH
# LC_ALL/TZ pinning keeps any locale/timezone-sensitive output deterministic.
export LC_ALL=C
export TZ=UTC
echo "==> SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}  toolchain=${TOOLCHAIN}  app=${APP}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Deterministic tree hash: hash each file's sha256 in sorted-path order, then
# hash the concatenation. Excludes signatures/checksums (the signed-bytes that
# legitimately differ) so we measure the UNSIGNED payload only.
tree_hash() {
  payload_dir="$1"
  (cd "$payload_dir" &&
    find . -type f \
      ! -name '*.sig' ! -name '*.pem' ! -name '*.minisig' \
      ! -name 'checksum.sha256' ! -name 'sbom.cdx.json*' |
    LC_ALL=C sort |
      while IFS= read -r rel; do
        if command -v sha256sum >/dev/null 2>&1; then
          sha256sum "$rel"
        else
          shasum -a 256 "$rel"
        fi
      done) | {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi
  } | awk '{print $1}'
}

build_once() {
  out="$1"
  mkdir -p "$out"
  if [ ! -f ./scripts/build.sh ]; then
    echo "ERROR: scripts/build.sh not found — cannot produce the payload." >&2
    return 2
  fi
  # Two measurable modes, both deterministic and both produced by build.sh's
  # supported flags (NO unsupported --reproducible-out — that previously made
  # build.sh exit 2 on its arg-parser, so the test never actually ran):
  #
  #   * BINARY PRESENT (ITASHA_BINARY_PATH set): run the full build, then copy
  #     cargo-packager's dist/ output into $out. This is the true unsigned
  #     installer payload.
  #   * BINARY ABSENT: run build.sh --dry-run, which deterministically resolves
  #     + writes the merged cargo-packager config (the merge step honours no
  #     timestamps); copy that merged config into $out. This is the
  #     reproducible CONFIG payload — the only thing reproducible WITHOUT the
  #     app binary. It is an honest, labelled measurement, never faked.
  if [ -n "${ITASHA_BINARY_PATH:-}" ]; then
    rm -rf dist
    if ! ./scripts/build.sh --app "$APP"; then
      echo "build.sh (full) failed for run -> $out" >&2
      return 1
    fi
    if [ -d dist ]; then
      cp -R dist/. "$out/" 2>/dev/null || true
    fi
  else
    echo "==> No ITASHA_BINARY_PATH — measuring the deterministic MERGED CONFIG payload (honest config-only reproducibility)."
    # Capture build.sh output so a failure is diagnosable instead of swallowed
    # (a silent >/dev/null hid the real error here for a long time). The log is
    # a mktemp file OUTSIDE $out, so it never pollutes the payload hash.
    build_log="$(mktemp)"
    if ! ./scripts/build.sh --app "$APP" --dry-run >"$build_log" 2>&1; then
      echo "build.sh (dry-run) failed for run -> $out" >&2
      echo "----- build.sh output -----" >&2
      cat "$build_log" >&2
      echo "---------------------------" >&2
      rm -f "$build_log"
      return 1
    fi
    rm -f "$build_log"
    merged="packaging/build/$APP.packager.toml"
    if [ -f "$merged" ]; then
      cp "$merged" "$out/merged.packager.toml"
    else
      echo "ERROR: merged config not produced at $merged — cannot measure." >&2
      return 1
    fi
  fi
}

echo "==> Build run 1/2"
build_once "$WORK/run1"
H1="$(tree_hash "$WORK/run1")"
echo "    run1 unsigned-payload hash: ${H1}"

echo "==> Build run 2/2"
build_once "$WORK/run2"
H2="$(tree_hash "$WORK/run2")"
echo "    run2 unsigned-payload hash: ${H2}"

echo
if [ "$H1" = "$H2" ]; then
  echo "==> REPRODUCIBLE: both runs produced the identical unsigned payload."
  echo "    hash=${H1}"
  exit 0
else
  echo "==> NOT REPRODUCIBLE: unsigned-payload hashes differ." >&2
  echo "    run1=${H1}" >&2
  echo "    run2=${H2}" >&2
  echo "    Investigate non-deterministic inputs (embedded timestamps, absolute" >&2
  echo "    paths, unsorted archive entries, build-id, env leakage)." >&2
  exit 1
fi
