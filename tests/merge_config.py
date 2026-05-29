#!/usr/bin/env python3
"""Merge the shared packager template with a per-app override.

Produces a cargo-packager-acceptable config for the standalone
`cargo packager --config <file>` invocation form. This closes
architectural-gap A documented in qa-report-2026-05-28.md: the
template carries an `[itasha]` framework table and an
`[package.metadata.packager.*]` embedded-in-Cargo.toml-shaped block
with `${APP_*}` placeholders; the per-app override carries an
`[itasha.app]` table; cargo-packager rejects both shapes when invoked
standalone. This module performs the documented merge step:

  1. Load template + override (both stdlib tomllib).
  2. Resolve every `${APP_*}` placeholder from override fields +
     `${ENV:NAME}` from os.environ.
  3. Deep-merge override-derived cargo-packager keys into the
     template's `[package.metadata.packager.*]` block.
  4. Strip the framework-only `[itasha]` and `[itasha.app]` tables.
  5. Flatten `[package.metadata.packager.*]` -> top-level (the
     standalone-cargo-packager schema form).
  6. Drop any subtable whose values are still unresolved `${ENV:NAME}`
     placeholders (cargo-packager rejects unknown keys; an unset CI
     secret must NOT leak through as a literal placeholder string).
  7. Emit TOML to the output path (default
     `packaging/build/<app>.packager.toml`) or to stdout (`-`).

Exit codes: 0 on success; 1 on validation/merge failure; 2 on usage
or IO error.

No third-party dependencies; tomllib is stdlib on Python 3.11+, and
the TOML emitter is hand-rolled (we do NOT bind to tomli_w / tomlkit
to honour the engine-agnostic discipline).
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path
from typing import Any

import tomllib

# ---------------------------------------------------------------------------
# Placeholder resolution
# ---------------------------------------------------------------------------

# ${APP_*} placeholders the template carries. The merge step fills them
# from the per-app override's [itasha.app] keys via this mapping.
APP_PLACEHOLDER_MAP: dict[str, str] = {
    "APP_PRODUCT_NAME": "product_name",
    "APP_IDENTIFIER": "identifier",
    "APP_VERSION": "version",
    "APP_HOMEPAGE": "homepage",
    "APP_DESCRIPTION": "description",
    "APP_LONG_DESCRIPTION": "long_description",
    "APP_ICON_ICO": "icon_ico",
    "APP_ICON_PNG": "icon_png",
    "APP_ICON_ICNS": "icon_icns",
    "APP_BINARY_PATH": "binary_path",
    "APP_BINARY": "binary",
    "APP_INSTALL_SUBDIR": "install_subdir",
    "APP_DESKTOP_FILE": "desktop_file",
    "APP_UPDATER_PUBKEY": "updater_pubkey",
    "APP_UPDATER_ENDPOINT": "updater_endpoint",
}

# Default version when the override does not set one. cargo-packager
# requires `version` on the resolved config; the per-app override is
# expected to supply it for releases, but a default lets dry-runs
# succeed and lets CI fill from the build-time tag.
DEFAULT_VERSION = "0.0.0"

_PLACEHOLDER_RE = re.compile(r"\$\{([A-Z][A-Z0-9_:]*)\}")


def _resolve_app_value(token: str, app: dict[str, Any]) -> str | None:
    """Resolve a single ${APP_*} placeholder via APP_PLACEHOLDER_MAP."""
    key = APP_PLACEHOLDER_MAP.get(token)
    if key is None:
        return None
    val = app.get(key)
    if val is None:
        return None
    return str(val)


def _resolve_env_value(token: str) -> str | None:
    """Resolve a single ${ENV:NAME} placeholder from os.environ."""
    if not token.startswith("ENV:"):
        return None
    return os.environ.get(token[4:])


def _resolve_placeholders(
    value: Any,
    app: dict[str, Any],
    *,
    binary_path: str | None,
    unresolved: list[str],
    unresolved_env: list[str],
) -> Any:
    """Recursively substitute ${APP_*} and ${ENV:NAME} placeholders.

    `unresolved` collects ${APP_*} tokens that have no mapping
    (fatal — fail loud). `unresolved_env` collects ${ENV:NAME} tokens
    whose env var is unset (non-fatal — the containing subtable is
    pruned by `_prune_unresolved`).
    """
    if isinstance(value, str):

        def _sub(match: re.Match[str]) -> str:
            token = match.group(1)
            if token == "APP_BINARY_PATH":
                # APP_BINARY_PATH is resolved at build time, not from
                # the override. When no binary is supplied (dry-run /
                # schema-validation), leave the placeholder as a
                # marker that prune_unresolved will strip the
                # containing binaries entry — keeping the resolved
                # config schema-valid for cargo-packager parsing.
                if binary_path is not None:
                    return binary_path
                # rewrite to the env-shape marker so the prune pass
                # drops the containing entry.
                return "${ENV:APP_BINARY_PATH_UNRESOLVED}"
            if token.startswith("ENV:"):
                resolved = _resolve_env_value(token)
                if resolved is None:
                    unresolved_env.append(token)
                    return match.group(0)
                return resolved
            resolved = _resolve_app_value(token, app)
            if resolved is None:
                # APP_UPDATER_* tokens are gated-not-faked: an app
                # may legitimately omit the updater. Rewrite to an
                # env-shape marker so the prune pass drops the
                # containing [updater] subtable rather than
                # surfacing a hard error.
                if token.startswith("APP_UPDATER_"):
                    return "${ENV:" + token + "_UNRESOLVED}"
                unresolved.append(token)
                return match.group(0)
            return resolved

        return _PLACEHOLDER_RE.sub(_sub, value)
    if isinstance(value, dict):
        return {
            k: _resolve_placeholders(
                v,
                app,
                binary_path=binary_path,
                unresolved=unresolved,
                unresolved_env=unresolved_env,
            )
            for k, v in value.items()
        }
    if isinstance(value, list):
        return [
            _resolve_placeholders(
                v,
                app,
                binary_path=binary_path,
                unresolved=unresolved,
                unresolved_env=unresolved_env,
            )
            for v in value
        ]
    return value


_UNRESOLVED_RE = re.compile(r"\$\{[A-Z][A-Z0-9_:]*\}")


def _prune_unresolved(value: Any) -> Any:
    """Recursively drop strings still carrying a ``${TOKEN}`` literal.

    Any remaining ``${...}`` marker means a placeholder did not resolve
    (an unset ``${ENV:NAME}`` secret OR an env-style override token
    like ``${MINISIGN_PUBLIC_KEY}`` for which there was no mapping).
    Strings get dropped; the caller decides whether the surrounding
    container survives (a list-of-dicts with one entry that pruned to
    empty drops the entry; a top-level subtable that pruned to empty
    is removed by the merge driver below).
    """
    if isinstance(value, str):
        if _UNRESOLVED_RE.search(value):
            return None
        return value
    if isinstance(value, dict):
        out: dict[str, Any] = {}
        for k, v in value.items():
            pruned = _prune_unresolved(v)
            if pruned is None:
                continue
            if isinstance(pruned, dict) and not pruned:
                # an empty subtable is still emitted; caller decides
                continue
            if isinstance(pruned, list) and not pruned:
                continue
            out[k] = pruned
        return out
    if isinstance(value, list):
        out_list = []
        for v in value:
            pruned = _prune_unresolved(v)
            if pruned is None:
                continue
            # For an array-of-tables (list of dicts), an entry that
            # became empty after pruning means a required key (path)
            # was stripped — drop the whole entry rather than emit
            # a binaries entry with only `main = true`.
            if isinstance(pruned, dict) and not pruned:
                continue
            if isinstance(pruned, dict) and isinstance(v, dict):
                # binaries entries require `path` — drop the entry
                # if path was pruned out.
                if "path" in v and "path" not in pruned:
                    continue
            out_list.append(pruned)
        return out_list
    return value


# ---------------------------------------------------------------------------
# Deep merge
# ---------------------------------------------------------------------------


def _deep_merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    """Deep-merge `override` into `base`.

    Scalars + lists in `override` replace the value in `base` (the
    template comments imply per-app overrides win; lists are NOT
    additively concatenated to keep the surface predictable and
    match how cargo-packager itself treats per-key settings).
    Dicts recurse.
    """
    out = dict(base)
    for k, v in override.items():
        if k in out and isinstance(out[k], dict) and isinstance(v, dict):
            out[k] = _deep_merge(out[k], v)
        else:
            out[k] = v
    return out


def _override_to_packager_overlay(app: dict[str, Any]) -> dict[str, Any]:
    """Translate per-app [itasha.app] keys into a cargo-packager overlay.

    Keys with no direct cargo-packager equivalent (e.g. install_subdir)
    are consumed by placeholder substitution only and are NOT carried
    into the overlay.
    """
    overlay: dict[str, Any] = {}
    # formats override (replace the template default).
    if "formats" in app:
        overlay["formats"] = list(app["formats"])
    # deb depends override.
    if "deb_depends" in app:
        overlay.setdefault("deb", {})["depends"] = list(app["deb_depends"])
    return overlay


# ---------------------------------------------------------------------------
# TOML emit (hand-rolled, stdlib-only)
# ---------------------------------------------------------------------------


def _toml_escape_str(s: str) -> str:
    # cargo-packager keys/values are simple strings; escape backslash + quote.
    return s.replace("\\", "\\\\").replace('"', '\\"')


def _toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, str):
        # Use TOML multi-line basic strings for any string containing
        # newlines (NSIS hook content, multi-line descriptions, etc.).
        # Triple-quoted basic strings preserve newlines literally and
        # only need backslash + the triple-quote sequence escaped.
        if "\n" in value:
            escaped = value.replace("\\", "\\\\").replace('"""', '\\"""')
            return '"""\n' + escaped + '"""'
        return f'"{_toml_escape_str(value)}"'
    if isinstance(value, list):
        parts = [_toml_value(v) for v in value]
        return "[" + ", ".join(parts) + "]"
    if isinstance(value, dict):
        # inline-table form (used for nested scalar dicts like
        # dmg.window_size = { width = 660, height = 400 }).
        parts = []
        for k, v in value.items():
            parts.append(f"{k} = {_toml_value(v)}")
        return "{ " + ", ".join(parts) + " }"
    raise TypeError(f"unsupported TOML value type: {type(value).__name__}")


def _emit_table(name: str, table: dict[str, Any], buf: list[str]) -> None:
    """Emit a single [table] section with its scalar keys, then recurse
    into sub-tables. Arrays-of-tables (list-of-dict) are emitted as
    [[name.sub]] sections.
    """
    scalars: list[tuple[str, Any]] = []
    subtables: list[tuple[str, dict[str, Any]]] = []
    array_subtables: list[tuple[str, list[dict[str, Any]]]] = []
    for k, v in table.items():
        if isinstance(v, dict):
            # Inline-table heuristic: small all-scalar dict -> inline.
            if all(not isinstance(x, (dict, list)) for x in v.values()) and len(v) <= 3:
                scalars.append((k, v))
            else:
                subtables.append((k, v))
        elif isinstance(v, list) and v and all(isinstance(x, dict) for x in v):
            array_subtables.append((k, v))
        else:
            scalars.append((k, v))

    if name or scalars:
        if name:
            buf.append(f"[{name}]")
        for k, v in scalars:
            buf.append(f"{k} = {_toml_value(v)}")
        if name or scalars:
            buf.append("")

    for k, v in subtables:
        sub_name = f"{name}.{k}" if name else k
        _emit_table(sub_name, v, buf)

    for k, items in array_subtables:
        sub_name = f"{name}.{k}" if name else k
        for item in items:
            buf.append(f"[[{sub_name}]]")
            for ik, iv in item.items():
                buf.append(f"{ik} = {_toml_value(iv)}")
            buf.append("")


def emit_toml(doc: dict[str, Any]) -> str:
    """Emit a TOML document.

    Top-level scalar keys come first; then named tables; then arrays
    of tables.
    """
    buf: list[str] = []
    scalars: list[tuple[str, Any]] = []
    tables: list[tuple[str, dict[str, Any]]] = []
    array_tables: list[tuple[str, list[dict[str, Any]]]] = []
    for k, v in doc.items():
        if isinstance(v, dict):
            tables.append((k, v))
        elif isinstance(v, list) and v and all(isinstance(x, dict) for x in v):
            array_tables.append((k, v))
        else:
            scalars.append((k, v))

    for k, v in scalars:
        buf.append(f"{k} = {_toml_value(v)}")
    if scalars:
        buf.append("")
    for k, v in tables:
        _emit_table(k, v, buf)
    for k, items in array_tables:
        for item in items:
            buf.append(f"[[{k}]]")
            for ik, iv in item.items():
                buf.append(f"{ik} = {_toml_value(iv)}")
            buf.append("")
    return "\n".join(buf).rstrip() + "\n"


# ---------------------------------------------------------------------------
# Top-level merge driver
# ---------------------------------------------------------------------------

# cargo-packager schema keys that must NEVER appear at the document
# root (the merge step strips them). The framework-only [itasha]
# tables are the obvious set; we strip them explicitly.
FRAMEWORK_ONLY_TABLES = ("itasha",)

# cargo-packager v0.11.8 accepted `formats` variants. The framework
# template/override carries a superset ("zip" is the portable-lane
# token documented in packager.template.toml NOTES §5) — the merge
# step filters to engine-known variants and surfaces a stderr note
# so the portable-zip lane is not silently dropped.
CARGO_PACKAGER_KNOWN_FORMATS = frozenset(
    ("all", "default", "app", "dmg", "wix", "nsis", "deb", "appimage", "pacman")
)

# cargo-packager v0.11.8 renamed several keys vs older versions /
# tauri-bundler conventions. The framework template uses the older
# names; the merge step aliases them to the engine's current schema.
# Each entry is keyed by the subtable name (or "" for top-level) and
# maps the framework name -> the engine name.
KEY_ALIASES: dict[str, dict[str, str]] = {
    "nsis": {"install_mode": "installer_mode"},
}


def merge(
    template_path: Path,
    override_path: Path,
    *,
    binary_path: str | None = None,
) -> dict[str, Any]:
    """Load, merge, resolve, strip, and flatten into the standalone
    cargo-packager schema form. Returns the resolved doc as a dict.
    """
    with template_path.open("rb") as fh:
        template = tomllib.load(fh)
    with override_path.open("rb") as fh:
        override = tomllib.load(fh)

    app = override.get("itasha", {}).get("app", {})
    if not app:
        raise SystemExit(
            f"merge_config.py: {override_path.name} is missing [itasha.app]"
        )

    # Fill in documented defaults for placeholders the override may
    # legitimately omit. Version defaults to DEFAULT_VERSION (CI
    # supplies the real one via the override at release time); the
    # binary_path is resolved from the explicit argument below.
    if "version" not in app:
        app = dict(app)
        app["version"] = DEFAULT_VERSION

    # The template's cargo-packager block lives at
    # [package.metadata.packager.*] (embedded-in-Cargo.toml form).
    pkg = template.get("package", {}).get("metadata", {}).get("packager", {})
    if not pkg:
        raise SystemExit(
            "merge_config.py: template missing [package.metadata.packager.*]"
        )

    # Apply the per-app overlay (formats / deb_depends).
    overlay = _override_to_packager_overlay(app)
    merged = _deep_merge(pkg, overlay)

    # Resolve ${APP_*} + ${ENV:NAME} placeholders throughout.
    unresolved: list[str] = []
    unresolved_env: list[str] = []
    resolved = _resolve_placeholders(
        merged,
        app,
        binary_path=binary_path,
        unresolved=unresolved,
        unresolved_env=unresolved_env,
    )
    if unresolved:
        # A required ${APP_*} placeholder has no mapping or value.
        missing = sorted(set(unresolved))
        raise SystemExit(
            "merge_config.py: unresolved ${APP_*} placeholders: " + ", ".join(missing)
        )

    # If the override did not set version, fill the documented default
    # so cargo-packager can parse the resolved config.
    if (
        isinstance(resolved.get("version"), str)
        and "${APP_VERSION}" in resolved["version"]
    ):
        resolved["version"] = DEFAULT_VERSION

    # Prune any value still carrying a literal ${ENV:...} placeholder
    # (unset CI secret). The containing subtable's affected keys are
    # dropped; a subtable left empty is removed at the top level.
    pruned = _prune_unresolved(resolved)

    # The updater subtable specifically must be dropped wholesale if
    # the pubkey or endpoint did not resolve — emitting a half-wired
    # updater config would silently misroute updates.
    updater = pruned.get("updater") if isinstance(pruned, dict) else None
    if isinstance(updater, dict):
        if not updater.get("pubkey") or (
            not updater.get("endpoint") and not updater.get("endpoints")
        ):
            # endpoint/endpoints are the same concept in different schema
            # versions; either being absent is a gate.
            pruned.pop("updater", None)

    # macos.signing_identity: if it was an unset ${ENV:...}, the prune
    # step already dropped it. cargo-packager defaults to unsigned.

    if not isinstance(pruned, dict):
        raise SystemExit("merge_config.py: prune step returned non-dict")

    # Apply key aliases (KEY_ALIASES maps framework names to the
    # engine's current schema names — covers e.g. nsis.install_mode
    # -> nsis.installer_mode rename in cargo-packager v0.11.x).
    for subtable, aliases in KEY_ALIASES.items():
        sub = pruned.get(subtable)
        if isinstance(sub, dict):
            for old, new in aliases.items():
                if old in sub and new not in sub:
                    sub[new] = sub.pop(old)

    # cargo-packager 0.11.8 has no `installer_hooks` key. The
    # framework template uses it to wire packaging/windows/options.nsh
    # into the NSIS script. Map it to `preinstall_section` by reading
    # the file content inline. NOTE: this delivers PARTIAL semantics —
    # options.nsh implements a full options/components page that
    # ideally lives in `nsis.template` (a custom NSIS template), not
    # in a preinstall section. Surface the limitation on stderr so the
    # drop is not silent (per silent-error-discipline).
    nsis = pruned.get("nsis")
    if isinstance(nsis, dict) and "installer_hooks" in nsis:
        hooks_relpath = nsis.pop("installer_hooks")
        # Read the hook file relative to the framework root.
        hooks_path = template_path.parent / hooks_relpath
        if hooks_path.is_file():
            try:
                hooks_content = hooks_path.read_text(encoding="utf-8")
            except OSError as exc:
                # silence-reason: caller-contract
                # File existed at is_file() check but read failed —
                # surface and continue without the hook content.
                print(
                    f"merge_config.py: failed to read {hooks_path}: {exc}; NSIS hook content NOT inlined.",
                    file=sys.stderr,
                )
                hooks_content = None
            if hooks_content is not None:
                nsis["preinstall_section"] = hooks_content
                print(
                    "merge_config.py: inlined "
                    f"{hooks_relpath} as nsis.preinstall_section "
                    "(cargo-packager 0.11.8 has no installer_hooks "
                    "key — full options-page semantics require a "
                    "custom nsis.template; see packager.template.toml "
                    "NOTES §1).",
                    file=sys.stderr,
                )
        else:
            print(
                f"merge_config.py: NSIS hook file not found: {hooks_path}; dropped nsis.installer_hooks.",
                file=sys.stderr,
            )

    # Filter `formats` to cargo-packager's known variants. The
    # framework's portable-zip lane ("zip") is documented in
    # packager.template.toml NOTES §5 as an APP-side behaviour — it
    # cannot be requested of cargo-packager directly. Surface the
    # filtered set on stderr so the drop is not silent.
    formats = pruned.get("formats")
    if isinstance(formats, list):
        kept = [f for f in formats if f in CARGO_PACKAGER_KNOWN_FORMATS]
        dropped = [f for f in formats if f not in CARGO_PACKAGER_KNOWN_FORMATS]
        if dropped:
            print(
                "merge_config.py: filtered framework-only format(s) not "
                "supported by cargo-packager 0.11.8: "
                + ", ".join(sorted(set(dropped)))
                + " (see packager.template.toml NOTES §5).",
                file=sys.stderr,
            )
        pruned["formats"] = kept

    # Flatten is a no-op here because we already plucked
    # [package.metadata.packager.*]; the resulting dict's keys ARE the
    # top-level schema keys (product_name, identifier, version, nsis,
    # dmg, deb, appimage, macos, updater, binaries).
    return pruned


def _strip_unsupported_keys(doc: dict[str, Any]) -> dict[str, Any]:
    """Remove any [itasha*] table that somehow survived (defensive).

    The pluck-from-[package.metadata.packager] already excludes the
    framework tables; this is a belt-and-suspenders guard.
    """
    return {k: v for k, v in doc.items() if k not in FRAMEWORK_ONLY_TABLES}


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        prog="merge_config.py",
        description=(
            "Merge packager.template.toml with apps/<APP>.toml into a cargo-packager-acceptable config (closes gap A)."
        ),
    )
    parser.add_argument(
        "app",
        help="App name (resolves to apps/<APP>.toml).",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="Framework root (default: parent of tests/).",
    )
    parser.add_argument(
        "--binary-path",
        default=None,
        help=(
            "Resolved binary path to substitute for ${APP_BINARY_PATH}. Falls back to env ITASHA_BINARY_PATH if unset."
        ),
    )
    parser.add_argument(
        "--output",
        default=None,
        help=(
            "Output path for the resolved cargo-packager config. "
            "Use '-' to write to stdout. Default: "
            "<root>/packaging/build/<APP>.packager.toml"
        ),
    )
    args = parser.parse_args(argv)

    root = args.root or Path(__file__).resolve().parent.parent
    template = root / "packager.template.toml"
    override = root / "apps" / f"{args.app}.toml"
    if not template.is_file():
        print(f"merge_config.py: template not found: {template}", file=sys.stderr)
        return 2
    if not override.is_file():
        print(f"merge_config.py: override not found: {override}", file=sys.stderr)
        return 2

    binary = args.binary_path or os.environ.get("ITASHA_BINARY_PATH")

    try:
        doc = merge(template, override, binary_path=binary)
    except SystemExit as exc:
        # SystemExit message already carries the diagnostic.
        msg = str(exc) if exc.code != 0 else ""
        if msg:
            print(msg, file=sys.stderr)
        return 1
    doc = _strip_unsupported_keys(doc)

    out_text = emit_toml(doc)

    if args.output == "-":
        sys.stdout.write(out_text)
        return 0

    out_path: Path
    if args.output is None:
        out_path = root / "packaging" / "build" / f"{args.app}.packager.toml"
    else:
        out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(out_text, encoding="utf-8")
    print(f"merge_config.py: wrote {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
