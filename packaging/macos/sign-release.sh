#!/bin/zsh
# Signs, notarizes and packages a release that the `release-build` workflow
# built unsigned on GitHub. Runs on the developer's Mac: the Developer ID
# certificates stay in the login keychain and notarization uses the local
# `tracr-notary` profile. Nothing secret is sent to GitHub.
#
#   packaging/macos/sign-release.sh 0.12.0            # newest run for tag v0.12.0
#   packaging/macos/sign-release.sh 0.12.0 <run-id>   # a specific run
#
# TRACR_TEST_ARTIFACT=<name> signs a test build (e.g. from a pull request)
# without the tag check; its output is for checking only, never for upload.
#
# Output in dist/: the three DMGs, the universal PKG and the CLI tar.gz,
# then checks, a personal-data scan and checksums. Upload stays a manual
# step (docs/release.md §6).
set -euo pipefail

V=${1:?version, e.g. 0.12.0}
RUN=${2:-}
APP_ID=${TRACR_APP_IDENTITY:-"Developer ID Application: Sven Friedemann Conrad (T9CA6D7T8N)"}
PKG_ID=${TRACR_PKG_IDENTITY:-"Developer ID Installer: Sven Friedemann Conrad (T9CA6D7T8N)"}
PROFILE=${TRACR_NOTARY_PROFILE:-tracr-notary}

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"
W=$(mktemp -d)
trap 'rm -rf "$W"' EXIT

# 1. Fetch the unsigned build and check it belongs to the tag.
if [[ -z $RUN ]]; then
  RUN=$(gh run list --workflow release-build --branch "v$V" --status success --limit 1 --json databaseId -q '.[0].databaseId')
fi
[[ -n $RUN ]] || { echo "no successful release-build run for v$V"; exit 1; }
gh run download "$RUN" -n "${TRACR_TEST_ARTIFACT:-tracr-unsigned-v$V}" -D "$W/in"
(cd "$W/in" && shasum -a 256 -c SHA256SUMS)
if [[ -z ${TRACR_TEST_ARTIFACT:-} ]]; then
  [[ $(cat "$W/in/COMMIT") == $(git rev-parse "v$V^{commit}") ]] || { echo "artifact is not from v$V"; exit 1; }
fi

notarize() { # file → notarized and stapled (an .app is sent as a zip)
  local f=$1 sub=$1
  if [[ $f == *.app ]]; then sub="$W/notarize.zip"; rm -f "$sub"; ditto -c -k --keepParent "$f" "$sub"; fi
  xcrun notarytool submit "$sub" --keychain-profile "$PROFILE" --wait | grep -E "^  status:"
  xcrun stapler staple "$f" | tail -1
}

mkdir -p dist
typeset -A SUFFIX=(aarch64-apple-darwin aarch64 x86_64-apple-darwin x64 universal-apple-darwin universal)
for T in aarch64-apple-darwin x86_64-apple-darwin universal-apple-darwin; do
  A="$W/$T/tracr.app"
  mkdir -p "$W/$T" && ditto -x -k "$W/in/tracr-$T.app.zip" "$W/$T"
  # 2. Sign inside out: the embedded CLI, then the app. Hardened runtime, timestamp.
  codesign --force --options runtime --timestamp -s "$APP_ID" "$A/Contents/MacOS/tracr"
  codesign --force --options runtime --timestamp -s "$APP_ID" "$A"
  codesign --verify --deep --strict "$A"
  # 3. Notarize the app itself, so the copy inside the DMG and the PKG carries the ticket.
  notarize "$A"
  # 4. The DMG with the same window layout as tauri.conf.json (bundle.macOS.dmg).
  D="dist/tracr-$V-${SUFFIX[$T]}.dmg"
  rm -f "$D"
  mkdir -p "$W/$T/src" && ditto "$A" "$W/$T/src/tracr.app"
  bash packaging/macos/create-dmg/bundle_dmg.sh --volname tracr --volicon "$A/Contents/Resources/icon.icns" \
    --background apps/desktop/src-tauri/dmg/background.tiff --window-size 720 470 --icon-size 128 \
    --icon tracr.app 190 240 --hide-extension tracr.app --app-drop-link 530 240 "$D" "$W/$T/src" >/dev/null
  codesign --force --timestamp -s "$APP_ID" "$D"
  notarize "$D"
done

# 5. The installer package (universal app; postinstall links /usr/local/bin/tracr).
P=$W/pkg; mkdir -p $P/root/Applications
ditto "$W/universal-apple-darwin/tracr.app" $P/root/Applications/tracr.app
pkgbuild --analyze --root $P/root $P/comp.plist >/dev/null
plutil -replace 0.BundleIsRelocatable -bool false $P/comp.plist
pkgbuild --root $P/root --component-plist $P/comp.plist --scripts apps/desktop/src-tauri/pkg --ownership recommended \
  --identifier app.tracr.desktop.pkg --version "$V" --install-location / $P/tracr-component.pkg >/dev/null
productbuild --distribution apps/desktop/src-tauri/pkg/distribution.xml --resources apps/desktop/src-tauri/pkg/resources \
  --package-path $P --timestamp --sign "$PKG_ID" "dist/tracr-$V-universal.pkg" >/dev/null
notarize "dist/tracr-$V-universal.pkg"

# 6. The command line tool alone (ad-hoc signed: lipo drops the linker signature).
C="tracr-$V-universal-apple-darwin"
mkdir -p "$W/$C" && cp "$W/in/tracr-cli-universal" "$W/$C/tracr" && chmod 755 "$W/$C/tracr"
codesign --force -s - "$W/$C/tracr"
tar --uid 0 --gid 0 --uname root --gname wheel -C "$W" -czf "dist/$C.tar.gz" "$C"   # no local user name in the archive

# 7. Checks.
echo "== checks"
for f in dist/tracr-$V-*.dmg; do
  xcrun stapler validate "$f" | tail -1
  spctl --assess --type open --context context:primary-signature -v "$f" 2>&1 | head -1
done
spctl --assess --type install -v "dist/tracr-$V-universal.pkg" 2>&1 | head -1
M=$W/mnt
hdiutil attach -nobrowse -readonly -mountpoint $M "dist/tracr-$V-universal.dmg" >/dev/null
spctl --assess --type execute -v $M/tracr.app 2>&1 | head -1
lipo -archs $M/tracr.app/Contents/MacOS/tracr-desktop
$M/tracr.app/Contents/MacOS/tracr --version

# 8. Personal data: no home path, user name or e-mail in any binary.
echo "== personal-data scan (counts must be 0)"
for f in $M/tracr.app/Contents/MacOS/* "$W/$C/tracr"; do
  # The public domain tracr.<owner>.com is expected and not counted.
  echo "$(basename $f): $(strings -a "$f" | grep -iE "/Users/|$(id -un)|@web\.de" | grep -viE "tracr\.$(id -un)\.com" | grep -c . || true)"
done
hdiutil detach $M >/dev/null

shasum -a 256 dist/tracr-$V-*
stat -f "%z %N" dist/tracr-$V-*
openssl dgst -rmd160 "dist/$C.tar.gz"
echo SIGN-RELEASE-DONE
