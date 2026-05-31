#!/usr/bin/env python3
"""Fill package-manager manifest placeholders from released artifacts.

Reads the sha256 of each uploaded installer artifact from the release
directory (preferring an existing ``checksum.sha256`` index, else computing it)
and substitutes the version / URL / hash placeholders in the winget, Homebrew
cask, and Scoop manifest templates.

It NEVER hand-edits hashes — it derives them from the actual artifact bytes, so
a published manifest can never drift from the file it points at (the WezTerm
#7713 hash-drift footgun).

AUTOMATION (preferred): the WezTerm #7713 hash-drift class is now closed
STRUCTURALLY by the managed package-manager auto-bump workflow
(``.github/workflows/package-bump.yml``): WinGet Releaser (Komac) and Homebrew
bump-cask each download the signed release asset and compute the hash
themselves before opening a bump PR, and the Scoop manifest carries
``checkver`` + ``autoupdate`` so ``scoop update`` recomputes the hash
autonomously. No manual SHA edit is ever required in the published flow.

This script remains valid and useful as the OFFLINE / LOCAL fallback: it
resolves the manifest templates from artifact bytes on disk for inspection or
for a manual, user-authorized submission when the managed actions are not in
play. It still NEVER hand-edits a hash.

GATING: if no installer artifact is found for a given platform, that platform's
manifest is skipped with a loud structured message and the script exits 0
(nothing to resolve is not an error). A usage/IO error exits 2.

The script does NOT submit anything to any package-manager repository; it only
emits resolved manifests for a separate, user-authorized submission step.

Exit codes: 0 ok (resolved or honestly-skipped); 2 usage/IO error.
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

# Artifact suffix -> which manifests it feeds.
WINDOWS_SUFFIXES = (".exe", "-setup.exe", ".nsis.exe")
MACOS_SUFFIXES = (".dmg",)


def _sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _find_artifact(release_dir: Path, suffixes: tuple[str, ...]) -> Path | None:
    for p in sorted(release_dir.rglob("*")):
        if p.is_file() and any(p.name.lower().endswith(s) for s in suffixes):
            return p
    return None


def _release_url(repo: str, version: str, filename: str) -> str:
    # Conventional GitHub Release asset URL.
    return f"https://github.com/{repo}/releases/download/v{version}/{filename}"


def _write_resolved(template: Path, out: Path, replacements: dict[str, str]) -> None:
    text = template.read_text(encoding="utf-8")
    for needle, value in replacements.items():
        text = text.replace(needle, value)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8")
    print(
        f"resolved {out.relative_to(out.parents[1]) if len(out.parents) > 1 else out.name}"
    )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--version", required=True, help="release version (no leading v)")
    ap.add_argument("--release-dir", required=True, type=Path)
    ap.add_argument("--manifest-root", required=True, type=Path)
    ap.add_argument("--out-dir", required=True, type=Path)
    ap.add_argument("--repo", required=True, help="owner/repo for asset URLs")
    args = ap.parse_args(argv[1:])

    release_dir: Path = args.release_dir
    mroot: Path = args.manifest_root
    out: Path = args.out_dir
    version: str = args.version

    if not release_dir.is_dir():
        print(f"ERROR: release dir not found: {release_dir}", file=sys.stderr)
        return 2
    if not mroot.is_dir():
        print(f"ERROR: manifest root not found: {mroot}", file=sys.stderr)
        return 2

    win = _find_artifact(release_dir, WINDOWS_SUFFIXES)
    mac = _find_artifact(release_dir, MACOS_SUFFIXES)

    # App-aware discovery: every winget THREE-FILE set in the manifest root is
    # resolved, not just C0PL4ND. The set is keyed by the version-manifest file
    # (ItashaCorp.<APP>.yaml has no .installer/.locale infix); its two siblings
    # are derived by name. Scoop + Homebrew sets are discovered by globbing the
    # per-PM directories. This makes a new app a non-fork addition (SCR1B3 and
    # any future app resolve automatically — maintainability axis).
    if win is None:
        print("SKIP winget/scoop: no Windows installer artifact in release dir.")
    else:
        win_sha = _sha256_of(win)
        win_url = _release_url(args.repo, version, win.name)
        winget_dir = mroot / "winget"
        # Version manifests are the files WITHOUT an .installer/.locale infix.
        version_manifests = sorted(
            p
            for p in winget_dir.glob("*.yaml")
            if ".installer." not in p.name and ".locale." not in p.name
        )
        for version_src in version_manifests:
            stem = version_src.stem  # e.g. ItashaCorp.SCR1B3
            for name in (
                f"{stem}.installer.yaml",
                f"{stem}.locale.en-US.yaml",
                version_src.name,
            ):
                src = winget_dir / name
                if src.is_file():
                    _write_resolved(
                        src,
                        out / "winget" / name,
                        {
                            "${VERSION}": version,
                            "${INSTALLER_URL}": win_url,
                            "${INSTALLER_SHA256}": win_sha,
                        },
                    )
        # scoop — every *.json manifest in the scoop dir.
        scoop_dir = mroot / "scoop"
        if scoop_dir.is_dir():
            for scoop_src in sorted(scoop_dir.glob("*.json")):
                _write_resolved(
                    scoop_src,
                    out / "scoop" / scoop_src.name,
                    {
                        "__VERSION__": version,
                        "__INSTALLER_URL__": win_url,
                        "__INSTALLER_SHA256__": win_sha,
                    },
                )

    if mac is None:
        print("SKIP homebrew: no macOS .dmg artifact in release dir.")
    else:
        mac_sha = _sha256_of(mac)
        mac_url = _release_url(args.repo, version, mac.name)
        homebrew_dir = mroot / "homebrew"
        # homebrew — every *.rb cask in the homebrew dir.
        if homebrew_dir.is_dir():
            for cask_src in sorted(homebrew_dir.glob("*.rb")):
                _write_resolved(
                    cask_src,
                    out / "homebrew" / cask_src.name,
                    {
                        "__VERSION__": version,
                        "__SHA256__": mac_sha,
                        "__URL__": mac_url,
                    },
                )

    print("manifest update complete (resolved present platforms; skipped absent ones).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
