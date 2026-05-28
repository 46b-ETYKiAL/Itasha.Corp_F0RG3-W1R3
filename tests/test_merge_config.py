#!/usr/bin/env python3
"""Unit tests for tests/merge_config.py.

Coverage focus:
  * placeholder substitution (${APP_*} + ${ENV:NAME})
  * [itasha] / [itasha.app] table stripping
  * deep-merge precedence (override scalar wins)
  * schema-validity (resolved doc carries no forbidden cargo-packager
    keys, has the renamed installer_mode key, formats filtered to
    known variants, [updater] dropped when pubkey unresolved)
  * fixtures: apps/c0pl4nd.toml + apps/scribe.toml round-trip
  * deterministic emit (re-emit equals original)

Stdlib-only (pytest harness; no third-party dependencies).
"""

from __future__ import annotations

import os
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

# Import the module under test directly.
HERE = Path(__file__).resolve().parent
FRAMEWORK_ROOT = HERE.parent
sys.path.insert(0, str(HERE))

import merge_config  # noqa: E402


# --------------------------------------------------------------------------
# Fixtures
# --------------------------------------------------------------------------

@pytest.fixture
def template_path() -> Path:
    return FRAMEWORK_ROOT / "packager.template.toml"


@pytest.fixture
def c0pl4nd_override() -> Path:
    return FRAMEWORK_ROOT / "apps" / "c0pl4nd.toml"


@pytest.fixture
def scribe_override() -> Path:
    return FRAMEWORK_ROOT / "apps" / "scribe.toml"


# --------------------------------------------------------------------------
# Placeholder substitution
# --------------------------------------------------------------------------

def test_resolves_app_placeholders_from_override_keys(template_path, c0pl4nd_override):
    doc = merge_config.merge(template_path, c0pl4nd_override)
    # APP_PRODUCT_NAME -> product_name from [itasha.app]
    assert doc["product_name"] == "C0PL4ND"
    # APP_IDENTIFIER -> identifier (reverse-DNS)
    assert doc["identifier"] == "corp.itasha.c0pl4nd"
    # APP_DESCRIPTION + APP_LONG_DESCRIPTION
    assert "terminal" in doc["description"].lower()
    assert "GPU" in doc["long_description"]
    # APP_HOMEPAGE
    assert doc["homepage"].startswith("https://github.com/")
    # Icons list resolves all three
    assert doc["icons"] == [
        "branding/c0pl4nd/icon.ico",
        "branding/c0pl4nd/icon-256.png",
        "branding/c0pl4nd/icon.icns",
    ]


def test_default_version_when_override_omits(template_path, c0pl4nd_override):
    """The override carries no `version` field; the merge step fills
    DEFAULT_VERSION so cargo-packager can parse the resolved config."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert doc["version"] == merge_config.DEFAULT_VERSION
    assert doc["version"] == "0.0.0"


def test_binary_path_resolved_from_arg(template_path, c0pl4nd_override, tmp_path):
    fake_binary = tmp_path / "c0pl4nd.exe"
    fake_binary.write_bytes(b"\x4d\x5a")  # MZ header
    doc = merge_config.merge(
        template_path, c0pl4nd_override, binary_path=str(fake_binary)
    )
    assert doc["binaries"]
    entry = doc["binaries"][0]
    assert entry["path"] == str(fake_binary)
    assert entry["main"] is True


def test_binary_entry_pruned_when_no_path(template_path, c0pl4nd_override):
    """Without a binary, the [[binaries]] entry must be dropped
    (cargo-packager rejects an entry missing `path`)."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    # The list may be empty or absent; both are acceptable.
    assert not doc.get("binaries"), "binaries entry should be pruned"


def test_env_placeholder_resolves_when_set(template_path, c0pl4nd_override, monkeypatch):
    monkeypatch.setenv("APPLE_SIGNING_IDENTITY", "Developer ID Application: Itasha")
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert "macos" in doc
    assert doc["macos"]["signing_identity"] == "Developer ID Application: Itasha"


def test_env_placeholder_pruned_when_unset(template_path, c0pl4nd_override, monkeypatch):
    monkeypatch.delenv("APPLE_SIGNING_IDENTITY", raising=False)
    doc = merge_config.merge(template_path, c0pl4nd_override)
    # macos block may still exist (for minimum_system_version), but
    # signing_identity must NOT carry a literal ${ENV:...} placeholder.
    macos = doc.get("macos", {})
    assert "signing_identity" not in macos or "${ENV:" not in macos.get("signing_identity", "")


# --------------------------------------------------------------------------
# [itasha] / [itasha.app] stripping
# --------------------------------------------------------------------------

def test_no_itasha_namespace_in_output(template_path, c0pl4nd_override):
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert "itasha" not in doc, "[itasha] table must be stripped"
    # Belt-and-suspenders: check no nested itasha either.
    for k, v in doc.items():
        if isinstance(v, dict):
            assert "itasha" not in v, f"[{k}.itasha] table must not survive"


def test_no_package_metadata_wrapper_in_output(template_path, c0pl4nd_override):
    """The template uses the embedded-in-Cargo.toml form
    [package.metadata.packager.*]; the merge step must flatten it to
    top-level keys."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert "package" not in doc
    # The flattened keys are at the root.
    assert "product_name" in doc
    assert "nsis" in doc


# --------------------------------------------------------------------------
# Deep-merge precedence + overlays
# --------------------------------------------------------------------------

def test_deb_depends_overlay_applied(template_path, c0pl4nd_override):
    """The c0pl4nd override declares deb_depends; the merge overlay
    must populate [deb].depends with the list."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert doc.get("deb", {}).get("depends") == [
        "libc6", "libxkbcommon0", "libwayland-client0", "libfontconfig1"
    ]


def test_formats_override_replaces_template(template_path, c0pl4nd_override):
    """The override's `formats` replaces the template default, then
    filtered to cargo-packager known variants (drops 'zip')."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    formats = doc["formats"]
    assert "zip" not in formats, "portable-zip is framework-only, must be filtered"
    assert set(formats).issubset(merge_config.CARGO_PACKAGER_KNOWN_FORMATS)
    # The four engine-known formats from the override survive.
    assert {"nsis", "dmg", "appimage", "deb"}.issubset(set(formats))


def test_scribe_override_works_independently(template_path, scribe_override):
    """Second-app smoke: the framework must produce a valid resolved
    config for scribe.toml just as it does for c0pl4nd.toml."""
    doc = merge_config.merge(template_path, scribe_override)
    assert doc["product_name"] == "SCR1B3"
    assert doc["identifier"] == "corp.itasha.scr1b3"
    # scribe omits deb_depends -> [deb] subtable exists but no depends key.
    assert "deb_depends" not in doc.get("deb", {})


# --------------------------------------------------------------------------
# Schema validity (engine renames + alias map)
# --------------------------------------------------------------------------

def test_nsis_installer_mode_alias_applied(template_path, c0pl4nd_override):
    """cargo-packager 0.11.8 renamed install_mode -> installer_mode."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert "install_mode" not in doc.get("nsis", {})
    assert doc["nsis"]["installer_mode"] == "perMachine"


def test_nsis_installer_hooks_inlined_as_preinstall_section(
    template_path, c0pl4nd_override
):
    """The template's installer_hooks file is read and emitted as
    preinstall_section (cargo-packager 0.11.8 has no installer_hooks
    key)."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    nsis = doc["nsis"]
    assert "installer_hooks" not in nsis
    # The hook content must be present (options.nsh is the framework's
    # branded-options file).
    section = nsis.get("preinstall_section", "")
    assert section, "preinstall_section must carry the NSIS hook content"
    # Sanity: at least one recognisable NSIS construct from options.nsh.
    assert "options.nsh" in section or "InstallLocation" in section


def test_updater_dropped_when_pubkey_unresolved(template_path, c0pl4nd_override):
    """The c0pl4nd override sets updater_pubkey to a literal
    ${MINISIGN_PUBLIC_KEY} placeholder (no override for that token).
    The merge step must drop the [updater] block rather than emit a
    half-wired config (gated-not-faked per packager.template.toml NOTES)."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    assert "updater" not in doc, (
        "updater block must be dropped when pubkey did not resolve"
    )


# --------------------------------------------------------------------------
# TOML emit + round-trip
# --------------------------------------------------------------------------

def test_emitted_toml_round_trips(template_path, c0pl4nd_override):
    doc = merge_config.merge(template_path, c0pl4nd_override)
    text = merge_config.emit_toml(doc)
    # Re-parse and compare.
    reparsed = tomllib.loads(text)
    assert reparsed == doc, "emit -> parse must be a fixed point"


def test_emitted_toml_has_no_itasha_table(template_path, c0pl4nd_override):
    doc = merge_config.merge(template_path, c0pl4nd_override)
    text = merge_config.emit_toml(doc)
    assert "[itasha" not in text
    assert "[package.metadata" not in text


def test_emitted_toml_starts_with_top_level_scalars(template_path, c0pl4nd_override):
    """cargo-packager's standalone-config schema expects top-level
    scalars (product_name, identifier, version) — the emit ordering
    must put them before any [table] header."""
    doc = merge_config.merge(template_path, c0pl4nd_override)
    text = merge_config.emit_toml(doc)
    first_table_idx = text.find("\n[")
    pre_table = text[: first_table_idx if first_table_idx > 0 else len(text)]
    assert "product_name" in pre_table
    assert "identifier" in pre_table


# --------------------------------------------------------------------------
# CLI end-to-end
# --------------------------------------------------------------------------

def test_cli_writes_resolved_config(tmp_path):
    out_path = tmp_path / "c0pl4nd.packager.toml"
    rc = merge_config.main(
        ["c0pl4nd", "--root", str(FRAMEWORK_ROOT), "--output", str(out_path)]
    )
    assert rc == 0
    assert out_path.is_file()
    # Resolved config must be parseable.
    with out_path.open("rb") as fh:
        doc = tomllib.load(fh)
    assert doc["product_name"] == "C0PL4ND"


def test_cli_stdout_mode(capsys):
    rc = merge_config.main(
        ["c0pl4nd", "--root", str(FRAMEWORK_ROOT), "--output", "-"]
    )
    assert rc == 0
    out = capsys.readouterr().out
    assert "product_name" in out
    # The framework table must NOT appear in stdout.
    assert "[itasha" not in out


def test_cli_rejects_unknown_app():
    rc = merge_config.main(
        ["nonexistent_app", "--root", str(FRAMEWORK_ROOT), "--output", "-"]
    )
    assert rc == 2


# --------------------------------------------------------------------------
# cargo-packager schema acceptance (end-to-end, skipped if absent)
# --------------------------------------------------------------------------

def _have_cargo_packager() -> bool:
    """Probe for cargo-packager on PATH."""
    try:
        r = subprocess.run(
            ["cargo-packager", "--version"],
            capture_output=True, text=True, timeout=10,
        )
        return r.returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


@pytest.mark.skipif(
    not _have_cargo_packager(),
    reason="cargo-packager not on PATH (install: cargo install cargo-packager --version 0.11.8 --locked)",
)
def test_cargo_packager_accepts_schema(tmp_path):
    """End-to-end: the resolved config must pass cargo-packager's
    schema parser. The probe inlines the config as JSON to
    --config (cargo-packager accepts a raw JSON string)."""
    import json

    out_path = tmp_path / "c0pl4nd.packager.toml"
    rc = merge_config.main(
        ["c0pl4nd", "--root", str(FRAMEWORK_ROOT), "--output", str(out_path)]
    )
    assert rc == 0
    with out_path.open("rb") as fh:
        doc = tomllib.load(fh)
    payload = json.dumps(doc)
    r = subprocess.run(
        ["cargo-packager", "--config", payload],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
        timeout=30,
    )
    combined = (r.stdout or "") + (r.stderr or "")
    # The schema-acceptance gate: NO TOML parse / unknown-field /
    # invalid-type errors. Semantic errors (missing main binary, etc.)
    # are allowed because no real binary is supplied here.
    assert "TOML parse error" not in combined, combined
    assert "unknown field" not in combined, combined
    assert "invalid type" not in combined, combined
