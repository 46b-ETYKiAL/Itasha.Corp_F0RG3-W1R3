#!/usr/bin/env sh
# ============================================================================
# _lib.sh — shared helpers for the installer test-matrix scripts (POSIX sh)
# ============================================================================
# Provides the honest detect-and-skip discipline every matrix script uses:
#   require_tool <name> <install-hint>
#       -> if the tool is on PATH: returns 0
#       -> if absent: prints a LOUD structured SKIP and returns 1 (the caller
#          records the skip and exits with the documented MATRIX_SKIP code).
#
# A skip is NEVER silent and NEVER faked as success. The exit codes are:
#   0   scenario passed
#   1   scenario FAILED (a real assertion failed)
#   77  scenario SKIPPED (a required tool/host is unavailable) — documented,
#       borrowed from the GNU automake convention for "skipped test".
# ----------------------------------------------------------------------------
# MATRIX_SKIP is consumed by sourcing scripts (tests/{linux,macos}/verify.sh
# and the matrix runner) via `exit "$MATRIX_SKIP"`. shellcheck cannot see
# cross-file usage of variables in sourced libraries.
# shellcheck disable=SC2034
MATRIX_SKIP=77

log() { printf '%s\n' "$*"; }
pass() { printf 'PASS: %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; }

# skip <scenario> <reason> — print a structured skip line and signal the code.
skip() {
  _scn="$1"
  shift
  printf 'SKIP: %s :: %s\n' "$_scn" "$*" >&2
  printf 'matrix-skip-reason: tool-or-host-unavailable\n' >&2
}

# require_tool <name> <install-hint> — 0 if present, 1 (with loud skip) if not.
require_tool() {
  _name="$1"
  _hint="$2"
  if command -v "$_name" >/dev/null 2>&1; then
    return 0
  fi
  printf 'SKIP: required tool not found: %s\n' "$_name" >&2
  printf '      install: %s\n' "$_hint" >&2
  printf 'matrix-skip-reason: tool-unavailable: %s\n' "$_name" >&2
  return 1
}

# require_os <expected> — 0 if the running OS matches, 1 (with loud skip) if not.
require_os() {
  _want="$1"
  _have="$(uname -s 2>/dev/null || echo unknown)"
  case "$_want:$_have" in
    windows:*MINGW* | windows:*MSYS* | windows:*CYGWIN*) return 0 ;;
    macos:Darwin) return 0 ;;
    linux:Linux) return 0 ;;
    *)
      printf 'SKIP: scenario requires OS=%s but running on %s\n' "$_want" "$_have" >&2
      printf 'matrix-skip-reason: wrong-host-os\n' >&2
      return 1
      ;;
  esac
}
