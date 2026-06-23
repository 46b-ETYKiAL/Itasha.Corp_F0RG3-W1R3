#!/usr/bin/env sh
# ============================================================================
# coverage.sh — installer-crate line-coverage gate (local + CI parity)
# ============================================================================
# Runs cargo-llvm-cov over the headless installer crate and enforces a
# line-coverage floor. The floor protects the privacy-critical W1TN3SS reporting
# integration (installer/src/reporting.rs), which is covered to 100% of its
# testable surface; the genuinely-uncoverable network/daemon/cross-platform-cfg
# lines are documented in docs/coverage-reporting.md (never faked, never
# suppressed).
#
# Usage: scripts/coverage.sh [FLOOR]
#   FLOOR   minimum line-coverage percent (default 94)
#
# Requires: cargo-llvm-cov (cargo install cargo-llvm-cov).
set -eu

FLOOR="${1:-94}"

# Resolve repo root from this script's location so it runs from anywhere.
ROOT="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"

echo "==> cargo llvm-cov (installer crate, headless) — line floor ${FLOOR}%"
cargo llvm-cov \
  --manifest-path "${ROOT}/installer/Cargo.toml" \
  --no-default-features \
  --locked \
  --fail-under-lines "${FLOOR}"

echo "==> coverage gate PASSED (>= ${FLOOR}% lines)"
