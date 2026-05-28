# Reproducible Builds

A reproducible build lets anyone re-run the build from the same source and get
the **same bytes**, proving the published artifact was not tampered with between
source and release. This page documents the method the framework uses and how it
promotes the `determinism_reproducibility` posture from **mitigated** to
**verified**.

## The signed-vs-unsigned distinction (read this first)

A **signed** installer is *not* bit-for-bit reproducible: a code signature
embeds a timestamp and a nonce, so two signing runs over identical input produce
different bytes by design. That is expected and is not a reproducibility defect.

What *is* reproducible — and what we verify — is the **UNSIGNED payload**: the
bundled binary + resources *before* any signing step runs. If the unsigned
payload is byte-identical across runs, then the only difference in the final
artifact is the (legitimately non-deterministic) signature.

## The method

`scripts/reproducible-build.sh` implements the
[reproducible-builds.org](https://reproducible-builds.org) convention:

1. **Pin the toolchain.** The Rust toolchain channel is pinned (`--toolchain`,
   default `stable`; a `rust-toolchain.toml` pins it further per-app). A fixed
   compiler removes the largest source of drift.
2. **Set `SOURCE_DATE_EPOCH`.** Exported to the build so every embedded
   timestamp/mtime is deterministic. The script derives it from the HEAD commit
   author time (`git log -1 --pretty=%ct`) so it is stable and meaningful;
   `release.yml` does the same at release time. `LC_ALL=C` and `TZ=UTC` are also
   pinned to remove locale/timezone-sensitive output.
3. **Build the unsigned payload twice** into two clean temp directories.
4. **Hash deterministically.** A *tree hash* is computed: each file's SHA-256 in
   sorted-path order, then a SHA-256 of that list. Signature/checksum/SBOM files
   (`*.sig`, `*.pem`, `*.minisig`, `checksum.sha256`, `sbom.cdx.json*`) are
   **excluded** so only the unsigned payload is measured.
5. **Compare.** Identical hashes → reproducible (exit 0). Differing hashes →
   not-yet-deterministic (exit 1) with guidance on the likely non-deterministic
   inputs (embedded timestamps, absolute paths, unsorted archive entries,
   build-id, env leakage).

## Running it

```bash
cd apps/itasha-installer
./scripts/reproducible-build.sh --app c0pl4nd
# optional: pin a specific toolchain channel
./scripts/reproducible-build.sh --app c0pl4nd --toolchain 1.83.0
```

### Honest-skip when the toolchain is absent

If `cargo` or `cargo-packager` is not installed, the script prints a clear
`SKIP:` message with the exact install command and **exits 0 without claiming a
result**. It never fakes a match. Install the toolchain
(<https://rustup.rs>, then `cargo install cargo-packager --locked`) to actually
measure reproducibility.

## From mitigated → verified

The `determinism_reproducibility` overlooked-considerations axis was previously
**mitigated** (the inputs were pinned, but reproducibility was asserted rather
than measured). This script + the `SOURCE_DATE_EPOCH` pin wired into `release.yml`
make it **verified**: two builds that produce an identical unsigned-payload hash
are objective evidence, not a claim. The promotion holds only when the script
actually runs (a tool-absent honest-skip does **not** promote the axis — it
reports that reproducibility was not measured on that host).

## CI integration

`release.yml` exports `SOURCE_DATE_EPOCH` (pinned from the tagged commit) before
the build so the released artifact is produced under the same deterministic
conditions this script verifies locally.

## References

- best-in-class-installer-2026-05-27.md §reproducible (P1-6).
- reproducible-builds.org — `SOURCE_DATE_EPOCH`, deterministic archives.
- ADR-0003 — signing posture (why the signed bytes legitimately differ).
- `docs/sbom.md`, `docs/verify-cosign.md` — the complementary supply-chain evidence.
