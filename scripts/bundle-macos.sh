#!/bin/bash
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
    echo "Building the macOS app bundle requires macOS." >&2
    exit 1
fi

profile=debug
cargo_args=(--locked)
if [[ $# -eq 1 && "$1" == --release ]]; then
    profile=release
    cargo_args=(--locked --release)
elif [[ $# -ne 0 ]]; then
    echo "Usage: $0 [--release]" >&2
    exit 1
fi

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
target_root="${CARGO_TARGET_DIR:-$project_root/target}"
mkdir -p "$target_root"
target_root="$(cd "$target_root" && pwd)"

"$project_root/scripts/generate-app-icon.sh"
cargo build --target-dir "$target_root" "${cargo_args[@]}"

bundle="$target_root/$profile/Bitman.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$target_root/$profile/bitman" "$bundle/Contents/MacOS/bitman"
cp "$project_root/assets/icons/macos/Bitman.icns" "$bundle/Contents/Resources/Bitman.icns"
cp "$project_root/packaging/macos/Info.plist" "$bundle/Contents/Info.plist"

# This project has one package; keep the bundle version in sync with Cargo.toml.
version="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$project_root/Cargo.toml")"
if [[ -z "$version" || "$version" == *$'\n'* ]]; then
    echo "Could not read a single package version from Cargo.toml." >&2
    exit 1
fi
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$bundle/Contents/Info.plist"
plutil -lint "$bundle/Contents/Info.plist"
codesign --force --sign - "$bundle"
echo "Built $bundle"
