use super::{Dither, Pattern, Shape, cluster::Rng, parallel::Execution};

#[derive(Default)]
pub struct PatternCache {
    pub blue: Vec<f32>,
    pub serpentine: Vec<f32>,
    spectrum: Vec<[f32; 2]>,
    ring: Vec<f32>,
    order: Vec<(f32, usize)>,
    serp_key: Option<(Pattern, u32)>,
    screen_key: Option<Shape>,
    stamp: [u16; 16],
    pub screen: Vec<f32>,
}

impl PatternCache {
    pub fn prepare_dither(&mut self, algo: Dither, pattern: Pattern, size: f32) {
        if algo == Dither::Blue && self.blue.is_empty() {
            self.build_blue();
        }
        if algo == Dither::Serpentine && self.serp_key != Some((pattern, size.to_bits())) {
            self.build_serpentine(pattern, size);
            self.serp_key = Some((pattern, size.to_bits()));
        }
    }

    fn build_blue(&mut self) {
        let n = 64usize;
        self.blue.resize(n * n, 0.0);
        let mut energy = [0.0f32; 4096];
        let mut placed = [false; 4096];
        let mut kernel = [0.0f32; 4096];
        let mut rng = Rng(7);
        for i in 0..n * n {
            let dx = (i % n).min(n - i % n) as f32;
            let dy = (i / n).min(n - i / n) as f32;
            kernel[i] = (-(dx * dx + dy * dy) / 4.5).exp();
            energy[i] = rng.unit() as f32 * 1e-3;
        }
        for rank in 0..n * n {
            let idx = (0..n * n)
                .filter(|&i| !placed[i])
                .min_by(|&a, &b| energy[a].total_cmp(&energy[b]))
                .unwrap();
            placed[idx] = true;
            self.blue[idx] = (rank as f32 + 0.5) / (n * n) as f32;
            let (y, x) = (idx / n, idx % n);
            for yy in 0..n {
                for xx in 0..n {
                    energy[yy * n + xx] += kernel[((yy + n - y) % n) * n + (xx + n - x) % n];
                }
            }
        }
    }

    fn build_serpentine(&mut self, pattern: Pattern, size: f32) {
        let n = 256;
        let len = n * n;
        self.spectrum.resize(len, [0.0; 2]);
        self.ring.resize(len, 0.0);
        self.serpentine.resize(len, 0.0);
        self.order.resize(len, (0.0, 0));
        let (band, iterations, gain, stretch, seed) = match pattern {
            Pattern::Mazes => (0.16, 45, 3.0, 1.0, 1),
            Pattern::Coral => (0.45, 25, 2.2, 1.0, 2),
            Pattern::Fingerprints => (0.13, 55, 3.2, 0.5, 3),
            Pattern::Loops => (0.30, 30, 1.6, 1.0, 4),
            Pattern::Spots => (0.20, 10, 1.1, 1.0, 5),
        };
        let mut rng = Rng(seed);
        let k0 = 1.0 / size;
        for i in 0..len {
            let frequency = |v: usize| {
                if v <= n / 2 {
                    v as f32 / n as f32
                } else {
                    (v as f32 - n as f32) / n as f32
                }
            };
            let fx = frequency(i % n);
            let fy = frequency(i / n) * stretch;
            self.ring[i] = (-0.5 * (((fx * fx + fy * fy).sqrt() - k0) / (band * k0)).powi(2)).exp();
            self.serpentine[i] = (-2.0 * rng.unit().max(1e-15).ln()).sqrt() as f32
                * (std::f32::consts::TAU * rng.unit() as f32).cos();
        }
        for iteration in 0..=iterations {
            for (dst, &v) in self.spectrum.iter_mut().zip(&self.serpentine) {
                *dst = [v, 0.0];
            }
            fft2(&mut self.spectrum, n, false);
            for (value, &r) in self.spectrum.iter_mut().zip(&self.ring) {
                value[0] *= r;
                value[1] *= r;
            }
            fft2(&mut self.spectrum, n, true);
            let mean = self.spectrum.iter().map(|v| v[0] as f64).sum::<f64>() / len as f64;
            let std = (self
                .spectrum
                .iter()
                .map(|v| (v[0] as f64 - mean).powi(2))
                .sum::<f64>()
                / len as f64)
                .sqrt() as f32
                + 1e-9;
            for (target, v) in self.serpentine.iter_mut().zip(&self.spectrum) {
                *target = if iteration == iterations {
                    v[0] / std
                } else {
                    (gain * v[0] / std).tanh()
                };
            }
        }
        for (i, v) in self.serpentine.iter().enumerate() {
            self.order[i] = (*v, i);
        }
        self.order
            .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (rank, &(_, i)) in self.order.iter().enumerate() {
            self.serpentine[i] = (rank as f32 + 0.5) / len as f32;
        }
    }

    pub fn prepare_screen(&mut self, shape: Shape, stamp: [u16; 16]) {
        if self.screen_key == Some(shape) && (shape != Shape::Custom || self.stamp == stamp) {
            return;
        }
        self.screen.resize(65536, 0.0);
        self.order.resize(65536, (0.0, 0));
        for i in 0..65536 {
            let u = (i % 256) as f32 / 256.0 - 0.5;
            let v = (i / 256) as f32 / 256.0 - 0.5;
            let value = match shape {
                Shape::Round => {
                    (std::f32::consts::TAU * u).cos() + (std::f32::consts::TAU * v).cos()
                }
                Shape::Ellipse => {
                    (std::f32::consts::TAU * u).cos() + 0.6 * (std::f32::consts::TAU * v).cos()
                }
                Shape::Square => -u.abs().max(v.abs()),
                Shape::Diamond => -u.abs() - v.abs(),
                Shape::Line => -v.abs(),
                Shape::Cross => -u.abs().min(v.abs()),
                Shape::Custom if stamp.iter().all(|&v| v == u16::MAX) => -u.abs().max(v.abs()),
                Shape::Custom => {
                    let x = (i % 256) as f32 / 16.0;
                    let y = (i / 256) as f32 / 16.0;
                    let inside = stamp[y as usize] & (1 << (x as usize)) != 0;
                    let mut dist = f32::INFINITY;
                    for (yy, &row) in stamp.iter().enumerate() {
                        for xx in 0..16 {
                            if (row & (1 << xx) != 0) != inside {
                                dist = dist.min(
                                    (x - xx as f32 - 0.5).powi(2) + (y - yy as f32 - 0.5).powi(2),
                                );
                            }
                        }
                    }
                    if inside { dist.sqrt() } else { -dist.sqrt() }
                }
            };
            self.order[i] = (value, i);
        }
        self.order
            .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        // Equal spot values get the same midpoint rank, preserving shape symmetry.
        let mut start = 0;
        while start < 65536 {
            let mut end = start + 1;
            while end < 65536 && self.order[end].0 == self.order[start].0 {
                end += 1;
            }
            let rank = 1.0 - (start + end) as f32 / (2.0 * 65536.0);
            for j in start..end {
                self.screen[self.order[j].1] = rank;
            }
            start = end;
        }
        self.screen_key = Some(shape);
        self.stamp = stamp;
    }

    pub fn screen_mask(
        &self,
        tone: &[f32],
        mask: &mut [u8],
        w: usize,
        cell: f32,
        angle: f32,
        exec: Execution,
    ) {
        let (sa, ca) = angle.to_radians().sin_cos();
        let h = tone.len() / w;
        exec.chunks(mask, w, |offset, part| {
            for (i, target) in part.iter_mut().enumerate() {
                let j = offset + i;
                let x = (j % w) as f32 - w as f32 / 2.0;
                let y = (j / w) as f32 - h as f32 / 2.0;
                let u = ((x * ca + y * sa) / cell).rem_euclid(1.0);
                let v = ((-x * sa + y * ca) / cell).rem_euclid(1.0);
                let threshold = self.screen
                    [((v * 256.0) as usize).min(255) * 256 + ((u * 256.0) as usize).min(255)];
                *target = u8::from(tone[j] > threshold);
            }
        });
    }

    // Separate borrowed planes keep ownership and buffer reuse explicit.
    #[allow(clippy::too_many_arguments)]
    pub fn dither_mask(
        &self,
        tone: &[f32],
        mask: &mut [u8],
        errors: &mut Vec<f32>,
        w: usize,
        algo: Dither,
        plate: usize,
        exec: Execution,
    ) {
        if matches!(algo, Dither::Floyd | Dither::Atkinson | Dither::Stucki) {
            diffuse(tone, mask, errors, w, algo);
            return;
        }
        exec.chunks(mask, w, |offset, part| {
            for (i, target) in part.iter_mut().enumerate() {
                let j = offset + i;
                let x = j % w;
                let y = j / w;
                let threshold = match algo {
                    Dither::Bayer => {
                        let x = x + plate * 29;
                        let y = y + plate * 17;
                        let mut rank = 0;
                        for b in 0..3 {
                            let xx = (x >> b) & 1;
                            let yy = (y >> b) & 1;
                            let v = match (xx, yy) {
                                (0, 0) => 0,
                                (1, 0) => 2,
                                (0, 1) => 3,
                                _ => 1,
                            };
                            rank += v << (2 * (2 - b));
                        }
                        (rank as f32 + 0.5) / 64.0
                    }
                    Dither::Blue => self.blue[((y + plate * 17) % 64) * 64 + (x + plate * 29) % 64],
                    Dither::Serpentine => {
                        self.serpentine[((y + plate * 61) % 256) * 256 + (x + plate * 97) % 256]
                    }
                    _ => {
                        let mut rng = Rng(j as u64 ^ (plate as u64).wrapping_mul(0x123456789) ^ 11);
                        (rng.unit() as f32).min(1.0f32.next_down())
                    }
                };
                *target = u8::from(tone[j] > threshold);
            }
        });
    }
}

fn diffuse(tone: &[f32], mask: &mut [u8], errors: &mut Vec<f32>, w: usize, algo: Dither) {
    const FLOYD: &[(isize, usize, f32)] = &[
        (1, 0, 7.0 / 16.0),
        (-1, 1, 3.0 / 16.0),
        (0, 1, 5.0 / 16.0),
        (1, 1, 1.0 / 16.0),
    ];
    const ATKINSON: &[(isize, usize, f32)] = &[
        (1, 0, 0.125),
        (2, 0, 0.125),
        (-1, 1, 0.125),
        (0, 1, 0.125),
        (1, 1, 0.125),
        (0, 2, 0.125),
    ];
    const STUCKI: &[(isize, usize, f32)] = &[
        (1, 0, 8.0 / 42.0),
        (2, 0, 4.0 / 42.0),
        (-2, 1, 2.0 / 42.0),
        (-1, 1, 4.0 / 42.0),
        (0, 1, 8.0 / 42.0),
        (1, 1, 4.0 / 42.0),
        (2, 1, 2.0 / 42.0),
        (-2, 2, 1.0 / 42.0),
        (-1, 2, 2.0 / 42.0),
        (0, 2, 4.0 / 42.0),
        (1, 2, 2.0 / 42.0),
        (2, 2, 1.0 / 42.0),
    ];
    let kernel = match algo {
        Dither::Floyd => FLOYD,
        Dither::Atkinson => ATKINSON,
        _ => STUCKI,
    };
    let stride = w + 4;
    errors.resize(stride * 3, 0.0);
    errors.fill(0.0);
    let h = tone.len() / w;
    for y in 0..h.min(3) {
        errors[y * stride + 2..y * stride + 2 + w].copy_from_slice(&tone[y * w..(y + 1) * w]);
    }
    // True error diffusion has ordered dependencies: do not split it into independent tiles.
    for y in 0..h {
        let row = (y % 3) * stride;
        for x in 0..w {
            let v = errors[row + x + 2];
            let on = v >= 0.5;
            mask[y * w + x] = u8::from(on);
            let e = v - f32::from(on);
            for &(dx, dy, wt) in kernel {
                let xx = x as isize + dx;
                if xx >= 0 && xx < w as isize && y + dy < h {
                    errors[((y + dy) % 3) * stride + xx as usize + 2] += e * wt;
                }
            }
        }
        errors[row..row + stride].fill(0.0);
        if y + 3 < h {
            errors[row + 2..row + 2 + w].copy_from_slice(&tone[(y + 3) * w..(y + 4) * w]);
        }
    }
}

fn fft(data: &mut [[f32; 2]], inverse: bool) {
    let n = data.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            data.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = if inverse {
            std::f32::consts::TAU / len as f32
        } else {
            -std::f32::consts::TAU / len as f32
        };
        let (sin, cos) = angle.sin_cos();
        for start in (0..n).step_by(len) {
            let mut wr = 1.0;
            let mut wi = 0.0;
            for k in 0..len / 2 {
                let a = data[start + k];
                let b = data[start + k + len / 2];
                let re = b[0] * wr - b[1] * wi;
                let im = b[0] * wi + b[1] * wr;
                data[start + k] = [a[0] + re, a[1] + im];
                data[start + k + len / 2] = [a[0] - re, a[1] - im];
                let next = wr * cos - wi * sin;
                wi = wr * sin + wi * cos;
                wr = next;
            }
        }
        len *= 2;
    }
    if inverse {
        for v in data {
            v[0] /= n as f32;
            v[1] /= n as f32;
        }
    }
}

fn fft2(data: &mut [[f32; 2]], n: usize, inverse: bool) {
    for row in data.chunks_mut(n) {
        fft(row, inverse);
    }
    let mut column = [[0.0; 2]; 256];
    for x in 0..n {
        for y in 0..n {
            column[y] = data[y * n + x];
        }
        fft(&mut column[..n], inverse);
        for y in 0..n {
            data[y * n + x] = column[y];
        }
    }
}
