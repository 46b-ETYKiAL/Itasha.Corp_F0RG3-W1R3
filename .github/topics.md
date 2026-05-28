# Suggested GitHub repository topics

Topics improve discoverability of the public installer-framework repo. Set them
under the repo's **About** panel (gear icon) or via the CLI:

```sh
gh repo edit <owner>/<repo> \
  --add-topic installer \
  --add-topic packaging \
  --add-topic cross-platform \
  --add-topic rust \
  --add-topic cargo-packager \
  --add-topic nsis \
  --add-topic dmg \
  --add-topic appimage \
  --add-topic flatpak \
  --add-topic code-signing \
  --add-topic notarization \
  --add-topic windows \
  --add-topic macos \
  --add-topic linux \
  --add-topic ci-cd \
  --add-topic supply-chain-security \
  --add-topic itasha-corp
```

## Topic list

| Topic | Why |
|-------|-----|
| `installer` | Core purpose — branded cross-platform installers. |
| `packaging` | Config-driven packaging engine. |
| `cross-platform` | One config → Windows / macOS / Linux. |
| `rust` | Rust-native engine (cargo-packager) and Rust app consumers. |
| `cargo-packager` | The packaging engine the framework wraps. |
| `nsis` | Windows branded-installer technology. |
| `dmg` | macOS disk-image format. |
| `appimage` | Linux portable format (primary). |
| `flatpak` | Linux store track (Flathub manifest). |
| `code-signing` | OV / Developer ID signing posture. |
| `notarization` | macOS Gatekeeper notarization. |
| `windows` / `macos` / `linux` | Target platforms. |
| `ci-cd` | Tag-gated release workflows. |
| `supply-chain-security` | Attestations, SBOM, checksum, SHA-pinned actions. |
| `itasha-corp` | Owning organization. |

## Notes

- GitHub allows up to 20 topics; the list above is within that limit.
- Keep topics lowercase and hyphenated (GitHub normalizes them anyway).
- Do **not** add topics that reference internal systems or internal repo names —
  only the public, product-relevant terms above.
