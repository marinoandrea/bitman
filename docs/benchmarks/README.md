# Image-processing benchmarks

Recorded 2026-11-12 on an Apple M4 Max (14 logical CPUs), Rust 1.99.1.
The standalone harness uses only std and compiles the actual effect modules with
`-C opt-level=3 -C target-cpu=native`. It does not rebuild or benchmark GPUI.
[Raw measurements](clustering-m4-max.csv) are included.

## Clustering

Each case uses three seeds, a warm-up, and three timed repetitions per optimizer.
Algorithm order alternates with seed. Timings include common k-means++
initialization, training, and fresh final-centroid MSE evaluation. Both run
serially with SIMD assignment. Lloyd stops on convergence or 25 iterations;
e-means uses five epochs. Values below are medians across the nine observations.
MSE is squared Euclidean distance in D65 CIELAB on the training sample, not RGB
error, held-out error, or a perceptual/user-study score.

The synthetic sets each contain 65,536 three-dimensional points: four almost-flat
colours, a continuous gradient, twelve noisy clusters, and uniform noise. The
real-image check samples 60,000 pixels from the checked-in app icon, composited
on white. It exposes behaviour hidden by the synthetic sets. This is a small
local benchmark, not evidence of a universal algorithm ranking.

| Dataset | Colours | Lloyd ms | e-means ms | Lloyd MSE | e-means MSE |
| --- | ---: | ---: | ---: | ---: | ---: |
| flat | 4 | 1.386 | 2.413 | 0.1596 | 0.1596 |
| flat | 8 | 9.630 | 3.110 | 0.1236 | 0.1236 |
| flat | 16 | 13.665 | 5.095 | 0.0800 | 0.0801 |
| gradient | 4 | 10.732 | 2.341 | 1149.5155 | 1149.5247 |
| gradient | 8 | 13.200 | 3.060 | 603.0742 | 602.6018 |
| gradient | 16 | 18.010 | 4.738 | 300.4869 | 298.9332 |
| clustered | 4 | 1.600 | 2.372 | 1074.2805 | 1074.2805 |
| clustered | 8 | 3.254 | 3.087 | 317.7012 | 317.7013 |
| clustered | 16 | 11.457 | 4.799 | 67.2858 | 67.2782 |
| noise | 4 | 6.013 | 2.342 | 2182.2305 | 2182.2391 |
| noise | 8 | 7.578 | 3.089 | 1550.0720 | 1527.0741 |
| noise | 16 | 12.037 | 4.794 | 889.0054 | 884.9475 |
| app icon | 4 | 3.817 | 2.277 | 3.5311 | 8.8825 |
| app icon | 8 | 4.740 | 3.017 | 1.2316 | 7.0435 |
| app icon | 16 | 8.140 | 4.644 | 0.4445 | 6.1642 |

Decision: use **Lloyd by default**. E-means often saves training time, but its
median MSE on this real image is 2.5–13.9 times Lloyd's. A faster palette with
that loss does not meet comparable-quality selection. E-means remains an
explicit option. Whether its low-mass cluster revival explains the icon result
is a hypothesis; this run does not isolate that cause. The implementation
adaptations and upstream sources are described in [effects.md](../effects.md).

## Assignment: intrinsics before threads

16 centres, fixed noise data, five repetitions. Counts are actual workers;
rows with one worker use the same kernel without thread scheduling.

| Pixels | Scalar ms | NEON ms | NEON + chunks ms | Workers |
| ---: | ---: | ---: | ---: | ---: |
| 65,536 | 0.3194 | 0.3077 | 0.3078 | 1 |
| 1,048,576 | 5.0996 | 4.9410 | 1.2927 | 4 |

On this machine the tuned explicit kernel gives only a modest ~3% gain over
optimized scalar Rust. Four chunks then give ~3.8× over one NEON worker (~3.9×
over scalar) at one million pixels. Small images stay serial to avoid thread
setup. These timings cover assignment alone, not the whole effects pipeline.
SSE2 is implemented but was not run on this ARM machine.

## Reproduce

```sh
./scripts/benchmark-effects.sh > /tmp/bitman-benchmark.csv
./scripts/benchmark-effects.sh --quick
```

For the real-image case, give the harness raw, white-composited RGB bytes.
Decoding happens outside the timed harness. If Pillow is already available in a
separate analysis environment, the following prepares the input; Pillow is not
a dependency of Bitman or its benchmark executable:

```python
from PIL import Image
image = Image.open('assets/icons/app-icon.png').convert('RGBA')
background = Image.new('RGBA', image.size, 'white')
background.alpha_composite(image)
open('/tmp/bitman-icon.rgb', 'wb').write(background.convert('RGB').tobytes())
```

```sh
./scripts/benchmark-effects.sh --rgb /tmp/bitman-icon.rgb > /tmp/bitman-benchmark.csv
```

The harness samples at most 60,000 equally spaced input pixels for that case.
It does not install any tool or package. Re-run on target hardware and a broader
image corpus before changing the default or making general performance claims.
