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

**6. Homebrew** (the repository is its own tap). Write `Formula/tracr-cli.rb` and `Casks/tracr.rb` with the new version and the two checksums from step 3, commit, push.

```ruby
# Formula/tracr-cli.rb
class TracrCli < Formula
  desc "Local development diary and time ledger (command line)"
  homepage "https://github.com/JoKeks2023/tracr"
  version "0.9.0"
  url "https://github.com/JoKeks2023/tracr/releases/download/v0.9.0/tracr-0.9.0-aarch64-apple-darwin.tar.gz"
  sha256 "<sha256 of the tar.gz>"
  depends_on arch: :arm64
  depends_on :macos
  def install
    bin.install "tracr"
  end
  test do
    assert_match "tracr", shell_output("#{bin}/tracr --version")
  end
end
```

```ruby
# Casks/tracr.rb
cask "tracr" do
  version "0.9.0"
  sha256 "<sha256 of the zip>"
  url "https://github.com/JoKeks2023/tracr/releases/download/v#{version}/tracr-#{version}-aarch64-apple-darwin.zip"
  name "tracr"
  desc "Local development diary and time ledger"
  homepage "https://github.com/JoKeks2023/tracr"
  depends_on arch: :arm64
  depends_on :macos
  app "tracr.app"
  zap trash: "~/Library/Application Support/tracr"
end
```

**7. Check on a clean setup**
```sh
brew tap JoKeks2023/tracr https://github.com/JoKeks2023/tracr
brew install --cask tracr
brew install tracr-cli
```

## Not covered yet

Windows packaging, Intel Macs, and a dedicated tap repository.
