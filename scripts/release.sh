#!/usr/bin/env bash
# Builds a tracr release on this Mac and, with --publish, releases it.
#
#   scripts/release.sh 0.9.0             build + package into dist/ (publishes nothing)
#   scripts/release.sh 0.9.0 --publish   also tag, create the GitHub Release, update Formula/Cask, push
#   scripts/release.sh 0.9.0 --unsigned  allow an unsigned app (otherwise signing is required)
#
# Signing: the Developer ID certificate must be in your login keychain.
#   export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"
# Notarization (optional, recommended): also
#   export APPLE_ID=... APPLE_PASSWORD=<app-specific password> APPLE_TEAM_ID=...
# See docs/release.md.
set -euo pipefail

version=${1:-}
[ -n "$version" ] || { sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }
shift
publish=0 unsigned=0
for a in "$@"; do
  case "$a" in
    --publish) publish=1 ;;
    --unsigned) unsigned=1 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
tag="v$version"
name="tracr-$version-aarch64-apple-darwin"
app="target/release/bundle/macos/tracr.app"

say() { printf '\n==> %s\n' "$*"; }
die() { echo "error: $*" >&2; exit 1; }

say "Checks"
[ "$(uname -s)-$(uname -m)" = "Darwin-arm64" ] || die "build on an Apple Silicon Mac (the release is aarch64-apple-darwin)"
for t in cargo pnpm gh codesign ditto shasum; do command -v "$t" >/dev/null || die "missing tool: $t"; done
crate=$(cargo metadata --no-deps --format-version 1 | python3 -c "import sys,json; print(next(p['version'] for p in json.load(sys.stdin)['packages'] if p['name']=='tracr-cli'))")
[ "$crate" = "$version" ] || die "crate version is $crate, not $version (bump the version first, see docs/release.md)"
free_gb=$(df -g . | awk 'NR==2 {print $4}')
[ "$free_gb" -ge 8 ] || die "only ${free_gb} GB free; a release build needs about 8 GB (cargo clean -p ... or free space)"
if [ "$publish" = 1 ]; then
  [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || die "publish from main"
  [ -z "$(git status --porcelain --untracked-files=no)" ] || die "uncommitted changes"
  git fetch -q origin main
  [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main differs from origin/main (push or pull first)"
  git rev-parse -q --verify "refs/tags/$tag" >/dev/null && die "tag $tag already exists"
  gh auth status >/dev/null 2>&1 || die "gh is not logged in"
fi
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  [ "$unsigned" = 1 ] || die "APPLE_SIGNING_IDENTITY is not set (pass --unsigned to build an unsigned app)"
  signed=no
elif [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
  signed=notarized
else
  signed=signed
fi
echo "version $version · signing: $signed · $free_gb GB free"

say "Command line tool"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}   # keep the 8 GB machine usable
cargo build --release --locked -p tracr-cli
rm -rf dist && mkdir -p "dist/$name"
cp target/release/tracr "dist/$name/tracr"
tar -C dist -czf "dist/$name.tar.gz" "$name"
rm -r "dist/$name"

say "Desktop app"
( cd apps/desktop && pnpm install --frozen-lockfile && pnpm tauri build --bundles app )
[ -d "$app" ] || die "no app bundle at $app"
ditto -c -k --keepParent "$app" "dist/$name.zip"

say "Verify"
tmp=$(mktemp -d); tar -xzf "dist/$name.tar.gz" -C "$tmp"; "$tmp/$name/tracr" --version
if [ "$signed" != no ]; then
  codesign --verify --deep --strict --verbose=2 "$app"
  if [ "$signed" = notarized ]; then
    xcrun stapler validate "$app"
    spctl --assess --type execute --verbose "$app"
  fi
fi
( cd dist && shasum -a 256 ./*.tar.gz ./*.zip | tee SHA256SUMS )

if [ "$publish" = 0 ]; then
  say "Built, nothing published"
  ls -la dist
  echo "Publish with: scripts/release.sh $version --publish"
  exit 0
fi

say "Publish $tag"
flags=(--generate-notes)
case "$version" in 0.*|*-*) flags+=(--prerelease) ;; esac
if [ "$signed" = no ]; then
  note='**This build of the macOS app is not signed.** On first launch macOS asks for confirmation (right-click, Open), or run `xattr -dr com.apple.quarantine /Applications/tracr.app`.'
else
  note="The macOS app is signed with a Developer ID ($signed)."
fi
git tag -a "$tag" -m "tracr $version"
git push origin "$tag"
gh release create "$tag" dist/* "${flags[@]}" --title "tracr $version" --notes "$note"

cli_sha=$(shasum -a 256 "dist/$name.tar.gz" | cut -d' ' -f1)
app_sha=$(shasum -a 256 "dist/$name.zip" | cut -d' ' -f1)
scripts/render-brew.sh "$version" "$cli_sha" "$app_sha"
git add Formula Casks
git commit -m "brew: tracr $version"
git push origin main
say "Done: $(gh release view "$tag" --json url -q .url)"
