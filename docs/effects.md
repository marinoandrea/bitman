# Image effects engine

The numerical engine in `src/effects` uses Rust's standard library only. GPUI
remains the sole direct dependency and supplies existing image codecs, SVG
rasterization, native dialogs, and GPU presentation. No models, packages, or
fonts are downloaded by the effects implementation.

## Implemented effects

| Area | Algorithms |
| --- | --- |
| Resize | Area integration for reductions; bicubic or nearest-neighbour enlargement. |
| Automatic mode | Saturation/value filtering and circular hue concentration distinguish monochrome scans from multicolour artwork. |
| Line art | Alpha composition onto white, luminance, border-median polarity, 3×3 median denoise, Otsu/manual/Gaussian adaptive threshold. |
| Paper correction | Reduced-resolution morphological closing and Gaussian background estimate; division with a solid-region floor and retained-ink guard. |
| Cleanup | Connected-component speckle/hole removal, Gaussian mask smoothing. |
| Colour conversion | 5×5 bilateral denoise, sRGB transfer LUT, D65 CIELAB, deterministic stratified sampling of up to 60,000 pixels. |
| Palettes | From-scratch k-means++ initialization and Lloyd iterations; selectable EMA e-means implementation. Fixed sampled inks are also supported. |
| Vector paths | Directed pixel-boundary contours including holes, closed Douglas–Peucker simplification, corner-aware cubic Bézier handles, stacked colour unions. |
| Levels | Black/white point LUT and gamma; histogram-based automatic levels. |
| Separations | Luminance for one ink; algebraic CMYK with adjustable black generation; hard Lab labels for flat spots; Gaussian Lab memberships for photo spots, with border-paper suppression and optional solid-area preservation. |
| Trapping | Earlier/light inks expand under later/dark inks; paper is excluded. |
| Screen halftones | Rotated rank-normalized spot tiles: round, diamond, ellipse, line, square, cross, or a custom 16×16 stamp with a signed distance field. |
| Dithers | Floyd–Steinberg, Atkinson, Stucki, 8×8 Bayer, cached void-filling blue noise, deterministic random thresholds. |
| Serpentine | Cached 256×256 threshold field generated with an in-house radix-2 2D FFT, Gaussian frequency ring, nonlinear iterations, and rank normalization. Mazes, coral, fingerprints, loops, and spots presets. |
| Dark garments | Design-region closing/hole filling, inversion inside an expanded outline, retained chromatic regions, and optional closed/choked white underbase. |
| Print output | Physical dimensions, aspect-preserving placement, page presets/orientation/margins, four/six registration marks, plate labels, and one layered SVG with black films. Dither cells use exact horizontal runs. |

This reproduces the effect families, rather than matching Python/OpenCV output
pixel for pixel. Morphology uses rectangular windows rather than elliptical
kernels. Contours follow pixel boundaries rather than marching squares. The
blue-noise and FFT pattern generators are independent implementations. CMYK is
an algebraic separation, without ICC colour management.

## Ownership and execution

`ImageView` borrows validated, strided BGRA input. If dimensions do not change,
the engine borrows that input directly. Resizing and bilateral filtering write
to separate reusable buffers. `Engine` retains float planes, labels, plate
masks, component queues, quantizer storage, FFT tiles, contour storage, SVG text,
and encoded BMP bytes across requests. Capacity grows on demand; changing image
size, colour count, or pattern can require further allocation.

Nearest-centroid assignment uses explicit NEON on aarch64 and SSE2 on x86_64,
with scalar tails and scalar fallback elsewhere. NEON processes four independent
4-pixel vectors together to hide reduction latency. Tone mapping also uses
NEON/SSE2; BGRA luminance has a NEON kernel. Unsafe code is restricted to checked
SIMD loads/stores. No unsupported CPU feature is assumed.

SIMD runs within each chunk. Small images stay on one thread; large independent
stages use disjoint scoped chunks, aligned to vectors or whole rows, with a
262,144-element grain and at most eight workers. On the measured 1,048,576-pixel
assignment workload this gives four workers. Error diffusion remains ordered;
it cannot be divided into independent tiles without changing the result.
Clustering sums use deterministic serial f64 reductions.

Tests with a counting allocator establish **zero heap allocations after warm-up
in the tested serial pipelines**, including SVG generation, BMP encoding, and
both quantizers. This is not a claim that the whole application allocates
nothing. Scoped thread scheduling, initial capacity growth, image decoding,
SVG rasterization, GPU uploads, and GPUI elements allocate. Separate image planes
also involve necessary writes/copies. A persistent worker pool could remove
thread setup overhead if later end-to-end profiling justifies it.

The editor moves one engine into a background job, borrows pixels from an
`Arc<RenderImage>`, and coalesces pending edits into the latest parameter
snapshot. Stale results cannot replace a newer preview. Plate selection reuses
existing masks. Retired image textures are released from the window cache.

GPUI 0.2.2's `Image::to_image_data` SVG branch returns premultiplied RGBA despite
`RenderImage`'s straight-BGRA contract. The preview adapter corrects channels and
alpha into the engine's existing pixel buffer, then uses a reusable BMP V4
buffer with explicit alpha masks. This currently entails an extra codec decode
at the display boundary; the effect kernels and exported SVG are unaffected.
The preview stays within 1024 pixels per side. Export reruns at up to 4096 pixels
per side; this is a bounded initial implementation, not unlimited-resolution
processing.

## Palette default and e-means experiment

**Lloyd k-means remains the default.** E-means is faster on most benchmark cases,
but fails the image-quality comparison on the actual app icon. At 16 colours,
median Lab squared error is 0.4445 for Lloyd and 6.1642 for e-means. The latter
is available explicitly in the Colour/Ink inspector for further experimentation.
See [measured results and reproduction](benchmarks/README.md).

The e-means implementation follows Meta's [Faiss recipe](https://github.com/facebookresearch/faiss/tree/main/contrib/e_means),
with the low-dimensional quality batch law, coupled EMA step law, cosine decay,
cluster revival, and moving-last-epoch closing means. The source of the recipe
constants is [base.py](https://github.com/facebookresearch/faiss/blob/main/contrib/e_means/base.py);
the state update and revival reference is
[e_means.py](https://github.com/facebookresearch/faiss/blob/main/contrib/e_means/e_means.py).
The associated paper is [E-Means: Scaling Laws for Mini-Batch K-Means](https://arxiv.org/abs/2611.09501).

For the comparison, both optimizers share deterministic k-means++ initialization,
rather than e-means' default random initialization. Our accumulators are f64,
coordinates are f32, permutations use a local PRNG, and weighted donor sampling
is sequential without replacement. Thus this is a Rust adaptation of the
Euclidean algorithm, not a bit-for-bit port or a benchmark of Faiss itself.
The epoch budget is five and Lloyd's limit is 25 iterations. Final MSE is always
recomputed against the final centres, rather than taken from moving assignments.

## Verification and examples

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked --example effect_gallery -- /tmp/bitman-effect-gallery
cargo run --locked -- --open /path/to/image.png
./scripts/benchmark-effects.sh > /tmp/bitman-bench.csv
```

The gallery writes BMP previews and SVG output for 15 representative pipelines,
without requiring a codec dependency in the example. Tests cover independent
scalar references, component topology, tone coverage, serial/threaded parity,
deterministic palettes, invalid input, and allocation reuse. The generated SVGs
were also rasterized independently with the already-installed `rsvg-convert`.

Convert and Print now process images. Compose, Simulate, Batch, the library,
and their editing/queue tools remain blueprint UI; this change does not
implement those separate workflows. Print preview shows artwork/individual ink
masks; physical page placement and registration marks are in exported SVG.
