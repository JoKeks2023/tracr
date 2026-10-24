# Releasing tracr

A release is a git tag. The `release` workflow (`.github/workflows/release.yml`) builds on a GitHub macOS (arm64) runner, so no heavy build runs on a developer machine.

## What a tag produces

| File | Contents |
|---|---|
| `tracr-<version>-aarch64-apple-darwin.tar.gz` | the `tracr` command line tool |
| `tracr-<version>-aarch64-apple-darwin.zip` | the desktop app (`tracr.app`) |
| `SHA256SUMS` | checksums of both |

The GitHub Release gets generated notes (grouped by label, see `.github/release.yml`) and is marked **pre-release** while the major version is 0. The workflow then renders `Formula/tracr-cli.rb` and `Casks/tracr.rb` from `.github/brew/` with the new URL and checksums and commits them to `main`, so the repository is its own Homebrew tap.

```sh
brew tap JoKeks2023/tracr https://github.com/JoKeks2023/tracr
brew install --cask tracr      # desktop app
brew install tracr-cli         # command line tool
```

## Cutting a release

1. Set the version (workspace `Cargo.toml`, the crates' path dependencies, `apps/desktop/package.json`, `apps/desktop/src-tauri/tauri.conf.json`), run the checks, commit.
2. `git tag v<version> && git push origin v<version>`. The tag must equal the crate version; the workflow refuses otherwise.
3. Check the release page and `brew install` on a clean machine.

"Run workflow" (Actions tab) builds the same files without publishing a release or touching the formula. Use it to test changes to the pipeline.

## Signing and notarization

Without secrets the app is built **unsigned**; macOS then asks for confirmation on first launch, and the release notes say so. With the repository secrets below, the app is signed with a Developer ID and notarized, and the workflow verifies the result (`codesign`, `stapler`, `spctl`).

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | the Developer ID Application certificate as a `.p12`, base64-encoded (`base64 -i cert.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | the password set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` | the Apple ID used for notarization |
| `APPLE_PASSWORD` | an app-specific password for that Apple ID |
| `APPLE_TEAM_ID` | the 10-character team ID |

Only the first three are needed for signing; all six for notarization.

## Not covered yet

Windows packaging, Intel Macs, and a dedicated tap repository (the main repository serves as the tap). The license is still undecided (`UNLICENSED`), which should be settled before binaries are widely shared.
