# macOS packaging — branded `.dmg` + sign / notarize / staple

The macOS target produces a **branded disk image** (`.dmg`) with a custom
background and a drag-to-Applications layout, then signs the `.app`, notarizes
the `.dmg`, and staples the ticket.

## What cargo-packager produces

cargo-packager generates the `.app` bundle and the `.dmg` from the shared
template's `[package.metadata.packager.dmg]` block:

```toml
[package.metadata.packager.dmg]
background = "branding/dmg-background.png"          # brand background image
window_size = { width = 660, height = 400 }          # disk-image window size
app_position = { x = 180, y = 200 }                  # the .app icon position
application_folder_position = { x = 480, y = 200 }   # the Applications symlink
```

This yields the canonical drag-to-Applications experience: the `.app` on the
left, an `/Applications` alias on the right, the brand background behind them.
`branding/dmg-background.png` is authored in Phase 4 (`branding/README.md`).

## Signing pipeline (`sign-notarize-staple.sh`)

After cargo-packager builds the `.dmg`, the signing pipeline runs:

1. `codesign --options runtime --timestamp` the `.app` (hardened runtime).
2. `xcrun notarytool submit --wait` the `.dmg` to Apple.
3. `xcrun stapler staple` the ticket onto the `.dmg`.
4. `xcrun stapler validate` + `spctl --assess` to verify.

Stapling is chosen specifically so **Gatekeeper passes offline** — a stapled
ticket does not require a network round-trip at first launch.

## The Apple Developer dependency (gated, never faked)

macOS notarization is a **real, non-deferrable external dependency**. For any
public / non-self-built distribution, Gatekeeper hard-blocks an unsigned app.
The only path is an **Apple Developer Program account ($99/yr)** for a
Developer ID. There is no free or self-hosted alternative.

Until that account exists:

- The build ships an **unsigned dev artifact**.
- `sign-notarize-staple.sh` detects the absent `APPLE_SIGNING_IDENTITY`,
  prints a documented dev-unsigned warning, and **exits 0 without faking**
  notarization.
- On the building Mac, the app runs after a right-click → Open.

All credentials are read **by name** from the environment / CI secrets:

| Env var | Purpose |
|---------|---------|
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Itasha.Corp (TEAMID)` |
| `APPLE_ID` / `APPLE_TEAM_ID` / `APPLE_APP_PASSWORD` | Apple-ID notarization path |
| `APPLE_API_KEY_ID` / `APPLE_API_ISSUER` / `APPLE_API_KEY_PATH` | App Store Connect API-key path (alternative) |

No literal key, password, or certificate ever appears in this repository (see
`ships-publicly-vs-never.md`). The full posture is recorded in
`docs/adr/0003-signing-posture.md`.
