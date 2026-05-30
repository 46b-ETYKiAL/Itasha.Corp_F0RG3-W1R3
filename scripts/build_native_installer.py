#!/usr/bin/env python3
"""Build the native Itasha.Corp CRT installer for a given app.

Reusable across every Itasha.Corp Windows app: this wrapper reads the app's
existing per-app override (``apps/<app>.toml``), zips the compiled payload, sets
the ``ITASHA_*`` build-time config the installer crate consumes, and produces a
single branded ``<app>-<version>-x86_64-setup.exe``.

Usage:
    python scripts/build_native_installer.py \
        --app c0pl4nd \
        --binary-dir path/to/dir/with/c0pl4nd.exe+assets \
        --version v0.1.0 \
        --out dist/c0pl4nd-v0.1.0-x86_64-setup.exe

The override may carry an optional ``[itasha.brand]`` table:
    [itasha.brand]
    voice_hex = "#B48CE8"   # banner voice colour (installer accent)
    kanji     = "端末"       # CRT kanji watermark
    tagline   = "The operator's shell into the wired."
CLI flags (--voice/--kanji/--tagline) override the override; sensible defaults
apply when neither is present.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tomllib
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent  # the F0RG3-W1R3 repo root
INSTALLER = ROOT / "installer"


def load_override(app: str) -> dict:
    p = ROOT / "apps" / f"{app}.toml"
    if not p.is_file():
        sys.exit(f"ERROR: per-app override not found: {p}")
    with p.open("rb") as fh:
        return tomllib.load(fh)


def zip_payload(binary_dir: Path, out_zip: Path) -> int:
    if not binary_dir.is_dir():
        sys.exit(f"ERROR: --binary-dir is not a directory: {binary_dir}")
    out_zip.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with zipfile.ZipFile(out_zip, "w", zipfile.ZIP_DEFLATED) as z:
        for f in sorted(binary_dir.rglob("*")):
            if f.is_file():
                z.write(f, f.relative_to(binary_dir).as_posix())
                count += 1
    return count


def main() -> None:
    # Print UTF-8 regardless of the host console codepage (Windows cp1252 chokes
    # on the kanji watermark). Safe no-op where stdout has no reconfigure.
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[attr-defined]
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser(description="Build the native Itasha.Corp installer for an app.")
    ap.add_argument("--app", required=True, help="per-app override stem (apps/<app>.toml)")
    ap.add_argument("--binary-dir", required=True, type=Path, help="dir holding the compiled binary + assets")
    ap.add_argument("--version", required=True, help="release version (e.g. v0.1.0)")
    ap.add_argument("--out", required=True, type=Path, help="output installer .exe path")
    ap.add_argument("--voice", help="override voice colour hex (e.g. #B48CE8)")
    ap.add_argument("--kanji", help="override CRT kanji watermark")
    ap.add_argument("--tagline", help="override tagline")
    ap.add_argument("--no-build", action="store_true", help="stage config + payload but skip cargo build")
    args = ap.parse_args()

    ov = load_override(args.app)
    app = ov.get("itasha", {}).get("app", {})
    brand = ov.get("itasha", {}).get("brand", {})
    if not app:
        sys.exit(f"ERROR: apps/{args.app}.toml has no [itasha.app] table")

    product = app.get("product_name") or args.app.upper()
    binary = app.get("binary") or args.app
    app_bin = binary if binary.lower().endswith(".exe") else f"{binary}.exe"
    subdir = app.get("install_subdir") or product
    homepage = app.get("homepage", "")
    version = args.version.lstrip("v")
    vendor = "Itasha.Corp"
    voice = args.voice or brand.get("voice_hex") or "#34E0D0"
    kanji = args.kanji or brand.get("kanji") or "端末"
    tagline = (
        args.tagline
        or brand.get("tagline")
        or app.get("description")
        or "An Itasha.Corp application."
    )

    # --- stage payload ---
    payload = INSTALLER / "payload-staging" / f"{args.app}-payload.zip"
    n = zip_payload(args.binary_dir, payload)
    print(f"==> payload: {n} files -> {payload}")

    # --- env for the crate's build.rs ---
    env = dict(os.environ)
    env.update(
        ITASHA_APP_NAME=product,
        ITASHA_APP_BIN=app_bin,
        ITASHA_VENDOR=vendor,
        ITASHA_INSTALL_SUBDIR=subdir,
        ITASHA_VERSION=version,
        ITASHA_PUBLISHER=vendor,
        ITASHA_HOMEPAGE=homepage,
        ITASHA_TAGLINE=tagline,
        ITASHA_KANJI=kanji,
        ITASHA_VOICE_HEX=voice,
        ITASHA_PAYLOAD_ZIP=str(payload.resolve()),
    )
    print(
        f"==> config: {product} {version} · {vendor}\\{subdir} · voice {voice} · kanji {kanji}"
    )

    if args.no_build:
        print("==> --no-build: config + payload staged; skipping cargo build.")
        return

    # --- build the installer crate (release) ---
    print("==> cargo build --release (installer crate)")
    r = subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(INSTALLER / "Cargo.toml")],
        env=env,
    )
    if r.returncode != 0:
        sys.exit(f"ERROR: cargo build failed ({r.returncode})")

    built = INSTALLER / "target" / "release" / "itasha-installer.exe"
    if not built.is_file():
        sys.exit(f"ERROR: expected installer not found: {built}")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(built, args.out)
    size_mb = args.out.stat().st_size / (1024 * 1024)
    print(f"==> installer: {args.out} ({size_mb:.1f} MiB)")


if __name__ == "__main__":
    main()
