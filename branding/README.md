# Branding Assets — F0RG3-W1R3

Brand identity for the **F0RG3-W1R3** installer framework and the per-app
installers it produces. Brand language: **Retro-Future Anime OS** — VOID BLACK
`#08060d` / SIGNAL TEAL `#00e5ff` / NEON PINK `#e020ff` / OPERATOR VIOLET
`#a020ff`, CRT bezel + scanline motif (shared with the C0PL4ND terminal and the
wider Itasha.Corp surface).

## Committed SVG sources

| File | Purpose |
|------|---------|
| `banner.svg` | Repo brand banner (widescreen CRT bezel). Mirror of `.github/assets/header.svg`. |
| `footer.svg` | Repo brand footer (CRT strip). Mirror of `.github/assets/footer.svg`. |
| `dmg-background.svg` | macOS disk-image background source (drag-to-Applications hint). |
| `<app>/icon.svg` | Per-app app-icon source (e.g. `c0pl4nd/icon.svg`). |

The canonical README banner/footer live at `.github/assets/{header,footer}.svg`;
the `branding/` copies exist so the engine config's `branding/banner.svg` /
`branding/footer.svg` references resolve.

## Generated raster outputs (git-ignored)

`gen-assets.sh` regenerates these from the SVG sources on demand — only the SVG
sources are committed:

| Output | Spec | Consumed by |
|--------|------|-------------|
| `<app>/icon-256.png` | 256×256 PNG | Linux hicolor icon, generic use |
| `<app>/icon.ico` | multi-res (16–256) | NSIS `installer_icon`, Windows ARP |
| `<app>/icon.icns` | multi-res Apple iconset | macOS `.app` bundle icon |
| `nsis-header.bmp` | 150×57 BMP3 | NSIS wizard header (`header_image`) |
| `nsis-sidebar.bmp` | 164×314 BMP3 | NSIS wizard sidebar (`sidebar_image`) |
| `dmg-background.png` | 660×400 PNG | macOS dmg background (matches the dmg window size) |

```sh
./gen-assets.sh --app c0pl4nd
```

`gen-assets.sh` uses free OSS tools (librsvg `rsvg-convert` or ImageMagick
`convert` for rasterization; `iconutil`/`png2icns` for `.icns`). If a tool is
missing it prints the install command and **skips that output honestly** — it
never writes a corrupt or placeholder asset.

## How the engine config references these

- `packager.template.toml` → `header_image = "branding/nsis-header.bmp"`,
  `sidebar_image = "branding/nsis-sidebar.bmp"`,
  `[dmg].background = "branding/dmg-background.png"`.
- `apps/c0pl4nd.toml` → `icon_ico = "branding/c0pl4nd/icon.ico"`,
  `icon_png = "branding/c0pl4nd/icon-256.png"`,
  `icon_icns = "branding/c0pl4nd/icon.icns"`.

## Adding a new app's brand

1. Add `branding/<app>/icon.svg` (256×256 source).
2. Run `./gen-assets.sh --app <app>` to produce the per-OS icons + wizard art.
3. Point the per-app override's `icon_*` keys at the generated files.
