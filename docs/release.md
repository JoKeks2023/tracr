# Releasing tracr

Releases are made by hand on the developer's Apple Silicon Mac: build, sign, notarize, upload. There is no release CI and no release script; the `ci` workflow only checks code, and `pages.yml` only publishes the static site.

## What a release contains

| File | Contents |
|---|---|
| `tracr-<version>-aarch64.dmg` | the desktop app (`tracr.app`) with the `tracr` command line tool inside (`Contents/MacOS/tracr`). Signed, notarized, stapled. This is the download on the website and what the Homebrew cask installs. |
| `tracr-<version>-aarch64-apple-darwin.tar.gz` | the command line tool alone, for `brew install tracr-cli` |

## One-time setup

- Developer ID Application certificate in the login keychain (`security find-identity -v -p codesigning`).
- An App Store Connect **team** API key (role Developer), the `.p8` kept outside the repo (`~/.appstoreconnect/private_keys/`, mode 600), plus its key ID and issuer ID.
- A notary profile for the DMG: `xcrun notarytool store-credentials tracr-notary --key <p8> --key-id <id> --issuer <uuid>`.
- The terminal app that runs the build may control Finder (System Settings, Privacy & Security, Automation). The DMG window is laid out through Finder; without the permission `bundle_dmg.sh` fails with `-1712`.

## Checklist

Replace `0.9.1` below. Needs at least 8 GB free disk (`df -h /`).

**1. Version and checks**
- Set the version in the workspace `Cargo.toml`, the crates' path dependencies, `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`, then `cargo update --workspace --offline`.
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && pnpm --dir apps/desktop typecheck`
- Commit and push to `main`.

**2. Build** (the CLI first, because the app embeds it as a sidecar)
```sh
cargo build --release --locked -p tracr-cli
mkdir -p apps/desktop/src-tauri/binaries
cp target/release/tracr apps/desktop/src-tauri/binaries/tracr-aarch64-apple-darwin
cd apps/desktop
APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)" \
APPLE_API_KEY=<key id> APPLE_API_ISSUER=<issuer id> \
APPLE_API_KEY_PATH=$HOME/.appstoreconnect/private_keys/AuthKey_<key id>.p8 \
  pnpm tauri build --config src-tauri/tauri.release.conf.json
```
`tauri.release.conf.json` adds the embedded CLI and the `dmg` target; the normal config does not, so `cargo test`, CI and `pnpm tauri dev` need no sidecar file. With the three `APPLE_API_*` variables Tauri notarizes and staples the app before it builds the DMG. Run the build in one go: if you run it again with `--bundles dmg` and without those variables, the app is re-signed and the notarization ticket is lost.

**3. Notarize the DMG**
```sh
D=target/release/bundle/dmg/tracr_0.9.1_aarch64.dmg
xcrun notarytool submit $D --keychain-profile tracr-notary --wait
xcrun stapler staple $D
```

**4. Check**
```sh
xcrun stapler validate $D
spctl --assess --type open --context context:primary-signature -v $D     # Notarized Developer ID
hdiutil attach -nobrowse -readonly -mountpoint /tmp/tracr-dmg-check $D
spctl --assess --type execute -v /tmp/tracr-dmg-check/tracr.app          # Notarized Developer ID
/tmp/tracr-dmg-check/tracr.app/Contents/MacOS/tracr --version
hdiutil detach /tmp/tracr-dmg-check
```
Then open the DMG in Finder once and look at the window (background, icon positions). Open the app from it.

**5. Package**
```sh
mkdir -p dist/tracr-0.9.1-aarch64-apple-darwin
cp $D dist/tracr-0.9.1-aarch64.dmg
cp target/release/tracr dist/tracr-0.9.1-aarch64-apple-darwin/
tar -C dist -czf dist/tracr-0.9.1-aarch64-apple-darwin.tar.gz tracr-0.9.1-aarch64-apple-darwin
shasum -a 256 dist/*.dmg dist/*.tar.gz
```

**6. GitHub Release** (pre-release while the major version is 0; notes are grouped by label, see `.github/release.yml`)
```sh
git tag -a v0.9.1 -m "tracr 0.9.1" && git push origin v0.9.1
gh release create v0.9.1 dist/*.dmg dist/*.tar.gz --prerelease --generate-notes --title "tracr 0.9.1"
```
A pre-release is not what `releases/latest` points to, so links use the explicit version.

**7. Homebrew.** The tap is its own repository, [`JoKeks2023/homebrew-tracr`](https://github.com/JoKeks2023/homebrew-tracr). In `Casks/tracr.rb` set `version` and the DMG checksum, in `Formula/tracr-cli.rb` set `version` and the tar.gz checksum (compare with `gh release download` first), commit and push there. The cask installs the app and links the embedded CLI via its `binary` stanza. The formula and the cask both provide `tracr`; Homebrew only warns and overwrites the link when both are installed, so install one of them.

**8. Website.** In `site/public/en/index.html`, `de/index.html` and the README replace the version and the DMG link, commit, push (this publishes GitHub Pages), then deploy to Cloudflare with `npx wrangler deploy` in `site/`.

**9. Check on a clean setup**
```sh
brew update && brew upgrade --cask tracr
tracr --version
spctl --assess --type execute -v /Applications/tracr.app
```
First install: `brew tap JoKeks2023/tracr && brew trust JoKeks2023/tracr`, then `brew install --cask tracr`.

## Not covered yet

Windows packaging, Intel Macs, a stable "latest" download link (needs a non-pre-release), and notarizing the standalone command line tool (it is signed ad hoc only; the copy inside the app is signed and notarized with it).
