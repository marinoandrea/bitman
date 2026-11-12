# Bitman

Native Rust/GPUI desktop rewrite of Boot Tool for image conversion, SVG export,
and print preparation. `src/effects/` contains reusable processing buffers,
parallel kernels, and NEON/SSE2 SIMD; `src/editor.rs` connects processing to the
GPUI workspace. Fonts and icons live in `assets/`; packaging lives in `packaging/`
and `scripts/`. Preserve Boot Tool attribution and bundled font licenses.

- Keep changes focused and preserve unrelated work. Follow existing Rust patterns.
- Use Conventional Commits for every commit and squash PR title:
  `feat:`, `fix:`, `perf:`, `refactor:`, `docs:`, `test:`, `build:`, `ci:`, or `chore:`.
  Add an optional scope; use `!` or a `BREAKING CHANGE:` footer for breaking changes.
- **Update the version when relevant through release-please.** It is the sole
  authority for SemVer, `Cargo.toml`, `Cargo.lock`, the manifest,
  changelog, tags, and GitHub releases. Use `feat` for features and `fix`/`perf` for
  patches; review and merge its release PR. Do not independently bump versions or
  create release tags. Packages and the About dialog derive versions from Cargo.
- Before finishing, run `cargo fmt --all --check`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo test --locked`, and
  `python3 scripts/check-release.py`. For build/release changes also run
  `actionlint`, `cargo build --locked --release`, and native packaging via
  `python3 scripts/package.py`. Report platforms you could not verify locally.
- CI tests and packages macOS (arm64/x64), Linux (x64), and Windows (x64).
  Keep release binaries portable: never use `target-cpu=native` in published builds.
- Pin GitHub Actions to full commit SHAs with version comments. Dependabot checks
  Cargo and Actions weekly; review dependency bumps and keep `Cargo.lock` current.
- CI and releases share Rust dependency caches per OS/target, toolchain, build
  environment, and manifests. Only successful main builds save; PRs restore.

## Release operations

Pushes to `main` run release-please and CI. Merge its release PR to publish the
SemVer tag; only successful CI on that exact commit uploads packages. The empty manifest bootstraps `0.1.0`
from the full history. Package versions and `--version` come from Cargo; the README
badge follows GitHub Releases. Do not add operational documentation to the README.

Only the built-in `GITHUB_TOKEN` is needed. Repository Actions settings must allow
PR creation. The release workflow explicitly dispatches CI on bot release branches
so bot PR checks need no PAT or manual workflow approval. If uploads fail,
rerun failed jobs in the original run to retain its release outputs and artifacts.

`scripts/package.py` packages a prebuilt release binary into `dist/`; pass the same
`--target` used by Cargo. It creates macOS app ZIPs/DMGs (ad-hoc signed, not
notarized), Linux tarballs/Debian packages (Ubuntu 22.04/glibc 2.35+), and Windows
portable ZIPs (unsigned, static MSVC runtime), plus checksums and commit metadata.
macOS needs Xcode Command Line Tools; Windows needs the C++ build tools and SDK
shader compiler (`GPUI_FXC_PATH`). Linux build dependencies are listed in CI;
runtime requires Vulkan, Fontconfig, X11/Wayland, xkbcommon, and a desktop portal.
