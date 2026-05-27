# ADR-0002 — Reserved Enterprise WiX/MSI Track

- **Status:** Accepted
- **Date:** 2026-05-26
- **Deciders:** Itasha.Corp installer framework

## Context

NSIS (via cargo-packager, ADR-0001) is the **primary** Windows installer for
its branded first-install UX. But enterprises deploy via Group Policy / Intune,
which want an **MSI**. A working MSI build already exists and must be preserved,
not silently dropped (decision D2: *recommend, do not drop*).

## Decision

Keep an MSI track as a **reserved, documented, parallel** path at
`packaging/windows/wix/` (WiX v6 + Burn). NSIS is recommended for the public
download; MSI is the enterprise/GPO reserve.

### The WiX version-match lesson (load-bearing)

`WixUIExtension` **MUST match the `wix.exe` toolset major version**. A v3
`WixUIExtension` against a v4/v6 toolset (or vice-versa) fails the build.

**GitHub `windows-latest` ships WiX v3.14.1 by default.** Therefore any v4/v6
build MUST install the matching WiX toolset + UI extension as an **explicit,
pinned CI step** — never rely on the runner's default WiX.

The WiX toolset version is pinned in `packaging/windows/wix/build-wix.ps1` and
must be kept in sync with the `WixUIExtension` NuGet/extension version used.

## Consequences

- The existing working MSI is preserved as backward-compatible (D2).
- Enterprise/Intune/GPO deployment is supported via a Group-Policy-compatible
  MSI without burdening the primary NSIS path with WiX XML.
- CI that builds the MSI must pin WiX explicitly (the v3.14.1-default callout).

## References

best-in-class-installer research §1 (windows-latest WiX v3.14.1 default; v4/v6
explicit pinned install; UI-ext-version-match).
