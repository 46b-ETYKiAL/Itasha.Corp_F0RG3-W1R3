# Linux packaging — AppImage (primary) + `.deb` (secondary) + Flathub (store)

cargo-packager produces two Linux artifacts from one config; a Flathub manifest
adds a third, store-managed distribution track:

| Format | Role | Why |
|--------|------|-----|
| **AppImage** | Primary (portable) | Single-file, runs on any modern distro, no root, no package manager. Delta-updates via zsync (see `appimage-update.md`). |
| **`.deb`** | Secondary | APT integration for Debian / Ubuntu fleets. |
| **Flatpak / Flathub** | Store discoverability | Flathub is the 2026 de-facto Linux desktop store. Manifest: `org.itashacorp.C0PL4ND.yaml` + `org.itashacorp.C0PL4ND.metainfo.xml`. Submission is an owner-authorized publish step. |

## Files

| File | Purpose |
|------|---------|
| `c0pl4nd.desktop` | Freedesktop `.desktop` entry — `Categories=System;Utility;TerminalEmulator;`, `Icon=c0pl4nd`, `StartupWMClass` for taskbar grouping. |
| `install.sh` | One-line user-scope installer for a downloaded AppImage: copies to `~/.local/bin`, registers the `.desktop` + 256px icon, verifies a `.sha256` sidecar if present, refreshes the desktop/icon caches. |
| `org.itashacorp.C0PL4ND.yaml` | flatpak-builder manifest for the Flathub store track (binary-as-artifact; submission is owner-gated). |
| `org.itashacorp.C0PL4ND.metainfo.xml` | AppStream metainfo required by Flathub (public product metadata only). |
| `appimage-update.md` | zsync + AppImageUpdate delta-update path for the portable AppImage (free, self-hosted). |

## Desktop-entry + icon integration

- The `.desktop` `Categories` list (`System;Utility;TerminalEmulator;`) places
  the app in the correct menu sections.
- `install.sh` rewrites `Exec=` to the resolved install path and drops the entry
  into `${XDG_DATA_HOME:-~/.local/share}/applications`.
- The 256px icon (`branding/<app>/icon-256.png`, authored in Phase 4) is copied
  into `~/.local/share/icons/hicolor/256x256/apps` so menus + the taskbar pick
  it up.

## `.deb` runtime dependencies

The per-app override declares the runtime `depends` line (cargo-packager
`[package.metadata.packager.deb]`). For C0PL4ND:

```toml
deb_depends = ["libc6", "libxkbcommon0", "libwayland-client0", "libfontconfig1"]
```

## Lint / verification (Phase 6, T6.2)

- **AppImage:** `appimagetool` validation on the produced AppImage.
- **`.deb`:** `lintian` on the produced package.
- **`.desktop`:** `desktop-file-validate` for freedesktop correctness.
- **`install.sh`:** `shellcheck`-clean (enforced in the framework CI).

These run on a clean container per `tests/linux/`.
