use bitman::effects::{
    cluster::{Algorithm, ClusterOptions, Quantizer, nearest},
    parallel::Execution,
    *,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountingAllocator;
thread_local! {static TRACK:Cell<bool>=const {Cell::new(false)};static ALLOCS:Cell<usize>=const {Cell::new(0)};}
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|t| {
            if t.get() {
                ALLOCS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK.with(|t| {
            if t.get() {
                ALLOCS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocation_count(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    TRACK.with(|t| t.set(true));
    f();
    TRACK.with(|t| t.set(false));
    ALLOCS.with(Cell::get)
}
fn picture(w: usize, h: usize) -> Vec<[u8; 4]> {
    (0..w * h)
        .map(|i| {
            let x = i % w;
            let y = i / w;
            if x < w / 4 || y < h / 5 {
                [255; 4]
            } else if x < w / 2 {
                [10, 30, 200, 255]
            } else if y < h / 2 {
                [200, 70, 20, 255]
            } else {
                let v = (x * 255 / w) as u8;
                [v, v, v, 255]
            }
        })
        .collect()
}
fn view(p: &[[u8; 4]], w: usize, h: usize) -> ImageView<'_> {
    ImageView::new(p.as_flattened(), w, h, w * 4).unwrap()
}

#[test]
fn image_validation_stride_and_alpha() {
    assert!(ImageView::new(&[], 0, 1, 0).is_err());
    assert!(ImageView::new(&[0; 8], 2, 2, 8).is_err());
    assert!(ImageView::new(&[0; 8], 2, 1, 4).is_err());
    let bytes = [0, 0, 0, 0, 0, 0, 255, 128, 11, 22, 33, 44];
    let image = ImageView::new(&bytes, 2, 1, 12).unwrap();
    assert_eq!(image.rgb(0), [255; 3]);
    assert_eq!(image.rgb(1), [255, 127, 127]);
}

#[test]
fn lab_roundtrip_primaries_and_greys() {
    for rgb in [
        [0; 3],
        [255; 3],
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [128; 3],
        [12, 42, 215],
    ] {
        let out = color::lab_to_rgb(color::rgb_to_lab(rgb));
        for d in 0..3 {
            assert!(rgb[d].abs_diff(out[d]) <= 1, "{rgb:?} -> {out:?}");
        }
    }
}

#[test]
fn simd_assignments_match_scalar_including_tails_and_ties() {
    let points: Vec<_> = (0..131)
        .map(|i| {
            [
                (i as f32 * 0.35).sin() * 100.0,
                i as f32 / 3.0,
                (i % 7) as f32,
            ]
        })
        .collect();
    for k in [1, 2, 8, 32] {
        let centers = &points[3..3 + k];
        let mut labels = vec![0; points.len()];
        simd::assign(&points, centers, &mut labels);
        for (p, &label) in points.iter().zip(&labels) {
            assert_eq!(label as usize, nearest(*p, centers).0);
        }
    }
    let mut out = [255; 5];
    simd::assign(&[[1.0; 3]; 5], &[[0.0; 3], [0.0; 3]], &mut out);
    assert_eq!(out, [0; 5]);
}

#[test]
fn simd_grayscale_matches_alpha_composited_scalar() {
    let pixels: Vec<_> = (0..137)
        .map(|i| [(i * 13) as u8, (i * 31) as u8, (i * 53) as u8, i as u8])
        .collect();
    let image = view(&pixels, 137, 1);
    let mut out = vec![0.0; 137];
    filters::grayscale(image, &mut out, Execution::SERIAL);
    for (i, &v) in out.iter().enumerate() {
        assert!((v - color::luma(image.rgb(i))).abs() < 1e-6, "{i}: {v}");
    }
}

#[test]
fn simd_tone_matches_scalar() {
    let mut a: Vec<_> = (0..71).map(|i| i as f32 / 70.0).collect();
    let original = a.clone();
    simd::tone_in_place(&mut a, 1.7, 0.23, true);
    for (i, &v) in a.iter().enumerate() {
        assert!((v - (1.0 - (original[i] * 1.7 - 0.23).clamp(0.0, 1.0))).abs() < 1e-7);
    }
}

#[test]
fn rectangular_morphology_matches_naive_with_non_square_images() {
    for (w, h) in [(1, 7), (7, 1), (9, 5), (4, 3)] {
        let input: Vec<_> = (0..w * h).map(|i| (i * 17 % 23) as f32).collect();
        let mut out = vec![0.0; w * h];
        let mut tmp = out.clone();
        for radius in [0, 1, 2, 15] {
            for dilate in [true, false] {
                filters::morphology(
                    &input,
                    &mut out,
                    &mut tmp,
                    w,
                    h,
                    radius,
                    dilate,
                    Execution {
                        threads: 4,
                        min_chunk: 1,
                    },
                );
                for y in 0..h {
                    for x in 0..w {
                        let mut expected = if dilate {
                            f32::NEG_INFINITY
                        } else {
                            f32::INFINITY
                        };
                        for yy in y.saturating_sub(radius)..=(y + radius).min(h - 1) {
                            for xx in x.saturating_sub(radius)..=(x + radius).min(w - 1) {
                                expected = if dilate {
                                    expected.max(input[yy * w + xx])
                                } else {
                                    expected.min(input[yy * w + xx])
                                };
                            }
                        }
                        assert_eq!(out[y * w + x], expected);
                    }
                }
            }
        }
    }
}

#[test]
fn gaussian_preserves_constant_and_symmetry() {
    let input = vec![0.6; 63];
    let mut out = vec![0.0; 63];
    let mut tmp = out.clone();
    filters::gaussian(&input, &mut out, &mut tmp, 9, 7, 1.3, Execution::SERIAL);
    assert!(out.iter().all(|v| (*v - 0.6).abs() < 1e-6));
}

#[test]
fn kmeans_and_emeans_recover_separated_clusters_without_nans() {
    let points: Vec<_> = (0..2000)
        .map(|i| {
            let c = if i % 2 == 0 { 10.0 } else { 80.0 };
            [c + (i % 3) as f32 * 0.01, c, c]
        })
        .collect();
    let mut q = Quantizer::default();
    for algorithm in [Algorithm::KMeans, Algorithm::EMeans] {
        let a = q
            .fit(
                &points,
                ClusterOptions {
                    k: 2,
                    algorithm,
                    ..ClusterOptions::default()
                },
                Execution::SERIAL,
            )
            .unwrap();
        assert!(a.mse < 0.001, "{algorithm:?}: {}", a.mse);
        let b = q
            .fit(
                &points,
                ClusterOptions {
                    k: 2,
                    algorithm,
                    ..ClusterOptions::default()
                },
                Execution::SERIAL,
            )
            .unwrap();
        assert_eq!(a.lab, b.lab);
        let constant = q
            .fit(
                &[[50.0; 3]; 64],
                ClusterOptions {
                    k: 10,
                    algorithm,
                    ..ClusterOptions::default()
                },
                Execution::SERIAL,
            )
            .unwrap();
        assert!(constant.mse.is_finite() && constant.mse < 0.001);
    }
    assert!(
        q.fit(
            &[[f32::NAN; 3]],
            ClusterOptions::default(),
            Execution::SERIAL
        )
        .is_err()
    );
}

#[test]
fn lloyd_loss_does_not_increase_with_more_iterations() {
    let points: Vec<_> = (0..1000)
        .map(|i| {
            [
                (i * 17 % 101) as f32,
                (i * 31 % 89) as f32,
                (i * 71 % 79) as f32,
            ]
        })
        .collect();
    let mut q = Quantizer::default();
    let a = q
        .fit(
            &points,
            ClusterOptions {
                lloyd_iterations: 1,
                ..ClusterOptions::default()
            },
            Execution::SERIAL,
        )
        .unwrap();
    let b = q
        .fit(
            &points,
            ClusterOptions {
                lloyd_iterations: 25,
                ..ClusterOptions::default()
            },
            Execution::SERIAL,
        )
        .unwrap();
    assert!(b.mse <= a.mse + 1e-6);
}

#[test]
fn serial_and_chunked_results_match() {
    let pixels = picture(53, 37);
    let image = view(&pixels, 53, 37);
    let p = Params {
        mode: Mode::Color,
        denoise: false,
        ..Params::default()
    };
    let mut a = Engine::with_execution(Execution::SERIAL);
    let mut b = Engine::with_execution(Execution {
        threads: 4,
        min_chunk: 1,
    });
    a.render(image, p).unwrap();
    b.render(image, p).unwrap();
    assert_eq!(a.pixels, b.pixels);
    assert_eq!(a.labels, b.labels);
}

#[test]
fn lineart_thresholds_and_blank_images() {
    let pixels = picture(64, 48);
    let mut e = Engine::with_execution(Execution::SERIAL);
    let p = Params {
        mode: Mode::Lineart,
        normalize: false,
        denoise: false,
        speckle: 0,
        edge: 0.0,
        threshold: 0.3,
        ..Params::default()
    };
    e.render(view(&pixels, 64, 48), p).unwrap();
    let small = e.masks.iter().map(|&v| v as usize).sum::<usize>();
    e.render(
        view(&pixels, 64, 48),
        Params {
            threshold: 0.8,
            ..p
        },
    )
    .unwrap();
    assert!(e.masks.iter().map(|&v| v as usize).sum::<usize>() > small);
    for color in [[255; 4], [0, 0, 0, 255]] {
        let pixels = vec![color; 64];
        e.render(
            view(&pixels, 8, 8),
            Params {
                mode: Mode::Lineart,
                ..Params::default()
            },
        )
        .unwrap();
        assert!(e.pixels.iter().all(|p| p[0] == 255));
    }
}

#[test]
fn normalization_preserves_large_solid_areas() {
    let mut pixels = vec![[240, 240, 240, 255]; 96 * 64];
    for y in 10..54 {
        for x in 10..86 {
            pixels[y * 96 + x] = [15, 15, 15, 255];
        }
    }
    let mut e = Engine::default();
    e.render(
        view(&pixels, 96, 64),
        Params {
            mode: Mode::Lineart,
            ..Params::default()
        },
    )
    .unwrap();
    assert!(e.masks.iter().map(|&v| v as usize).sum::<usize>() > 2000);
}

#[test]
fn cmyk_and_spot_build_distinct_plates_and_never_print_paper() {
    let pixels = picture(90, 60);
    let mut e = Engine::default();
    for method in [Method::Cmyk, Method::Spot] {
        for photo in [false, true] {
            let p = Params {
                operation: Operation::Print,
                method,
                photo,
                max_side: 128,
                render: Render::Dither,
                dither: Dither::Bayer,
                denoise: false,
                ..Params::default()
            };
            e.render(view(&pixels, 90, 60), p).unwrap();
            assert!(e.info.plates > 0);
            assert_eq!(e.masks.len(), e.info.width * e.info.height * e.info.plates);
            if method == Method::Spot {
                for plate in e.masks.chunks(e.info.width * e.info.height) {
                    assert_eq!(plate[0], 0);
                }
            }
        }
    }
}

#[test]
fn all_dithers_preserve_endpoints_and_approximate_tone() {
    let mut e = Engine::with_execution(Execution::SERIAL);
    for dither in [
        Dither::Floyd,
        Dither::Atkinson,
        Dither::Stucki,
        Dither::Bayer,
        Dither::Blue,
        Dither::Serpentine,
        Dither::Random,
    ] {
        for value in [0u8, 128, 255] {
            let pixels = vec![[value, value, value, 255]; 64 * 64];
            e.render(
                view(&pixels, 64, 64),
                Params {
                    operation: Operation::Print,
                    render: Render::Dither,
                    dither,
                    max_side: 128,
                    grain: 1.0,
                    ..Params::default()
                },
            )
            .unwrap();
            let mean = e.masks.iter().map(|&v| v as f32).sum::<f32>() / e.masks.len() as f32;
            if value == 0 {
                assert_eq!(mean, 1.0);
            } else if value == 255 {
                assert_eq!(mean, 0.0);
            } else {
                assert!((mean - 0.5).abs() < 0.06, "{dither:?}: {mean}");
            }
        }
    }
}

#[test]
fn floyd_diffusion_matches_textbook_with_edge_clipping() {
    let w = 23;
    let h = 17;
    let pixels: Vec<_> = (0..w * h)
        .map(|i| {
            let v = (i * 37 % 256) as u8;
            [v, v, v, 255]
        })
        .collect();
    let p = Params {
        operation: Operation::Print,
        render: Render::Dither,
        dither: Dither::Floyd,
        art_width: w as f32,
        grain: 1.0,
        max_side: 128,
        ..Params::default()
    };
    let mut e = Engine::default();
    e.render(view(&pixels, w, h), p).unwrap();
    assert_eq!(e.info.width, w);
    assert_eq!(e.info.height, h);
    let mut tone: Vec<_> = pixels
        .iter()
        .map(|p| 1.0 - color::luma([p[2], p[1], p[0]]))
        .collect();
    let mut expected = vec![0; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let on = tone[i] >= 0.5;
            expected[i] = u8::from(on);
            let err = tone[i] - f32::from(on);
            for (dx, dy, wt) in [
                (1isize, 0, 7.0 / 16.0),
                (-1, 1, 3.0 / 16.0),
                (0, 1, 5.0 / 16.0),
                (1, 1, 1.0 / 16.0),
            ] {
                let xx = x as isize + dx;
                if xx >= 0 && xx < w as isize && y + dy < h {
                    tone[(y + dy) * w + xx as usize] += err * wt;
                }
            }
        }
    }
    assert_eq!(e.masks, expected);
}

#[test]
fn screen_shapes_and_custom_pattern_are_real_effects() {
    let pixels = vec![[128, 128, 128, 255]; 64 * 64];
    let mut e = Engine::default();
    let mut previous = Vec::new();
    for shape in [
        Shape::Round,
        Shape::Diamond,
        Shape::Ellipse,
        Shape::Line,
        Shape::Square,
        Shape::Cross,
        Shape::Custom,
    ] {
        e.render(
            view(&pixels, 64, 64),
            Params {
                operation: Operation::Print,
                shape,
                max_side: 192,
                ..Params::default()
            },
        )
        .unwrap();
        let coverage = e.masks.iter().map(|&v| v as f32).sum::<f32>() / e.masks.len() as f32;
        assert!((coverage - 0.5).abs() < 0.08, "{shape:?} {coverage}");
        if !previous.is_empty() {
            assert_ne!(previous, e.masks);
        }
        previous.clone_from(&e.masks);
    }
    assert!(
        e.render(
            view(&pixels, 64, 64),
            Params {
                operation: Operation::Print,
                shape: Shape::Custom,
                stamp: [0; 16],
                ..Params::default()
            }
        )
        .is_err()
    );
}

#[test]
fn svg_tracing_preserves_a_hole_and_declared_size() {
    let mut pixels = vec![[255; 4]; 16 * 16];
    for y in 2..14 {
        for x in 2..14 {
            if !(5..11).contains(&x) || !(5..11).contains(&y) {
                pixels[y * 16 + x] = [0, 0, 0, 255];
            }
        }
    }
    let p = Params {
        mode: Mode::Lineart,
        edge: 0.0,
        speckle: 0,
        denoise: false,
        normalize: false,
        epsilon: 0.01,
        ..Params::default()
    };
    let mut e = Engine::default();
    e.render(view(&pixels, 16, 16), p).unwrap();
    e.write_svg(p, true);
    assert_eq!(e.info.paths, 2);
    assert!(e.svg.contains("evenodd"));
    assert!(e.svg.contains("viewBox=\"0 0 16 16\""));
    assert_eq!(e.svg.matches("M").count(), 2);
}

#[test]
fn warm_serial_pipeline_and_quantizer_allocate_nothing() {
    let pixels = picture(64, 48);
    let image = view(&pixels, 64, 48);
    let mut e = Engine::with_execution(Execution::SERIAL);
    for p in [
        Params {
            mode: Mode::Lineart,
            ..Params::default()
        },
        Params {
            mode: Mode::Color,
            ..Params::default()
        },
        Params {
            operation: Operation::Print,
            method: Method::Spot,
            photo: true,
            keep_solid: true,
            max_side: 128,
            ..Params::default()
        },
        Params {
            operation: Operation::Print,
            render: Render::Dither,
            max_side: 128,
            ..Params::default()
        },
    ] {
        e.render(image, p).unwrap();
        e.write_svg(p, true);
        e.encode_bmp();
        let allocations = allocation_count(|| {
            e.render(image, p).unwrap();
            e.write_svg(p, true);
            e.encode_bmp();
        });
        assert_eq!(allocations, 0, "{p:?}");
    }
}

#[test]
fn shrinking_pixel_style_uses_area_filter_and_keeps_average() {
    let pixels = [[0, 0, 0, 255], [255; 4], [255; 4], [0, 0, 0, 255]];
    let mut out = [[0; 4]; 1];
    filters::resize(view(&pixels, 2, 2), &mut out, 1, 1, true, Execution::SERIAL);
    assert_eq!(out[0], [128, 128, 128, 255]);
}

#[test]
fn grayscale_rejects_out_of_range_before_intrinsic_loads() {
    let pixels = [[255; 4]; 8];
    let image = view(&pixels, 8, 1);
    assert!(std::panic::catch_unwind(|| simd::grayscale(image, &mut [0.0; 8], 1)).is_err());
    let mut out = [0.0; 5];
    simd::grayscale(image, &mut out, 3);
    assert!(out.iter().all(|&v| (v - 1.0).abs() < 1e-6));
}

#[test]
fn flat_dark_inversion_preserves_colour_and_adds_white_outline() {
    let (w, h) = (80, 64);
    let mut pixels = vec![[255; 4]; w * h];
    for y in 15..49 {
        for x in 20..60 {
            pixels[y * w + x] = if x < 40 {
                [0, 0, 0, 255]
            } else {
                [30, 30, 220, 255]
            };
        }
    }
    let p = Params {
        operation: Operation::Print,
        method: Method::Spot,
        dark: true,
        dark_invert: true,
        inks: 3,
        outline_mm: 10.0,
        denoise: false,
        speckle: 0,
        edge: 0.0,
        trap: 0.0,
        ..Params::default()
    };
    let mut engine = Engine::with_execution(Execution::SERIAL);
    engine.render(view(&pixels, w, h), p).unwrap();
    let n = w * h;
    assert!(
        engine.info.palette[..engine.info.plates]
            .iter()
            .any(|c| c.iter().all(|&v| v > 240))
    );
    assert!(
        engine.info.palette[..engine.info.plates]
            .iter()
            .any(|c| c[0] > 180 && c[1] < 80)
    );
    assert!((0..engine.info.plates).all(|k| engine.masks[k * n] == 0));
    assert!(
        (0..engine.info.plates).all(|k| engine.masks[k * n + 30 * w + 25] == 0),
        "dark interior should become garment"
    );
    let mut under = p;
    under.underbase = true;
    engine.render(view(&pixels, w, h), under).unwrap();
    assert_eq!(engine.info.palette[0], [255; 3]);
    assert!(engine.masks[..n].iter().any(|&v| v != 0));
}

#[test]
fn both_clustering_algorithms_reuse_allocations_and_reject_nan() {
    let points: Vec<_> = (0..2048)
        .map(|i| [(i % 113) as f32, (i % 79) as f32, (i % 37) as f32])
        .collect();
    let mut quant = Quantizer::default();
    for algorithm in [Algorithm::KMeans, Algorithm::EMeans] {
        let options = ClusterOptions {
            k: 8,
            algorithm,
            ..ClusterOptions::default()
        };
        quant.fit(&points, options, Execution::SERIAL).unwrap();
        assert_eq!(
            allocation_count(|| {
                quant.fit(&points, options, Execution::SERIAL).unwrap();
            }),
            0
        );
    }
    assert!(
        quant
            .fit(
                &[[f32::NAN, 0.0, 0.0]],
                ClusterOptions::default(),
                Execution::SERIAL
            )
            .is_err()
    );
}

#[test]
fn premultiplied_svg_colours_and_bmp_alpha_are_preserved() {
    assert_eq!(
        color::premultiplied_rgba_to_bgra([220, 45, 55, 255]),
        [55, 45, 220, 255]
    );
    assert_eq!(
        color::premultiplied_rgba_to_bgra([128, 0, 0, 128]),
        [0, 0, 255, 128]
    );
    assert_eq!(color::premultiplied_rgba_to_bgra([0, 0, 0, 0]), [0; 4]);
    let mut e = Engine::default();
    e.render(view(&[[255; 4]; 2], 2, 1), Params::default())
        .unwrap();
    e.pixels[0] = [15, 30, 220, 128];
    e.pixels[1] = [0; 4];
    e.encode_bmp();
    assert_eq!(&e.bmp[10..14], &122u32.to_le_bytes());
    assert_eq!(&e.bmp[66..70], &0xff000000u32.to_le_bytes());
    assert_eq!(&e.bmp[122..], e.pixels.as_flattened());
}

#[test]
fn a_single_white_pixel_is_blank_lineart() {
    let mut engine = Engine::default();
    engine
        .render(view(&[[255; 4]], 1, 1), Params::default())
        .unwrap();
    assert_eq!(engine.masks, [0]);
    assert_eq!(engine.pixels, [[255; 4]]);
}
