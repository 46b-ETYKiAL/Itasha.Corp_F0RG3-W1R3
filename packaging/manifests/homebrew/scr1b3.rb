# Homebrew Cask (template) — Itasha.Corp SCR1B3
#
# A cask packages the notarized macOS .dmg. This is a TEMPLATE; the CI release
# job fills the placeholders from the actually-uploaded notarized artifact:
#   #{version}    — release version (tag minus leading v)
#   #{sha256}     — sha256 of the .dmg (CI computes from checksum.sha256;
#                   never hand-edited — WezTerm #7713 hash-drift avoidance)
#   #{url}        — GitHub Release download URL of the .dmg
#
# Until a notarized build exists, leave sha256 as :no_check is NOT used here —
# CI substitutes the real hash (or the Homebrew bump-cask Action recomputes it
# from the published asset; see .github/workflows/package-bump.yml). Installing
# an un-notarized dmg will hit Gatekeeper; see docs/adr/0003-signing-posture.md.
#
# Tap + placement: this cask is published to the Itasha.Corp Homebrew tap
#   `itasha-corp/homebrew-tap` — install with
#   `brew install --cask itasha-corp/tap/scr1b3`.
# `brew bump-cask-pr` (and the bump-cask Action) keep the version + sha256
# current; the hash is recomputed from the asset, never hand-edited.
cask "scr1b3" do
  version "__VERSION__"
  sha256 "__SHA256__"

  url "__URL__"
  name "SCR1B3"
  desc "Fast, cross-platform note-taking and scribe app"
  homepage "https://github.com/itasha-corp/scr1b3"

  # The `app` stanza installs SCR1B3.app into /Applications (Homebrew's
  # canonical cask appdir) and symlinks it; uninstall removes it cleanly.
  app "SCR1B3.app", target: "/Applications/SCR1B3.app"

  zap trash: [
    "~/Library/Application Support/SCR1B3",
    "~/Library/Caches/corp.itasha.scr1b3",
    "~/Library/Preferences/corp.itasha.scr1b3.plist",
  ]
end
