#!/usr/bin/env python3
"""Validate the shared packager template and per-app overrides.

Checks, for each argument:
  * the file is valid TOML (parses with stdlib tomllib);
  * the shared template carries the [itasha] framework marker with the
    company defaults (publisher, windows_install_root, NSIS install_mode);
  * each per-app override declares the required [itasha.app] keys and resolves
    to a Windows install directory of  <windows_install_root>\\<install_subdir>.

Exit codes: 0 all valid; 1 a validation failure; 2 a usage / IO error.

No third-party dependencies; tomllib is stdlib on Python 3.11+.
"""

from __future__ import annotations

import sys
from pathlib import Path

import tomllib

REQUIRED_APP_KEYS = ("product_name", "identifier", "install_subdir", "binary")


def _load(path: Path) -> dict:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def _is_template(data: dict) -> bool:
    itasha = data.get("itasha", {})
    return "template_version" in itasha and "windows_install_root" in itasha


def validate_template(path: Path, data: dict) -> list[str]:
    errors: list[str] = []
    itasha = data.get("itasha", {})
    if itasha.get("publisher") != "Itasha.Corp":
        errors.append("template [itasha].publisher must be 'Itasha.Corp'")
    root = itasha.get("windows_install_root", "")
    if not root.endswith("Itasha.Corp"):
        errors.append(
            "template [itasha].windows_install_root must end with 'Itasha.Corp' "
            f"(got {root!r})"
        )
    nsis = (
        data.get("package", {}).get("metadata", {}).get("packager", {}).get("nsis", {})
    )
    if nsis.get("install_mode") != "perMachine":
        errors.append("template NSIS install_mode must be 'perMachine' (D4)")
    return errors


def validate_override(path: Path, data: dict, template: dict | None) -> list[str]:
    errors: list[str] = []
    app = data.get("itasha", {}).get("app", {})
    if not app:
        errors.append(f"{path.name}: missing [itasha.app] table")
        return errors
    for key in REQUIRED_APP_KEYS:
        if not app.get(key):
            errors.append(f"{path.name}: [itasha.app].{key} is required")
    # Resolve the Windows install directory against the template root.
    if template is not None and not errors:
        root = template["itasha"]["windows_install_root"]
        subdir = app["install_subdir"]
        resolved = root.rstrip("\\") + "\\" + subdir
        print(f"OK  {path.name:<28} (resolved install dir = {resolved})")
    elif not errors:
        print(f"OK  {path.name:<28} (override valid; no template to resolve against)")
    return errors


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(
            "usage: validate_config.py <file.toml> [<file.toml> ...]", file=sys.stderr
        )
        return 2

    paths = [Path(a) for a in argv[1:]]
    parsed: dict[Path, dict] = {}
    template_data: dict | None = None

    # First pass: parse everything, locate the template.
    for p in paths:
        if not p.is_file():
            print(f"ERROR: not a file: {p}", file=sys.stderr)
            return 2
        try:
            data = _load(p)
        except tomllib.TOMLDecodeError as exc:
            print(f"FAIL {p.name}: invalid TOML — {exc}", file=sys.stderr)
            return 1
        parsed[p] = data
        if _is_template(data):
            template_data = data

    all_errors: list[str] = []
    for p, data in parsed.items():
        if _is_template(data):
            errs = validate_template(p, data)
            if not errs:
                print(
                    f"OK  {p.name:<28} (valid TOML, [itasha] template marker present)"
                )
            all_errors.extend(errs)
        else:
            all_errors.extend(validate_override(p, data, template_data))

    if all_errors:
        print(file=sys.stderr)
        for e in all_errors:
            print(f"FAIL {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
