# Software Bill of Materials (SBOM)

Every release emits a **CycloneDX SBOM** describing the components inside the
installer bundle. The SBOM is attached to the GitHub Release next to the
installers, `checksum.sha256`, and the provenance attestation.

## What is emitted

| File | Format | Produced by |
|---|---|---|
| `sbom.cdx.json` | CycloneDX JSON | `anchore/sbom-action` (syft) over the bundled `dist/` artifact dir |
| `sbom.cdx.json.sha256` | SHA-256 digest | the release workflow, for integrity |

The SBOM is generated in `release.yml` by the **Generate CycloneDX SBOM** step,
which runs syft (via the pinned `anchore/sbom-action`) over the `dist/` bundle
**after** the installer is built and signed. It is then checksummed and attached
to the Release.

> Honest note: if the SBOM step produces no `dist/sbom.cdx.json` (e.g. an empty
> bundle on a dry-run runner), the workflow emits a LOUD `::warning::` and the
> release ships **without** an SBOM rather than attaching an empty or fabricated
> document.

## Why CycloneDX

- CycloneDX is an OWASP-stewarded, OASIS-standard SBOM format with broad 2026
  tool support (syft, Dependency-Track, Grype, cosign attach/attest).
- It is the format cosign and GitHub attestations both accept, so the SBOM can
  optionally be attested with the same keyless pipeline.
- It carries component identity (name, version, PURL), licenses, and hashes —
  enough for downstream vulnerability scanning and license audit.

## How to consume it

### Inspect components

```bash
# pretty-print the component list
jq '.components[] | {name, version, type, purl}' sbom.cdx.json
```

### Verify integrity

```bash
sha256sum -c sbom.cdx.json.sha256     # Linux
shasum -a 256 -c sbom.cdx.json.sha256 # macOS
```

### Scan for known vulnerabilities

Feed the SBOM to any CycloneDX-aware scanner, e.g. Grype:

```bash
grype sbom:sbom.cdx.json
```

or upload it to a Dependency-Track instance for continuous monitoring.

### Validate the schema

```bash
# requires the cyclonedx CLI (github.com/CycloneDX/cyclonedx-cli)
cyclonedx validate --input-file sbom.cdx.json --input-format json
```

## Relationship to other release evidence

| Evidence | Answers |
|---|---|
| `checksum.sha256` / minisign | "is this the exact file that was released?" |
| `gh attestation verify` (provenance) | "was this built from this source, in this workflow?" |
| `cosign verify-blob` | "was this signed by a workflow in this org?" |
| **`sbom.cdx.json` (this doc)** | "**what is inside it, and what are its dependencies + licenses?**" |

See `docs/verify-cosign.md` for the provenance + signature checks.

## References

- best-in-class-installer-2026-05-27.md §SBOM (P1-4).
- CycloneDX specification (OWASP / OASIS).
- anchore syft / `anchore/sbom-action`; Grype; CycloneDX CLI.
