//! Standalone std-only harness: compile with scripts/benchmark-effects.sh.
#![allow(dead_code)]
#[path = "../src/effects/mod.rs"]
mod effects;
use effects::{
    cluster::{Algorithm, ClusterOptions, Quantizer, nearest},
    parallel::Execution,
    simd,
};
use std::{hint::black_box, time::Instant};

fn rng(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 40) as u32) as f32 / 16777216.0
}
fn dataset(name: &str, n: usize) -> Vec<[f32; 3]> {
    let mut state = 9182;
    (0..n)
        .map(|i| {
            let x = (i % 256) as f32 / 255.0;
            let y = (i / 256 % 256) as f32 / 255.0;
            match name {
                "flat" => {
                    let c = (i / 4096) % 4;
                    let base = [
                        [90.0, 0.0, 0.0],
                        [35.0, 55.0, 30.0],
                        [50.0, -35.0, 25.0],
                        [20.0, 20.0, -60.0],
                    ][c];
                    base.map(|v| v + (rng(&mut state) - 0.5) * 0.8)
                }
                "gradient" => [x * 100.0, (y - 0.5) * 160.0, ((x + y) * 3.0).sin() * 65.0],
                "clustered" => {
                    let c = (i % 12) as f32;
                    [
                        c / 12.0 * 80.0 + 10.0 + (rng(&mut state) - 0.5) * 10.0,
                        (c * 2.0).sin() * 60.0 + (rng(&mut state) - 0.5) * 20.0,
                        (c * 3.0).cos() * 65.0 + (rng(&mut state) - 0.5) * 20.0,
                    ]
                }
                _ => [
                    rng(&mut state) * 100.0,
                    (rng(&mut state) - 0.5) * 180.0,
                    (rng(&mut state) - 0.5) * 180.0,
                ],
            }
        })
        .collect()
}

fn main() {
    let quick = std::env::args().any(|s| s == "--quick");
    println!("kind,dataset,n,k,seed,algorithm,repeat,threads,backend,milliseconds,mse,passes");
    let n = if quick { 16384 } else { 65536 };
    let repeats = if quick { 1 } else { 3 };
    let seeds = if quick { 1 } else { 3 };
    let mut data: Vec<(&str, Vec<[f32; 3]>)> = ["flat", "gradient", "clustered", "noise"]
        .into_iter()
        .map(|name| (name, dataset(name, n)))
        .collect();
    // Optional real RGB image bytes, decoded outside the timed harness.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|s| s == "--rgb") {
        let bytes = std::fs::read(&args[pos + 1]).expect("raw RGB input");
        assert_eq!(bytes.len() % 3, 0);
        let pixels = bytes.len() / 3;
        let count = pixels.min(60000);
        let points = (0..count)
            .map(|i| {
                let j = i * pixels / count * 3;
                effects::color::rgb_to_lab([bytes[j], bytes[j + 1], bytes[j + 2]])
            })
            .collect();
        data.push(("app-icon", points));
    }
    for (name, points) in data {
        let n = points.len();
        let mut quant = Quantizer::default();
        quant.reserve(n);
        for k in [4, 8, 16] {
            for seed in 0..seeds {
                let algorithms = if seed % 2 == 0 {
                    [Algorithm::KMeans, Algorithm::EMeans]
                } else {
                    [Algorithm::EMeans, Algorithm::KMeans]
                };
                for algorithm in algorithms {
                    let options = ClusterOptions {
                        k,
                        algorithm,
                        seed,
                        ..ClusterOptions::default()
                    };
                    quant.fit(&points, options, Execution::SERIAL).unwrap();
                    for repeat in 0..repeats {
                        let start = Instant::now();
                        let result = quant
                            .fit(black_box(&points), options, Execution::SERIAL)
                            .unwrap();
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        println!(
                            "cluster,{name},{n},{k},{seed},{algorithm:?},{repeat},1,{},{ms:.6},{:.9},{}",
                            simd::backend(),
                            result.mse,
                            result.passes
                        );
                    }
                }
            }
        }
    }
    for n in [65536, 1_048_576] {
        let points = dataset("noise", n);
        let centers = &points[..16];
        let mut labels = vec![0u8; n];
        for algorithm in ["scalar", "simd", "simd+chunks"] {
            for repeat in 0..5 {
                let exec = if algorithm == "simd+chunks" {
                    Execution::default()
                } else {
                    Execution::SERIAL
                };
                let start = Instant::now();
                if algorithm == "scalar" {
                    for (&p, target) in points.iter().zip(&mut labels) {
                        *target = nearest(p, centers).0 as u8;
                    }
                } else {
                    exec.chunks(&mut labels, 4, |offset, out| {
                        simd::assign(&points[offset..offset + out.len()], centers, out)
                    });
                }
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                black_box(&labels);
                println!(
                    "assignment,noise,{n},16,0,{algorithm},{repeat},{},{},{ms:.6},0,0",
                    exec.worker_count(n, 4),
                    simd::backend()
                );
            }
        }
    }
}
