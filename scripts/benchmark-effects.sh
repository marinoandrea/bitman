#!/bin/bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
mkdir -p target/benchmarks
# This harness needs only std; it does not rebuild the GUI or install a benchmarking crate.
rustc --edition=2024 -C opt-level=3 -C target-cpu=native \
    benches/effects.rs -o target/benchmarks/effects
exec target/benchmarks/effects "$@"
