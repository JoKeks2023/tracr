#!/bin/sh
# Renders Formula/tracr-cli.rb and Casks/tracr.rb from .github/brew/ for one release.
# Usage: scripts/render-brew.sh <version> <cli sha256> <app sha256>
set -eu
[ "$#" -eq 3 ] || { echo "usage: $0 <version> <cli sha256> <app sha256>" >&2; exit 2; }
version=$1 cli_sha=$2 app_sha=$3
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/Formula" "$root/Casks"
sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$cli_sha/g" "$root/.github/brew/tracr-cli.rb.in" > "$root/Formula/tracr-cli.rb"
sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$app_sha/g" "$root/.github/brew/tracr.rb.in" > "$root/Casks/tracr.rb"
