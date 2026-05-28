# AppImage delta updates — zsync + AppImageUpdate (free, self-hosted)

The AppImage is the framework's portable Linux primary. This document wires the
**free, self-hosted delta-update path** for it: a `*.zsync` companion file plus
[AppImageUpdate](https://github.com/AppImageCommunity/AppImageUpdate). It pairs
with `cargo-packager-updater` (used for the NSIS / DMG tracks) so every platform
has a signed update path with no paid service.

> An AppImage update with zsync downloads **only the changed blocks** between the
> installed image and the new one — not the whole file — over plain HTTPS. No
> server-side software beyond a static file host is required.

## How zsync delta-update works

1. The build emits the AppImage **and** a `<App>-<version>-x86_64.AppImage.zsync`
   companion (the zsync control file: block checksums + the target URL).
2. The AppImage embeds an **update-information** string (the `zsync|<url>` form)
   in its ELF header so `AppImageUpdate` knows where to look.
3. On update, `AppImageUpdate` reads the embedded URL, fetches the `.zsync`
   control file, and pulls only the changed blocks from the published AppImage.

## Build-time emission (framework-side)

The Linux build step emits the companion alongside the AppImage. `zsyncmake` is
the canonical generator (from the `zsync` package, free/OSS):

```sh
# After cargo-packager produces the AppImage:
APPIMAGE="C0PL4ND-${VERSION}-x86_64.AppImage"
ZSYNC_URL="https://github.com/itasha-corp/c0pl4nd/releases/latest/download/${APPIMAGE}.zsync"

# Generate the .zsync control file with the canonical download URL.
zsyncmake -u "${ZSYNC_URL}" -o "${APPIMAGE}.zsync" "${APPIMAGE}"
```

## Embedded update-information

The update-information string is set on the AppImage so AppImageUpdate is
self-describing. The canonical zsync transport form for a GitHub release is:

```
zsync|https://github.com/itasha-corp/c0pl4nd/releases/latest/download/C0PL4ND-x86_64.AppImage.zsync
```

(`appimagetool` writes this into the AppImage's `.upd_info` section via its
`-u` / update-information argument; the zsync companion is what it points at.)

## End-user update flow

```sh
# One-time: install AppImageUpdate (itself an AppImage).
# Then, to update an installed C0PL4ND AppImage in place:
AppImageUpdate ~/.local/bin/c0pl4nd
```

AppImageUpdate verifies block checksums from the `.zsync` control file as it
assembles the new image; the `checksum.sha256` sidecar published with the
release is the additional whole-file integrity check (verified by `install.sh`).

## Why zsync (not Snap/Flatpak deltas) for the portable track

| Path | Delta updates | Server requirement | Cost |
|------|---------------|--------------------|------|
| **zsync + AppImageUpdate** | Yes (block-level) | Static file host only | Free / self-hosted |
| Flatpak (Flathub) | Yes (OSTree) | Flathub infra | Free, but store-managed (see the Flatpak manifest track) |
| Snap | Yes | Snap Store | Store-managed |

The portable AppImage track stays fully self-hosted with zsync; the Flathub
manifest (`org.itashacorp.C0PL4ND.yaml`) is the complementary store-managed
track for discoverability.

## Honest-skip / verification status

- `zsyncmake` and `appimagetool` are **not installed in this development
  environment**, so a real `.zsync` companion is **not** generated here — the
  commands above are the exact, documented build-step invocations that run on a
  Linux build runner (where the toolchain is present). Nothing is faked: no
  placeholder `.zsync` artifact is committed.
- The Flatpak manifest's YAML is schema-validated by the framework
  (`python -c "import yaml; yaml.safe_load(...)"`); a real `flatpak-builder`
  build requires `flatpak` + `flatpak-builder` + the freedesktop runtime, which
  are likewise absent in this environment — that build is a Linux-side step,
  documented here as an honest-skip rather than claimed.
