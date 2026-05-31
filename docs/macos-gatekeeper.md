# macOS first-run prompt — why it appears and how to proceed

> Audience: end users opening a F0RG3-W1R3-packaged app on macOS for the first
> time. Maintainers: see `packaging/macos/sign-notarize-staple.sh` and
> `docs/adr/0003-signing-posture.md` for the signing/notarization pipeline.

When you open a freshly downloaded app, macOS may show a security prompt before
it launches. This is **Gatekeeper** — the built-in macOS check on apps from
outside the App Store. The prompt you see depends on whether the build was
**notarized** and on your macOS version.

## The short version

| Build state | What you see | What to do |
|---|---|---|
| Notarized + stapled (a signed public release) | Usually nothing, or a one-time "downloaded from the internet" confirmation | Click **Open** |
| Unsigned / un-notarized (a dev build) | A block dialog; on macOS Sequoia and later it sends you to System Settings | Follow **"Opening an unsigned build"** below |

A **signed public release is notarized and stapled**, so it opens cleanly and
works **offline** (the notarization ticket is attached to the disk image — no
internet round-trip is needed at launch). If you got the app from the official
release page, this is the normal path.

## What changed in recent macOS versions

macOS first-run handling got stricter, and our pipeline is built for it:

- **macOS Sequoia (15) removed the Control-click "Open Anyway" shortcut.**
  Previously you could right-click (Control-click) an app and choose **Open** to
  bypass the warning. That shortcut is **gone**. For an un-notarized app, macOS
  now sends you into **System Settings > Privacy & Security** on every first
  run. This is exactly why our public releases are **notarized and stapled** —
  a notarized build skips this trip entirely.
- **macOS Tahoe (26) wipes a disk image's custom icon during notarization.**
  Our build pipeline re-applies the branded disk-image icon **after** stapling
  (`sign-notarize-staple.sh --volicon ...`), so the disk image you download
  still looks right. This is cosmetic and does not affect security or launch.

## Opening an unsigned build (dev builds only)

If you are intentionally running an **unsigned developer build** (for example,
a build made before the project's signing identity was set up), macOS will not
let it open with a normal double-click. To open it:

1. Try to open the app once (double-click). macOS shows a block dialog.
2. Open **System Settings** > **Privacy & Security**.
3. Scroll to the **Security** section. You will see a line naming the app that
   was blocked, with an **Open Anyway** button.
4. Click **Open Anyway**, then authenticate (Touch ID or your password).
5. Open the app again — it now launches and is remembered for future runs.

> This path exists only for builds that were never notarized. We **never fake**
> notarization to make this prompt disappear — an unsigned build is honestly
> unsigned. For public distribution we notarize, so end users do not hit this.

## Verifying a download yourself (optional)

Every release artifact is also signed with a free, cross-platform **minisign**
key and carries a `.minisig` detached signature plus a `checksum.sha256`. You
can verify integrity without relying on Apple or GitHub:

```sh
# Verify the artifact against the project's public key (keys/minisign.pub):
minisign -Vm <downloaded-file> -p keys/minisign.pub
```

A successful verification means the file matches what the project published.

## For maintainers: why notarize + staple

- **Notarize**: Apple scans the signed app and issues a ticket. Required for any
  app distributed outside the App Store to open without the block dialog.
- **Staple**: attaches the ticket to the disk image so Gatekeeper passes
  **offline**. Without stapling, a user with no internet at launch can still be
  blocked.
- **Gating, not faking**: notarization needs an Apple Developer Program account
  ($99/yr). Until those credentials are present, the pipeline ships the build
  **unsigned dev-only** with this document as the user-facing explanation — it
  does not pretend to be notarized. See `docs/adr/0003-signing-posture.md`.

### The asserted (load-bearing) verification chain

Apple's guidance is explicit: **"notarization passing is not the same as
Gatekeeper passing"** — a build can be notarized yet still be rejected by
Gatekeeper if the ticket is not stapled or the runtime is not hardened. So when
signing is engaged (the Apple Developer ID credentials are present),
`sign-notarize-staple.sh` asserts BOTH halves of the chain and a failure
**hard-fails the build**:

1. `codesign --force --options runtime --timestamp` — sign the `.app` under the
   **Hardened Runtime** (a notarization prerequisite).
2. `xcrun notarytool submit … --wait` — submit and wait for the Apple ticket.
3. `xcrun stapler staple` — attach the ticket to the `.dmg` (offline Gatekeeper).
4. `xcrun stapler validate` — **load-bearing**: a missing/invalid ticket exits
   the script non-zero.
5. `spctl --assess --type open --context context:primary-signature` —
   **load-bearing**: a Gatekeeper rejection exits the script non-zero.

Steps 4 and 5 were previously suffixed `|| true`, which **swallowed** the
verdict — a tampered or un-notarized `.dmg` would print a rejection yet the
script still reported success. That swallow is removed: on the creds-present
path a failed `stapler validate` or `spctl --assess` now emits `::error::` and
fails the build. `tests/macos/verify.sh --expect-signed` mirrors this and FAILS
on a deliberately un-stapled or tampered `.app`/`.dmg`, proving the verdict is
no longer masked.

The credential-**absent** path is unchanged and honest: with no
`APPLE_SIGNING_IDENTITY`, the script skips signing, emits a loud `::warning::`,
ships the build unsigned (+ minisign + cosign-keyless), and **never fakes a
notarization ticket**. The `release-verify` staple assertion is then
**skipped-with-a-structured-reason**, not failed.

### Consequence of an unsigned / un-notarized public build

On macOS Sequoia (15) and later there is no Control-click "Open Anyway"
shortcut, so an un-notarized public build **hard-blocks** on first run and traps
the user in System Settings > Privacy & Security every launch. That is why a
public release MUST be notarized + stapled — the asserted chain above is the
guarantee that a build claiming to be release-ready actually passes Gatekeeper.
