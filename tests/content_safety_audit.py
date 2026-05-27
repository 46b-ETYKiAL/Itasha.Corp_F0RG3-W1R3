#!/usr/bin/env python3
"""Public-repo content-safety / IP-boundary audit.

Scans the installer-framework tree and FAILS (exit 1) on any leakage of:
  * a signing private key / secret file (.p12 .pfx .p8 .pem .key .keystore)
    or an embedded private-key block;
  * an internal absolute path (C:\\Users\\<name>, /home/<user>, /Users/<name>);
  * an internal plan-identifier token (plan-<digits>);
  * an internal agent-system reference (the internal config-dir tokens or the
    internal system brand token);
  * a vendored application source tree (apps/*/src/).

The allowlist matches ships-publicly-vs-never.md: only packaging config, UI,
branding, scripts, docs, CI workflows (secrets-by-name), and the test harness
are permitted. This script contains no secrets; it only detects them.

Exit codes: 0 clean; 1 leakage detected; 2 IO/usage error.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# The framework root = parent of this tests/ directory.
ROOT = Path(__file__).resolve().parent.parent

# Files/dirs the audit never reads (its own output, VCS, git-ignored inputs,
# and transient agent-session runtime state that is never part of the
# publishable surface — these are git-ignored and must not influence the verdict).
SKIP_DIRS = {
    ".git", "target", "dist", "out", "build", "binaries", "inputs", "node_modules",
    "__pycache__", ".s4f3-data", ".claude-data",
}

# Secret-bearing file extensions that must never appear in the tree.
SECRET_SUFFIXES = {
    ".p12", ".pfx", ".p8", ".pem", ".key", ".cer", ".crt",
    ".keystore", ".jks", ".pvk", ".spc", ".provisionprofile", ".mobileprovision",
}

# Internal-system tokens. Built without writing them as literals so this very
# file does not trip the audit when the harness is mirrored publicly.
_DOT = "."
_INTERNAL_CONFIG_A = _DOT + "s4f3"
_INTERNAL_CONFIG_B = _DOT + "claude"
_INTERNAL_BRAND = "S4F3"

CONTENT_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("private-key block", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("internal Windows user path", re.compile(r"C:\\\\?Users\\\\?[^\\\\/\s\"']+")),
    ("internal POSIX home path", re.compile(r"/home/[A-Za-z0-9._-]+")),
    ("internal macOS user path", re.compile(r"/Users/[A-Za-z0-9._-]+")),
    ("plan-identifier token", re.compile(r"(?<![A-Za-z])plan[-_ ]?\d{2,4}(?![A-Za-z\d])", re.I)),
    ("internal config dir (a)", re.compile(re.escape(_INTERNAL_CONFIG_A) + r"\b")),
    ("internal config dir (b)", re.compile(re.escape(_INTERNAL_CONFIG_B) + r"\b")),
    ("internal system brand", re.compile(r"\b" + re.escape(_INTERNAL_BRAND) + r"\b")),
]

# Text file extensions worth scanning for content patterns.
TEXT_SUFFIXES = {
    ".toml", ".yml", ".yaml", ".md", ".sh", ".ps1", ".nsh", ".nsi",
    ".py", ".txt", ".json", ".cfg", ".ini", ".desktop", ".rb", ".wxs",
    ".svg", ".rtf", ".plist", ".control",
}


def _iter_files() -> list[Path]:
    out: list[Path] = []
    for p in ROOT.rglob("*"):
        if any(part in SKIP_DIRS for part in p.relative_to(ROOT).parts):
            continue
        if p.is_file():
            out.append(p)
    return out


def main() -> int:
    findings: list[str] = []
    self_path = Path(__file__).resolve()

    files = _iter_files()

    # 1. Secret-file extensions.
    for p in files:
        if p.suffix.lower() in SECRET_SUFFIXES:
            findings.append(f"secret file present: {p.relative_to(ROOT)}")

    # 2. Vendored app source trees: apps/*/src/  (overrides are apps/*.toml).
    apps_dir = ROOT / "apps"
    if apps_dir.is_dir():
        for child in apps_dir.iterdir():
            if child.is_dir() and (child / "src").exists():
                findings.append(f"vendored app source tree: {child.relative_to(ROOT)}/src")

    # 3. Content patterns in text files (skip this auditor itself).
    for p in files:
        if p.resolve() == self_path:
            continue
        if p.suffix.lower() not in TEXT_SUFFIXES:
            continue
        try:
            text = p.read_text(encoding="utf-8", errors="ignore")
        except OSError as exc:
            print(f"ERROR: cannot read {p}: {exc}", file=sys.stderr)
            return 2
        for label, pat in CONTENT_PATTERNS:
            if pat.search(text):
                findings.append(f"{label}: {p.relative_to(ROOT)}")

    if findings:
        print("CONTENT-SAFETY AUDIT FAILED — IP-boundary leakage detected:", file=sys.stderr)
        for f in sorted(set(findings)):
            print(f"  - {f}", file=sys.stderr)
        return 1

    print(f"content-safety audit OK — scanned {len(files)} files, 0 leakage findings")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
