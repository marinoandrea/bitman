<h1 align="center">
  <img src="assets/readme-logo.png" alt="Bitman" width="667">
</h1>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.99%2B-71F7EE?style=flat&amp;logo=rust&amp;logoColor=white" alt="Rust 1.99 or newer">
  <img src="https://img.shields.io/badge/Cargo-1.99.1-71F7EE?style=flat&amp;logo=rust&amp;logoColor=white" alt="Cargo 1.99.1 (verified toolchain)">
  <img src="https://img.shields.io/badge/version-0.1.0-71F7EE?style=flat" alt="Package version 0.1.0">
</p>

A native GPUI image and print workspace rewrite of [Boot Tool](https://github.com/sshhiiff/Boot-Tool).

All credits go to the original author [sshhiiff](https://github.com/sshhiiff) for the original application, its image-to-vector and screen-printing workflows, workspace design, interface layout, and colour palette.

## Why?

The original project does not make full use of native desktop performance.
This rewrite simply implements efficient image processing by leveraging native primitives such as SIMD and efficient memory allocations.
