# Releasing tracr

Releases are made by hand on the developer's Apple Silicon Mac: build, sign, notarize, upload. There is no release CI and no release script; the `ci` workflow only checks code, and `pages.yml` only publishes the static site.

## What a release contains

| File | Contents |
|---|---|
| `tracr-<version>-universal.dmg` | the desktop app (`tracr.app`) for Apple Silicon and Intel, with the `tracr` command line tool inside (`Contents/MacOS/tracr`). Signed, notarized, stapled. The main download on the website (`/download`) and what the Homebrew cask installs. |
| `tracr-<version>-aarch64.dmg`, `tracr-<version>-x64.dmg` | the same app for one architecture (smaller), at `/download/apple-silicon` and `/download/intel` |
| `tracr-<version>-universal.pkg` | an installer for the universal app (`/Applications/tracr.app`) that also links `/usr/local/bin/tracr` to the command line tool inside (`apps/desktop/src-tauri/pkg/postinstall`). Signed with the Developer ID Installer certificate, notarized, stapled. At `/download/pkg`. |
| `tracr-<version>-universal-apple-darwin.tar.gz` | the command line tool alone (universal), for `brew install tracr-cli` |

## One-time setup

- Developer ID Application certificate in the login keychain (`security find-identity -v -p codesigning`).
- An App Store Connect **team** API key (role Developer), the `.p8` kept outside the repo (`~/.appstoreconnect/private_keys/`, mode 600), plus its key ID and issuer ID.
- A notary profile for the DMG: `xcrun notarytool store-credentials tracr-notary --key <p8> --key-id <id> --issuer <uuid>`.
- The terminal app that runs the build may control Finder (System Settings, Privacy & Security, Automation). The DMG window is laid out through Finder; without the permission `bundle_dmg.sh` fails with `-1712`.

## Checklist

Replace `0.9.2` below. Needs at least 8 GB free disk (`df -h /`).

**1. Version and checks**
- Set the version in the workspace `Cargo.toml`, the crates' path dependencies, `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`, then `cargo update --workspace --offline`.
- `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && pnpm --dir apps/desktop typecheck`
- Commit and push to `main`.

**2. Build** (the CLI first, because the app embeds it as a sidecar; Tauri expects one sidecar per target triple)
```sh
rustup target add x86_64-apple-darwin          # once
for T in aarch64-apple-darwin x86_64-apple-darwin; do cargo build --release --locked -p tracr-cli --target $T; done
mkdir -p apps/desktop/src-tauri/binaries
cp target/aarch64-apple-darwin/release/tracr apps/desktop/src-tauri/binaries/tracr-aarch64-apple-darwin
cp target/x86_64-apple-darwin/release/tracr  apps/desktop/src-tauri/binaries/tracr-x86_64-apple-darwin
lipo -create target/aarch64-apple-darwin/release/tracr target/x86_64-apple-darwin/release/tracr \
  -output apps/desktop/src-tauri/binaries/tracr-universal-apple-darwin
cd apps/desktop
for T in aarch64-apple-darwin x86_64-apple-darwin universal-apple-darwin; do
  APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)" \
  APPLE_API_KEY=<key id> APPLE_API_ISSUER=<issuer id> \
  APPLE_API_KEY_PATH=$HOME/.appstoreconnect/private_keys/AuthKey_<key id>.p8 \
    pnpm tauri build --target $T --config src-tauri/tauri.release.conf.json
done
```
`tauri.release.conf.json` adds the embedded CLI and the `dmg` target; the normal config does not, so `cargo test`, CI and `pnpm tauri dev` need no sidecar file. With the three `APPLE_API_*` variables Tauri notarizes and staples each app before it builds its DMG. Run each build in one go: if you run it again with `--bundles dmg` and without those variables, the app is re-signed and the notarization ticket is lost. The DMGs land in `target/<triple>/release/bundle/dmg/` as `tracr_<version>_aarch64.dmg`, `_x64.dmg` and `_universal.dmg`.

**3. Notarize the DMGs**
```sh
for pair in aarch64-apple-darwin:aarch64 x86_64-apple-darwin:x64 universal-apple-darwin:universal; do
  D=target/${pair%%:*}/release/bundle/dmg/tracr_0.9.2_${pair##*:}.dmg
  xcrun notarytool submit $D --keychain-profile tracr-notary --wait
  xcrun stapler staple $D
done
```

**4. Check** (for each DMG; `$D` as above)
```sh
xcrun stapler validate $D
spctl --assess --type open --context context:primary-signature -v $D     # Notarized Developer ID
hdiutil attach -nobrowse -readonly -mountpoint /tmp/tracr-dmg-check $D
spctl --assess --type execute -v /tmp/tracr-dmg-check/tracr.app          # Notarized Developer ID
lipo -archs /tmp/tracr-dmg-check/tracr.app/Contents/MacOS/tracr-desktop   # arm64 | x86_64 | x86_64 arm64
/tmp/tracr-dmg-check/tracr.app/Contents/MacOS/tracr --version             # arm64 and universal only, unless Rosetta is installed
hdiutil detach /tmp/tracr-dmg-check
```
Then open one DMG in Finder (background, icon positions) and start the app from it. An Intel build cannot be run on an Apple Silicon Mac without Rosetta; without it the Intel DMG is only checked statically (signature, notarization, architecture).

**5. Package**
```sh
mkdir -p dist/tracr-0.9.2-universal-apple-darwin
cp target/aarch64-apple-darwin/release/bundle/dmg/tracr_0.9.2_aarch64.dmg   dist/tracr-0.9.2-aarch64.dmg
cp target/x86_64-apple-darwin/release/bundle/dmg/tracr_0.9.2_x64.dmg        dist/tracr-0.9.2-x64.dmg
cp target/universal-apple-darwin/release/bundle/dmg/tracr_0.9.2_universal.dmg dist/tracr-0.9.2-universal.dmg
cp apps/desktop/src-tauri/binaries/tracr-universal-apple-darwin dist/tracr-0.9.2-universal-apple-darwin/tracr
codesign --force -s - dist/tracr-0.9.2-universal-apple-darwin/tracr   # lipo drops the linker signature
tar -C dist -czf dist/tracr-0.9.2-universal-apple-darwin.tar.gz tracr-0.9.2-universal-apple-darwin
shasum -a 256 dist/*.dmg dist/*.tar.gz
```

**5b. Installer package** (needs the *Developer ID Installer* certificate in the login keychain)
```sh
W=$(mktemp -d); mkdir -p $W/root/Applications
ditto target/universal-apple-darwin/release/bundle/macos/tracr.app $W/root/Applications/tracr.app   # the stapled app
pkgbuild --analyze --root $W/root $W/comp.plist
plutil -replace 0.BundleIsRelocatable -bool false $W/comp.plist
pkgbuild --root $W/root --component-plist $W/comp.plist --scripts apps/desktop/src-tauri/pkg \
  --ownership recommended --identifier app.tracr.desktop.pkg --version 0.9.2 --install-location / $W/tracr-component.pkg
productbuild --synthesize --package $W/tracr-component.pkg $W/dist.xml     # add <title>tracr</title> after the first line
productbuild --distribution $W/dist.xml --package-path $W --timestamp \
  --sign "Developer ID Installer: Name (TEAMID)" dist/tracr-0.9.2-universal.pkg
xcrun notarytool submit dist/tracr-0.9.2-universal.pkg --keychain-profile tracr-notary --wait
xcrun stapler staple dist/tracr-0.9.2-universal.pkg
spctl --assess --type install -v dist/tracr-0.9.2-universal.pkg            # Notarized Developer ID
```
The postinstall only runs as root during a real install. Test it on a spare machine or a throwaway account; uninstalling is manual (`sudo rm -rf /Applications/tracr.app /usr/local/bin/tracr && sudo pkgutil --forget app.tracr.desktop.pkg`).

**6. GitHub Release** (pre-release while the major version is 0; notes are grouped by label, see `.github/release.yml`)
```sh
git tag -a v0.9.2 -m "tracr 0.9.2" && git push origin v0.9.2
gh release create v0.9.2 dist/*.dmg dist/*.pkg dist/*.tar.gz --prerelease --generate-notes --title "tracr 0.9.2"
```
A pre-release is not what `releases/latest` points to, so links use the explicit version.

**7. Homebrew.** The tap is its own repository, [`JoKeks2023/homebrew-tracr`](https://github.com/JoKeks2023/homebrew-tracr). In `Casks/tracr.rb` set `version` and the DMG checksum, in `Formula/tracr-cli.rb` set `version` and the tar.gz checksum (compare with `gh release download` first), commit and push there. The cask installs the app and links the embedded CLI via its `binary` stanza. The formula and the cask both provide `tracr`; Homebrew only warns and overwrites the link when both are installed, so install one of them.

**8. Website.** The download button and the README use the stable link `https://tracr.jorisconrad.com/download`, which `site/public/_redirects` sends to the universal DMG of the current release (`/download/apple-silicon` and `/download/intel` go to the single-architecture ones, `/download/pkg` to the installer). Change the four URLs there, replace the version labels (`0.9.2`) in `site/public/en/index.html`, `de/index.html` and the README, commit, push (this publishes GitHub Pages), then deploy to Cloudflare with `npx wrangler deploy` in `site/` and check `curl -sIL https://tracr.jorisconrad.com/download`.

**9. Check on a clean setup**
```sh
brew update && brew upgrade --cask tracr
tracr --version
spctl --assess --type execute -v /Applications/tracr.app
```
First install: `brew tap JoKeks2023/tracr && brew trust JoKeks2023/tracr`, then `brew install --cask tracr`.

## Not covered yet

Windows packaging, a stable "latest" download link (needs a non-pre-release), and notarizing the standalone command line tool (it is signed ad hoc only; the copy inside the app is signed and notarized with it).
