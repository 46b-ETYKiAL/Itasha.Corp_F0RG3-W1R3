# Homebrew Cask (template) — Itasha.Corp C0PL4ND
#
# A cask packages the notarized macOS .dmg. This is a TEMPLATE; the CI release
# job fills the placeholders from the actually-uploaded notarized artifact:
#   #{version}    — release version (tag minus leading v)
#   #{sha256}     — sha256 of the .dmg (CI computes from checksum.sha256;
#                   never hand-edited — WezTerm #7713 hash-drift avoidance)
#   #{url}        — GitHub Release download URL of the .dmg
#
# Until a notarized build exists, leave sha256 as :no_check is NOT used here —
# CI substitutes the real hash. Installing an un-notarized dmg will hit
# Gatekeeper; see docs/adr/0003-signing-posture.md.
cask "c0pl4nd" do
  version "__VERSION__"
  sha256 "__SHA256__"

  url "__URL__"
  name "C0PL4ND"
  desc "Fast, cross-platform terminal emulator"
  homepage "https://github.com/itasha-corp/c0pl4nd"

  app "C0PL4ND.app"

  zap trash: [
    "~/Library/Application Support/C0PL4ND",
    "~/Library/Caches/corp.itasha.c0pl4nd",
    "~/Library/Preferences/corp.itasha.c0pl4nd.plist",
  ]
end
