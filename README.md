# Bitman

A native GPUI image and print workspace based on Boot Tool. Convert and Print
now run a Rust effects engine with reusable buffers, SIMD kernels, and chunked
computation for large independent stages. GPUI remains the only direct dependency.

## Run

On macOS with Xcode selected as the active developer directory:

```sh
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
cargo run --locked
```

`rust-toolchain.toml` selects stable Rust (verified with 1.99.1). `Cargo.lock`
locks the dependency graph. GPUI 0.2.2 is the only direct dependency. Only
`font-kit` (required by GPUI's native font loader) and `runtime_shaders`
are enabled. Departure Mono (titles and headings) and Sometype Mono (copy and
controls) are embedded in the executable from local assets copied from Spits.
No font installation or runtime download is needed. Font licenses are embedded
too; run `cargo run --locked -- --font-licenses` to read them. See
[assets/fonts/README.md](assets/fonts/README.md) for sources and notices.
`runtime_shaders` uses the system Metal
runtime so the separate Xcode Metal compiler download is unnecessary.

This initial setup targets macOS and its native Metal backend. Other operating
systems will need their GPUI platform features and prerequisites enabled.

## macOS app icon and bundle

To launch with the application icon in Finder and the Dock:

```sh
./scripts/bundle-macos.sh
open target/debug/Bitman.app
```

Use `./scripts/bundle-macos.sh --release` for `target/release/Bitman.app`.
`CARGO_TARGET_DIR` is respected when set. The script builds the binary, packages
the icon and macOS metadata, and applies a local ad-hoc signature. It uses the
existing Rust and macOS tools; no additional packages are required.

The original PNG lives in `assets/icons/app-icon.png`, with the generated ICNS in
`assets/icons/macos/Bitman.icns`. See [assets/README.md](assets/README.md) for how
to update it. `cargo run` launches the bare executable; the OS icon is supplied
by the `.app` bundle.

## Effects

Open or drop an image, adjust the labelled controls, compare the SVG with the
original, and save the result. Convert includes thresholding, scan cleanup,
colour quantization, and vector tracing. Print includes one-ink/CMYK/spot
separations, halftones, seven dithers, custom stamps, dark-garment inversion,
underbase, trapping, and layered SVG output with registration marks.

Lloyd k-means is the default after benchmarking quality and speed against a
from-scratch adaptation of Meta's e-means. Both are selectable. See
[engine algorithms and memory model](docs/effects.md) and
[benchmark results](docs/benchmarks/README.md).

The preview is capped at 1024 pixels per side and export at 4096. No image is
uploaded. Compose, Simulate, Batch, the library, and their editing/queue tools
remain layout blueprints. Fit/100% are still presentation controls.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --example effect_gallery -- /tmp/bitman-effect-gallery
./scripts/benchmark-effects.sh
```

## Source

- `src/main.rs`: native application and window lifecycle.
- `src/app.rs`: workspace navigation, display state, and shared shell.
- `src/controls.rs`: shared control styling and colours.
- `src/fonts.rs`: embedded fonts, licenses, and startup registration.
- `src/screens.rs`: remaining workspace blueprints and canvas layouts.
- `src/editor.rs`: image dialogs, background processing, preview lifecycle, exports.
- `src/effect_ui.rs`: live effects controls and image comparison views.
- `src/effects/`: standard-library-only numerical kernels and SVG generation.
- `tests/effects.rs`: numerical, topology, safety, threading, and allocation checks.

The layout reference is `sshhiiff/boot-tool` at commit
`5bd1a4303093d01bbab3d1f5b61cbc8f19699e3a`. No Python code or Paper.js was copied. No image-processing package, UI component
kit, or runtime asset download was added.

## Checks

```sh
cargo fmt --all --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
```
