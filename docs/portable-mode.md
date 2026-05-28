# Portable mode (no-install ZIP + config-next-to-binary)

> Audience: users who cannot or prefer not to run an installer (locked-down
> corporate machines, USB-stick / network-share usage, "I don't install
> things"). Maintainers: the portable lane is the `"zip"` entry in an app's
> `formats` (see `apps/c0pl4nd.toml`).

Alongside every branded installer (Windows NSIS, macOS `.dmg`, Linux
`.deb`/AppImage), F0RG3-W1R3 can emit a **portable ZIP**: extract anywhere,
run the binary directly, leave nothing behind. There is no system-wide install,
no elevation, no `Program Files` write, no registry entry.

## When to use portable mode

| Situation | Portable ZIP fits because… |
|---|---|
| Locked-down / managed workstation (no admin) | No elevation, no per-machine write |
| USB stick / network share | Self-contained; runs from where it sits |
| Try-before-install | Extract, run, delete the folder — zero residue |
| CI / scripted environments | No installer UI; just unzip and execute |

For a normal personal machine, prefer the installer — it adds the Start-Menu
entry, PATH integration, and clean uninstall. Portable mode trades those for
zero-footprint.

## The `%APPDATA%` trap (why config-next-to-binary)

A common portability failure (Alacritty issue #3838 is the canonical example):
the app advertises a "portable" ZIP but still writes its config to
`%APPDATA%` (Windows) / `~/.config` (Linux) / `~/Library` (macOS). The moment
you move the binary to another machine, your settings do not come with it — so
it is not actually portable.

F0RG3-W1R3's portable contract fixes this: **a portable build keeps its config
next to the binary, never in the per-user app-data directory.**

### How an app detects portable mode

The installer cannot inject runtime behaviour into a compiled binary, so the
**consuming app** honours the contract via a sentinel:

- The portable ZIP ships a marker file **next to the binary** — a zero-byte
  `portable.marker` (or an empty `config/` directory beside the executable).
- On startup the app checks for this marker. If present, it resolves **all**
  config, cache, and state paths **relative to the executable's own directory**
  instead of the OS per-user app-data location.
- If the marker is absent (a normal installed build), the app uses the standard
  per-user paths as usual.

This keeps a single binary capable of both modes — installed builds behave
normally; the portable ZIP is genuinely self-contained.

### Layout of a portable ZIP

```
<app>-portable/
├── <app>(.exe)          # the binary
├── portable.marker      # presence => config lives in this folder
├── config/              # settings written here, not in %APPDATA%
│   └── (created on first run)
├── README.txt           # one-line "run the binary; settings stay in this folder"
└── <app>.minisig        # detached minisign signature for integrity verification
```

## Building the portable ZIP

The portable lane is the `"zip"` format in an app's override:

```toml
# apps/<app>.toml
formats = ["nsis", "dmg", "appimage", "deb", "zip"]
```

The build wrapper produces the ZIP alongside the installers. The portable
**behaviour** (the `portable.marker` check) is implemented in the consuming app
— the framework packages the binary, marker, and folder layout; the app reads
the marker. No signing/notarization gate blocks the portable ZIP itself, but it
is still covered by the same minisign `.minisig` detached signature as every
other artifact, so users can verify integrity:

```sh
minisign -Vm <app>-portable.zip -p keys/minisign.pub
```

## Auto-update and portable mode

Installed builds get **silent, signature-verified, on-restart updates** via the
self-hosted `cargo-packager-updater` feed (configured in
`packager.template.toml` under `[package.metadata.packager.updater]`): the app
downloads the next version in the background, verifies its **minisign**
signature against the embedded public key, and swaps it in on the next launch —
no blocking modal, no forced restart, and no UAC prompt. A non-blocking
"what's new" note may be shown; the user is never interrupted.

**Portable builds do not auto-update.** A portable ZIP is intentionally
self-contained and may live on read-only media or a share, so it does not phone
home or modify itself. To update a portable copy, download the new portable ZIP
and replace the folder (your `config/` carries over because it sits next to the
binary). This is the deliberate trade-off: zero-footprint and no background
network activity, in exchange for manual updates.
