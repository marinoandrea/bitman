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

cargo build --target-dir "$target_root" "${cargo_args[@]}"
CARGO_TARGET_DIR="$target_root" python3 scripts/package.py --bundle-only --profile "$profile"
