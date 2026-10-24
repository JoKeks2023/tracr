# Releasing tracr

Releases are built and published by hand, on the developer's Apple Silicon Mac, with one script: `scripts/release.sh`. There is no release CI (the `ci` workflow only checks code).

## What a release contains

| File | Contents |
|---|---|
| `tracr-<version>-aarch64-apple-darwin.tar.gz` | the `tracr` command line tool |
| `tracr-<version>-aarch64-apple-darwin.zip` | the desktop app (`tracr.app`) |
| `SHA256SUMS` | checksums of both |

The GitHub Release gets generated notes (grouped by label, see `.github/release.yml`) and is marked **pre-release** while the major version is 0. The repository is its own Homebrew tap: the script renders `Formula/tracr-cli.rb` and `Casks/tracr.rb` from `.github/brew/` with the new URL and checksums and commits them to `main`.

```sh
brew tap JoKeks2023/tracr https://github.com/JoKeks2023/tracr
brew install --cask tracr      # desktop app
brew install tracr-cli         # command line tool
```

## Cutting a release

1. **Set the version** in the workspace `Cargo.toml` and the path dependencies of the crates, `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`. Run the checks (`cargo fmt --check`, clippy, tests, `pnpm typecheck`) and commit and push to `main`.
2. **Build without publishing:** `scripts/release.sh 0.9.0`. This checks the machine (Apple Silicon, at least 8 GB free, version matches the crates), builds the CLI and the app in release mode (`CARGO_BUILD_JOBS=2` by default, so the machine stays usable), packages both into `dist/`, runs the CLI once, verifies the signature and writes `SHA256SUMS`.
3. **Try the app** from `dist/`.
4. **Publish:** `scripts/release.sh 0.9.0 --publish`. It requires a clean `main` equal to `origin/main`, creates and pushes the tag `v0.9.0`, creates the GitHub Release with the files, then updates and pushes the formula and cask. It refuses if the tag already exists.
5. On a clean machine: `brew tap …` and `brew install --cask tracr`.

A release build adds several GB under `target/release`; clean it afterwards if disk space is tight (`cargo clean --release`).

## Signing and notarization

The script requires a signing identity and refuses an unsigned app unless you pass `--unsigned` (the release notes then say the app is unsigned and how to open it).

```sh
# Developer ID Application certificate installed in the login keychain
export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"

# Notarization (recommended): an app-specific password from appleid.apple.com
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="TEAMID"
```

With the identity only, the app is signed; with all four variables it is also notarized, and the script verifies the result with `codesign`, `stapler` and `spctl`. Keep these values out of the repository and out of shell history files.

## Not covered yet

Windows packaging, Intel Macs, and a dedicated tap repository (the main repository serves as the tap). The license is still undecided (`UNLICENSED`), which should be settled before binaries are widely shared.
