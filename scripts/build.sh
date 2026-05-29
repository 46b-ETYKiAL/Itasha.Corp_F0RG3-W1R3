#!/usr/bin/env sh
# ============================================================================
# build.sh — Itasha.Corp installer framework build wrapper (POSIX sh)
# ============================================================================
# Merges packager.template.toml with a per-app override and invokes
# cargo-packager. If cargo-packager is NOT installed, prints the exact install
# command and exits non-zero (NO silent success).
#
# Usage:
#   ./scripts/build.sh --app c0pl4nd [--binary <path>] [--dry-run]
#
# Flags:
#   --app <name>     Per-app override to use (apps/<name>.toml). Required.
#   --binary <path>  Resolve the app binary from this path (else $ITASHA_BINARY_PATH).
#   --dry-run        Validate + resolve the config without invoking cargo-packager.
# ----------------------------------------------------------------------------
set -eu

CARGO_PACKAGER_VERSION="0.11.8"
APP=""
BINARY=""
DRY_RUN=0

while [ $# -gt 0 ]; do
  case "$1" in
    --app)     APP="${2:-}"; shift 2 ;;
    --binary)  BINARY="${2:-}"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help)
      echo "Usage: $0 --app <name> [--binary <path>] [--dry-run]"
      exit 0 ;;
    *)
      echo "ERROR: unknown argument: $1" >&2
      exit 2 ;;
  esac
done

if [ -z "$APP" ]; then
  echo "ERROR: --app <name> is required (e.g. --app c0pl4nd)" >&2
  exit 2
fi

SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
ROOT="$(CDPATH='' cd -- "$SCRIPT_DIR/.." && pwd)"
TEMPLATE="$ROOT/packager.template.toml"
OVERRIDE="$ROOT/apps/$APP.toml"

if [ ! -f "$TEMPLATE" ]; then
  echo "ERROR: template not found: $TEMPLATE" >&2
  exit 1
fi
if [ ! -f "$OVERRIDE" ]; then
  echo "ERROR: per-app override not found: $OVERRIDE" >&2
  exit 1
fi

# --- Resolve a Python interpreter (python3 on POSIX, python on Windows shells).
if command -v python3 >/dev/null 2>&1; then
  PY=python3
elif command -v python >/dev/null 2>&1; then
  PY=python
else
  echo "ERROR: no Python interpreter found (need python3 or python on PATH)." >&2
  exit 127
fi

# --- Validate + resolve the merged config (no external tool needed). ---
echo "==> Validating template + override for app '$APP'"
"$PY" "$ROOT/tests/validate_config.py" "$TEMPLATE" "$OVERRIDE"

# --- Resolve the binary input path (binary-input-contract.md). ---
RESOLVED_BINARY="${BINARY:-${ITASHA_BINARY_PATH:-}}"
if [ "$DRY_RUN" -eq 1 ]; then
  echo "==> Dry run: config resolves; skipping cargo-packager invocation."
  if [ -n "$RESOLVED_BINARY" ]; then
    echo "    binary input: $RESOLVED_BINARY"
  else
    echo "    binary input: (none supplied — dry run uses a placeholder)"
  fi
  exit 0
fi

if [ -z "$RESOLVED_BINARY" ]; then
  echo "ERROR: no binary input. Pass --binary <path> or set ITASHA_BINARY_PATH." >&2
  echo "       See docs/binary-input-contract.md." >&2
  exit 1
fi
if [ ! -f "$RESOLVED_BINARY" ]; then
  echo "ERROR: binary input does not exist: $RESOLVED_BINARY" >&2
  exit 1
fi

# --- Require cargo-packager; fail loudly if missing (no silent success). ---
if ! command -v cargo-packager >/dev/null 2>&1; then
  echo "ERROR: cargo-packager is not installed." >&2
  echo "" >&2
  echo "Install the pinned version with:" >&2
  echo "    cargo install cargo-packager --version $CARGO_PACKAGER_VERSION --locked" >&2
  echo "" >&2
  echo "Then re-run: $0 --app $APP --binary $RESOLVED_BINARY" >&2
  exit 127
fi

echo "==> Invoking cargo-packager for app '$APP'"
cd "$ROOT"
# cargo-packager reads the merged config; the binary path is passed through the
# resolved config produced by the validator's merge step.
ITASHA_BINARY_PATH="$RESOLVED_BINARY" cargo-packager --config "apps/$APP.toml"
