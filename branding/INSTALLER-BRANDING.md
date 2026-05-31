# Installer Branding Standard — F0RG3-W1R3

The reusable spec for the **per-app installer splash** every Itasha.Corp app
ships through F0RG3-W1R3. Each app gets its own branded wizard art that shares
one house style but is *customized to fit the contents of its repo*. Follow this
when onboarding a new app so the installers stay a consistent, recognizable
family.

## What the user sees

Every installer's welcome/finish page carries a **portrait sidebar** with the
three-line wordmark:

```
<app>          ← the app's readable name, lowercase (e.g. "scribe", "copland")
by
Itasha.Corp
```

plus an app **mark** (top) and an app **motif** (bottom) that say *what the app
is at a glance*, and a one-line **tagline**. Every page after the welcome page
carries a compact **header** strip with the same wordmark + mark.

## House style — "wired-noir / NERV title-card"

Shared by every app (this is the family resemblance). Derived from
`brand.yaml` (DECISION-2026-005 wired-noir) and the F0RG3-W1R3 branding README.

| Element | Spec |
|---|---|
| Plate | VOID BLACK `#08060d` (subtle vertical gradient `#0a0810`→`#070509`) |
| Primary accent | SIGNAL TEAL `#00e5ff` — the wordmark + mark, with a phosphor glow (`feGaussianBlur`) |
| Frame accent | OPERATOR VIOLET `#a020ff` — thin bezel + secondary ring, low opacity |
| Alarm accent | NEON PINK `#e020ff` — **alarm-only, essentially unused** on installers |
| Text | near-white `#f0eef5` for "Itasha.Corp" + "by" |
| Texture | CRT **scanlines** (`#f0eef5` @ ~3.5% on a 3px pattern) + corner-darkening **vignette** |
| Chrome | thin violet bezel inset + four **NERV corner registration ticks** (teal L-marks) |

### The wordmark font (the "NERV / Evangelion" look)

Per `brand.yaml` typography: the wordmark is set in **horizontally-squeezed
Times New Roman** — the brand's reliably-available stand-in for the Matisse EB
Mincho that Eva title cards use. There is **no commercial font to license**: it
is plain Times New Roman compressed on the X axis.

- Font stack (Linux-safe): `'Times New Roman','Liberation Serif',serif`
  (fontconfig maps Times New Roman → Liberation Serif on CI runners — same
  metrics, so the squeeze is identical).
- Squeeze: wrap the text in `transform="scale(0.66–0.74,1)"` around its anchor.
  Use a tighter squeeze for longer names so they still fit the 164px width
  (`scribe` 0.72, `copland` 0.68).
- "by" is small letter-spaced **sans** (`Arial Narrow`) — it is connective
  tissue, not a wordmark, so it stays out of the serif voice.

## The two assets

Both are committed as **SVG sources** under `branding/<app>/`; `gen-assets.sh
--app <app>` rasterizes them to the BMP3 files NSIS consumes (24-bit, no alpha —
each PNG is flattened onto the void-black plate).

| Source (committed) | Output (git-ignored) | Size | NSIS slot |
|---|---|---|---|
| `branding/<app>/nsis-sidebar.svg` | `branding/nsis-sidebar.bmp` | **164×314** | `MUI_WELCOMEFINISHPAGE_BITMAP` (`sidebar_image`) |
| `branding/<app>/nsis-header.svg` | `branding/nsis-header.bmp` | **150×57** | `MUI_HEADERIMAGE` (`header_image`) |

The `packager.template.toml` references the fixed BMP paths; `gen-assets.sh`
regenerates them **per app** at build time, so no template change is needed to
add an app — only the two per-app SVGs.

### Sidebar layout (164×314)

```
 ┌──────────────┐
 │   ‹mark›      │  app mark in a teal+violet ring, ~52px, y≈64
 │              │
 │   <app>      │  wordmark, squeezed Times, ~44–46px teal, baseline y≈150
 │     by       │  Arial Narrow, 11px, letter-spacing 5, y≈178
 │ Itasha.Corp  │  squeezed Times, 22px near-white, baseline y≈206
 │              │
 │   ‹motif›    │  app-specific, y≈250
 │  <tagline>   │  Arial Narrow, ~6px, letter-spacing 1.2, teal 72%, y≈291
 └──────────────┘   (scanlines + vignette + bezel + corner ticks over all)
```

### Header layout (150×57)

Wordmark (squeezed Times, ~26px teal, left) + app mark in a ring (~30px, right)
+ a teal baseline rule with a faint violet under-rule. Keep it to the wordmark +
mark only — it repeats on every page, so it must stay quiet.

## What you customize per app (fit the repo's contents)

The house style is fixed; **three things change per app** so the splash matches
what the app actually is:

1. **Mark** — a single glyph in the ring that is the app's identity.
   Reuse the geometry of the app's existing `branding/<app>/icon.svg` mark.
   - `scribe` (editor): **caret-in-ring** (vertical caret bar + baseline).
   - `copland` (terminal): **prompt chevron `>` + cursor block**.
2. **Motif** (bottom of sidebar) — a tiny scene of the app in use.
   - `scribe`: a **code-buffer rule stack** with a live teal caret.
   - `copland`: a **command line** — `>` prompt + command rule + block cursor.
3. **Tagline** — the app's one-liner, ALL CAPS, ≤ ~26 chars so it fits 164px.
   - `scribe`: `PRESENT DAY, PRESENT TEXT`.
   - `copland`: `GPU-ACCELERATED TERMINAL`.

The wordmark text is the app's **readable lowercase name** ("scribe",
"copland") — not the leetspeak product id (SCR1B3, C0PL4ND), which stays the
install/binary identifier. Installers show the human-readable name.

## librsvg / CI safety

The BMPs are generated on Linux CI with `rsvg-convert` (librsvg). Keep the SVGs
within librsvg's support:

- Use `feGaussianBlur` + `feMerge` for the phosphor glow (supported). Avoid
  exotic filters.
- Fonts must resolve via fontconfig — stick to the Times New Roman / Liberation
  Serif and Arial Narrow / Arial / sans-serif stacks above.
- No external images, no scripts, no CSS `@import`. Self-contained vector only.

## Visual QA (required before commit)

`gen-assets.sh` never fakes an asset, but it can't tell you the splash *looks*
right. Before committing a new/changed splash, render and eyeball it **at native
size** (legibility) and zoomed (detail):

1. Serve `branding/` over HTTP and open it in a browser
   (`python -m http.server`), or rasterize with `rsvg-convert -w 164 -h 314`.
2. Check, at native 164×314: the wordmark is legible, the tagline is **not
   clipped** at the 164px width, the mark/motif read as the app, and the teal
   glow + scanlines + corner ticks are present and balanced.
3. Confirm the sidebar reads against the **white** NSIS wizard body to its right.

## Checklist — onboarding a new app

- [ ] `branding/<app>/icon.svg` exists (defines the mark geometry).
- [ ] `branding/<app>/nsis-sidebar.svg` (164×314) — house style + app mark,
      wordmark `<app>` / by / Itasha.Corp, app motif, app tagline.
- [ ] `branding/<app>/nsis-header.svg` (150×57) — wordmark + mark + baseline rule.
- [ ] Tagline ≤ ~26 chars; wordmark squeeze chosen so it fits 164px.
- [ ] `gen-assets.sh --app <app>` produces both BMP3s with a rasterizer present.
- [ ] Visual QA passed at native + zoom (no clipping; reads as the app).
- [ ] `apps/<app>.toml` points `icon_ico/png/icns` at `branding/<app>/…`.

## Related

- `branding/README.md` — asset inventory + `gen-assets.sh` usage.
- `packager.template.toml` — `[nsis] header_image` / `sidebar_image` wiring.
- `brand.yaml` (monorepo) — DECISION-2026-005 wired-noir palette + the NERV
  squeezed-serif typography decision this standard implements.
