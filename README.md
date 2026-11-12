# Bitman

A native GPUI layout prototype based on Boot Tool's Convert, Compose, Print,
Simulate, and Help workspaces. It reproduces the header navigation, scrolling
inspector, preview toolbar, canvas, library shelf, and output statistics with a
neutral visual style.

## Run

On macOS with Xcode selected as the active developer directory:

```sh
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
cargo run --locked
```

`rust-toolchain.toml` selects stable Rust (verified with 1.99.1). `Cargo.lock`
locks the dependency graph. GPUI 0.2.2 is the only direct dependency. Only
`font-kit` (required to render installed system fonts) and `runtime_shaders`
are enabled. No fonts are downloaded. `runtime_shaders` uses the system Metal
runtime so the separate Xcode Metal compiler download is unnecessary.

This initial setup targets macOS and its native Metal backend. Other operating
systems will need their GPUI platform features and prerequisites enabled.

## Scope

Navigation, preview tabs, editor tool selection, and layout-related selectors
switch the displayed panels. Print includes one-ink, CMYK, spot flat/photo,
screen/dither, and custom-pattern layouts. Convert includes the batch layout.

Sliders, text fields, checkboxes, editing tools, file dialogs, library access,
processing, and exports are presentation-only placeholders. The app does not
read or write image files, perform image processing, or make network requests.
The artboard and shirt silhouette are decorative GPUI drawing primitives.

## Source

- `src/main.rs`: native application and window lifecycle.
- `src/app.rs`: workspace navigation, display state, and shared shell.
- `src/controls.rs`: reusable presentation-only controls and colours.
- `src/screens.rs`: screen inspectors and canvas layouts.

The layout reference is `sshhiiff/boot-tool` at commit
`5bd1a4303093d01bbab3d1f5b61cbc8f19699e3a`. No Python code, Paper.js, image
processing packages, UI component kits, fonts, or asset downloads are included.

## Checks

```sh
cargo fmt --all --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
```
