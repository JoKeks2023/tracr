# Releasing tracr

Releases are made by hand on the developer's Apple Silicon Mac: build, sign, upload. There is no release CI and no release script; the `ci` workflow only checks code.

## What a release contains

| File | Contents |
|---|---|
| `tracr-<version>-aarch64-apple-darwin.tar.gz` | the `tracr` command line tool |
| `tracr-<version>-aarch64-apple-darwin.zip` | the desktop app (`tracr.app`) |

## Checklist

Replace `0.9.0` below. Needs at least 8 GB free disk (`df -h /`); a release build adds several GB under `target/release`.

**1. Version and checks**
- Set the version in the workspace `Cargo.toml`, the crates' path dependencies, `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`.
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && pnpm --dir apps/desktop typecheck`
- Commit and push to `main`.

**2. Build**
```sh
cargo build --release --locked -p tracr-cli
cd apps/desktop && pnpm install --frozen-lockfile && pnpm tauri build --bundles app && cd ../..
```
The app is at `target/release/bundle/macos/tracr.app`. To sign, have the Developer ID certificate in the login keychain and export `APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"` before `pnpm tauri build`. For notarization also export `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and `APPLE_TEAM_ID`.

**3. Package**
```sh
mkdir -p dist/tracr-0.9.0-aarch64-apple-darwin
cp target/release/tracr dist/tracr-0.9.0-aarch64-apple-darwin/
tar -C dist -czf dist/tracr-0.9.0-aarch64-apple-darwin.tar.gz tracr-0.9.0-aarch64-apple-darwin
ditto -c -k --keepParent target/release/bundle/macos/tracr.app dist/tracr-0.9.0-aarch64-apple-darwin.zip
shasum -a 256 dist/*.tar.gz dist/*.zip
```

**4. Try it**
- `target/release/tracr --version`, open the app from `dist`.
- If signed: `codesign --verify --deep --strict target/release/bundle/macos/tracr.app`; if notarized also `xcrun stapler validate …` and `spctl --assess --type execute -v …`.

**5. GitHub Release** (pre-release while the major version is 0; notes are grouped by label, see `.github/release.yml`)
```sh
git tag -a v0.9.0 -m "tracr 0.9.0" && git push origin v0.9.0
gh release create v0.9.0 dist/*.tar.gz dist/*.zip --prerelease --generate-notes --title "tracr 0.9.0"
```
If the app is unsigned, say so in the notes: macOS asks for confirmation on first launch (right-click, Open).

**6. Homebrew.** The tap is its own repository, [`JoKeks2023/homebrew-tracr`](https://github.com/JoKeks2023/homebrew-tracr). Update `version`, `url` and `sha256` in `Formula/tracr-cli.rb` (checksum of the tar.gz) and `Casks/tracr.rb` (checksum of the zip), commit and push there. Compare the checksums with `gh release download` first.

**7. Check on a clean setup**
```sh
brew update && brew upgrade tracr-cli && brew upgrade --cask tracr
tracr --version
spctl --assess --type execute -v /Applications/tracr.app
```
First install: `brew tap JoKeks2023/tracr && brew trust JoKeks2023/tracr`, then `brew install --cask tracr` and `brew install tracr-cli`.

## Notarization

Notarize before packaging (step 3). One-time: an App Store Connect **team** API key (role Developer) stored with `xcrun notarytool store-credentials tracr-notary --key <p8> --key-id <id> --issuer <uuid>`; keep the `.p8` outside the repo, e.g. in `~/.appstoreconnect/private_keys/` (mode 600). Then:

```sh
ditto -c -k --keepParent target/release/bundle/macos/tracr.app dist/notarize.zip
xcrun notarytool submit dist/notarize.zip --keychain-profile tracr-notary --wait
xcrun stapler staple target/release/bundle/macos/tracr.app && rm dist/notarize.zip
spctl --assess --type execute -v target/release/bundle/macos/tracr.app   # "Notarized Developer ID"
```

## Not covered yet

Windows packaging, Intel Macs, notarizing the command line binary (it is signed ad hoc only).
