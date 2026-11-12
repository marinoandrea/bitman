# Embedded fonts

Copied unchanged from `spits/public/fonts`:

- **Departure Mono 1.500**, by Helena Zhang: app title and headings, regular weight.
- **Sometype Mono**, by Ryoichi Tsunekawa / Dharma Type: all other app content,
  with regular, medium, and bold faces in upright and italic styles.

`sources.json` preserves the upstream revisions, download URLs, and SHA-256
checksums from Spits. Both families' original SIL Open Font License 1.1 notices
are included in their respective directories.

`src/fonts.rs` embeds the original WOFF2 bytes using `include_bytes!` and registers
them with GPUI before opening the window. The macOS native font loader supports
these files directly. Builds and app bundles need no external font files,
installed copies, conversion step, downloads, or additional dependencies.

The copyright and license notices are also embedded in the executable. Read them
with `cargo run --locked -- --font-licenses`, or `bitman --font-licenses` for a
built executable.
