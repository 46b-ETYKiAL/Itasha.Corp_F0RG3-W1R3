# Banner / header image spec

Canonical aspect ratio for all Itasha.Corp repo banners: **21:9** (ultrawide).

| Field | Value |
|-------|-------|
| Aspect ratio | **21:9** (≈ 2.33:1) |
| Canvas | `1280 × 549` (integer height for 21:9 at 1280 width) |
| File | [`.github/assets/header.svg`](assets/header.svg) |
| Motif | CRT-bezel terminal — scanlines, vignette, phosphor glow, perspective grid |
| Right-side imagery | Repo-unique: must represent the repo's purpose (F0RG3-W1R3 = the `BIN → nsis/dmg/appimage/deb` installer-forge pipeline + sign/verify column) |
| Animation | Pure SVG/CSS; honours `prefers-reduced-motion` |

When regenerating the banner, keep the `viewBox="0 0 1280 549"` (21:9). Do **not** ship 16:9 (1280×720) or the legacy 2.56:1 (1280×500) — 21:9 is the standard across every Itasha.Corp repo banner.
