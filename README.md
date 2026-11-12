<h1 align="center">
  <img src="assets/readme-logo.png" alt="Bitman" width="400">
</h1>

<p align="center">
  <a href="https://github.com/marinoandrea/bitman/actions/workflows/release.yml"><img src="https://github.com/marinoandrea/bitman/actions/workflows/release.yml/badge.svg?branch=main&amp;event=push" alt="Main builds and releases"></a>
  <a href="https://github.com/marinoandrea/bitman/actions/workflows/ci.yml"><img src="https://github.com/marinoandrea/bitman/actions/workflows/ci.yml/badge.svg?event=workflow_dispatch" alt="Release PR checks"></a>
  <a href="https://github.com/marinoandrea/bitman/releases"><img src="https://img.shields.io/github/v/release/marinoandrea/bitman?color=71F7EE&amp;style=flat" alt="Latest release"></a>
  <a href="Cargo.toml"><img src="https://img.shields.io/badge/Rust-1.99%2B-71F7EE?style=flat&amp;logo=rust&amp;logoColor=white" alt="Rust 1.99 or newer"></a>
  <a href="https://github.com/marinoandrea/bitman/pulls?q=is%3Apr+author%3Aapp%2Fdependabot"><img src="https://img.shields.io/badge/Dependabot-weekly-71F7EE?style=flat&amp;logo=dependabot&amp;logoColor=white" alt="Weekly Dependabot updates for Cargo and GitHub Actions"></a>
</p>

A native GPUI image and print workspace rewrite of [Boot Tool](https://github.com/sshhiiff/Boot-Tool).

All credits go to the original author [sshhiiff](https://github.com/sshhiiff) for the original application, its image-to-vector and screen-printing workflows, workspace design, interface layout, and colour palette.

## Why?

The original project does not make full use of native desktop performance.
This rewrite simply implements efficient image processing by leveraging native primitives such as SIMD and efficient memory allocations.

While the original tool can also be bundled as an application, it still renders in a browser engine.
This GPUI rewrite makes it into a fully standolone binary executable with targeted platform builds.

## How to use

Just pick your platform and download the application from the [latest release page](https://github.com/marinoandrea/bitman/releases/latest).

## Licensing

For the time being, the original project does not have a license, and we vouch to keep our own licensing in sync with it when and if it happens to introduce one for as long as this project constitutes substantially a 1:1 replica.
We might want to refactor this in the future if the project changes and grows beyond the original design.
