#!/bin/bash
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
    echo "Generating the macOS icon requires macOS (sips and iconutil)." >&2
    exit 1
fi

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_icon="$project_root/assets/icons/app-icon.png"
destination="$project_root/assets/icons/macos/Bitman.icns"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/bitman-icon.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
iconset="$work_dir/Bitman.iconset"
mkdir -p "$iconset" "$(dirname "$destination")"

for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$source_icon" \
        --out "$iconset/icon_${size}x${size}.png" >/dev/null
    retina_size=$((size * 2))
    sips -z "$retina_size" "$retina_size" "$source_icon" \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil --convert icns "$iconset" --output "$work_dir/Bitman.icns"
mv "$work_dir/Bitman.icns" "$destination"
echo "Generated $destination"
