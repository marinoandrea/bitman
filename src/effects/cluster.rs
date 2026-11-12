//! From-scratch Lloyd/k-means++ and Meta's EMA e-means recipe (3-D Euclidean).
//! Recipe: https://github.com/facebookresearch/faiss/tree/main/contrib/e_means
use super::{Error, MAX_COLORS, color::distance, parallel::Execution};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Algorithm {
    #[default]
    KMeans,
    EMeans,
}

#[derive(Clone, Copy, Debug)]
pub struct ClusterOptions {
    pub k: usize,
    pub algorithm: Algorithm,
    pub seed: u64,
    pub lloyd_iterations: usize,
    pub ema_epochs: usize,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        Self {
            k: 8,
            algorithm: Algorithm::KMeans,
            seed: 0,
            lloyd_iterations: 25,
            ema_epochs: 5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub lab: [[f32; 3]; MAX_COLORS],
    pub len: usize,
    pub mse: f64,
    pub passes: usize,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            lab: [[0.0; 3]; MAX_COLORS],
            len: 0,
            mse: 0.0,
            passes: 0,
        }
    }
}

#[derive(Default)]
pub struct Quantizer {
    labels: Vec<u8>,
    distances: Vec<f32>,
    order: Vec<usize>,
}

#[derive(Clone, Copy)]
pub(crate) struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = self.0;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / ((1u64 << 53) as f64))
    }
}

#[inline]
pub fn nearest(point: [f32; 3], centers: &[[f32; 3]]) -> (usize, f32) {
    let mut best = (0, f32::INFINITY);
    for (i, &center) in centers.iter().enumerate() {
        let d = distance(point, center);
        if d < best.1 {
            best = (i, d);
        }
    }
    best
}

impl Quantizer {
    pub fn reserve(&mut self, n: usize) {
        self.labels.resize(n, 0);
        self.distances.resize(n, 0.0);
        self.order.resize(n, 0);
    }

    pub fn fit(
        &mut self,
        points: &[[f32; 3]],
        options: ClusterOptions,
        exec: Execution,
    ) -> Result<Palette, Error> {
        if points.is_empty()
            || options.k == 0
            || options.k > MAX_COLORS
            || options.lloyd_iterations == 0
            || options.ema_epochs == 0
            || points.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(Error::InvalidParameters);
        }
        self.reserve(points.len());
        let mut result = Palette {
            len: options.k.min(points.len()),
            ..Palette::default()
        };
        let mut rng = Rng(options.seed);
        result.lab[0] = points[(rng.next() as usize) % points.len()];
        self.distances.fill(f32::INFINITY);
        // Shared deterministic k-means++ initialization makes the optimizer comparison fair.
        for c in 1..result.len {
            let center = result.lab[c - 1];
            let mut total = 0.0f64;
            for (p, d) in points.iter().zip(&mut self.distances) {
                *d = d.min(distance(*p, center));
                total += *d as f64;
            }
            let mut target = rng.unit() * total;
            let mut chosen = points.len() - 1;
            for (i, &d) in self.distances.iter().enumerate() {
                target -= d as f64;
                if target <= 0.0 {
                    chosen = i;
                    break;
                }
            }
            result.lab[c] = points[chosen];
        }
        match options.algorithm {
            Algorithm::KMeans => self.lloyd(points, options, &mut result, exec),
            Algorithm::EMeans => self.ema(points, options, &mut result),
        }
        result.mse = points
            .iter()
            .map(|&p| nearest(p, &result.lab[..result.len]).1 as f64)
            .sum::<f64>()
            / points.len() as f64;
        Ok(result)
    }

    fn lloyd(
        &mut self,
        points: &[[f32; 3]],
        options: ClusterOptions,
        result: &mut Palette,
        exec: Execution,
    ) {
        for pass in 0..options.lloyd_iterations {
            let centers = &result.lab[..result.len];
            exec.chunks(&mut self.labels, 4, |offset, labels| {
                super::simd::assign(&points[offset..offset + labels.len()], centers, labels);
            });
            let mut sums = [[0.0f64; 3]; MAX_COLORS];
            let mut counts = [0usize; MAX_COLORS];
            for (&p, &label) in points.iter().zip(&self.labels) {
                let c = label as usize;
                counts[c] += 1;
                for (s, v) in sums[c].iter_mut().zip(p) {
                    *s += v as f64;
                }
            }
            let mut movement = 0.0f32;
            for c in 0..result.len {
                let next = if counts[c] > 0 {
                    sums[c].map(|s| (s / counts[c] as f64) as f32)
                } else {
                    // Farthest-point recovery also handles constant images without NaNs.
                    points
                        .iter()
                        .copied()
                        .max_by(|a, b| {
                            nearest(*a, centers_for(result))
                                .1
                                .total_cmp(&nearest(*b, centers_for(result)).1)
                        })
                        .unwrap()
                };
                movement = movement.max(distance(next, result.lab[c]));
                result.lab[c] = next;
            }
            result.passes = pass + 1;
            if movement <= 1e-6 {
                break;
            }
        }
    }

    fn ema(&mut self, points: &[[f32; 3]], options: ClusterOptions, result: &mut Palette) {
        let n = points.len();
        let k = result.len;
        let epochs = options.ema_epochs;
        let (batch, alpha0) = ema_recipe(n, k, epochs);
        let total_steps = n.div_ceil(batch) * epochs;
        let mut usage = [2.0f64.powi(-16); MAX_COLORS];
        let mut step = 0;
        let mut split_rng = Rng(options.seed ^ 0x5f3759df);
        let mut closing = [[0.0f64; 3]; MAX_COLORS];
        let mut closing_counts = [0usize; MAX_COLORS];
        for epoch in 0..epochs {
            for (i, v) in self.order.iter_mut().enumerate() {
                *v = i;
            }
            let mut rng = Rng(options
                .seed
                .wrapping_mul(1_000_003)
                .wrapping_add(epoch as u64));
            for i in (1..n).rev() {
                let j = (rng.next() as usize) % (i + 1);
                self.order.swap(i, j);
            }
            for order in self.order.chunks(batch) {
                let mut sums = [[0.0f64; 3]; MAX_COLORS];
                let mut counts = [0usize; MAX_COLORS];
                super::simd::assign_indexed(
                    points,
                    order,
                    &result.lab[..k],
                    &mut self.labels[..order.len()],
                );
                for (&i, &label) in order.iter().zip(&self.labels) {
                    let p = points[i];
                    let c = label as usize;
                    counts[c] += 1;
                    for d in 0..3 {
                        sums[c][d] += p[d] as f64;
                    }
                }
                if epoch + 1 == epochs {
                    for c in 0..k {
                        closing_counts[c] += counts[c];
                        for d in 0..3 {
                            closing[c][d] += sums[c][d];
                        }
                    }
                }
                let alpha = alpha0
                    * (0.1
                        + 0.9
                            * 0.5
                            * (1.0
                                + (std::f64::consts::PI * step as f64
                                    / total_steps.max(1) as f64)
                                    .cos()));
                for c in 0..k {
                    let old = (1.0 - alpha) * usage[c];
                    let mass = old + alpha * counts[c] as f64;
                    if mass > 0.0 {
                        for (value, &sum) in result.lab[c].iter_mut().zip(&sums[c]) {
                            *value = ((old * *value as f64 + alpha * sum) / mass) as f32;
                        }
                    }
                    usage[c] = mass;
                }
                if (step + 1) % 5 == 0 {
                    let threshold = 0.1 * usage[..k].iter().sum::<f64>() / k as f64;
                    // Freeze eligibility and donor weights before any split, as in Meta's recipe.
                    let weights = usage;
                    let unused = weights.map(|mass| mass < threshold);
                    let mut used = [false; MAX_COLORS];
                    for target in 0..k {
                        if !unused[target] {
                            continue;
                        }
                        let total = (0..k)
                            .filter(|&c| !used[c] && !unused[c] && weights[c] > 0.0)
                            .map(|c| weights[c])
                            .sum::<f64>();
                        if total <= 0.0 {
                            break;
                        }
                        let mut sample = split_rng.unit() * total;
                        let mut donor = 0;
                        for c in 0..k {
                            if !used[c] && !unused[c] && weights[c] > 0.0 {
                                donor = c;
                                sample -= weights[c];
                                if sample <= 0.0 {
                                    break;
                                }
                            }
                        }
                        used[donor] = true;
                        usage[donor] *= 0.5;
                        usage[target] = usage[donor];
                        for d in 0..3 {
                            let v = result.lab[donor][d];
                            let delta = if d % 2 == 0 { 1e-4 } else { -1e-4 };
                            result.lab[donor][d] = v * (1.0 - delta);
                            result.lab[target][d] = v * (1.0 + delta);
                        }
                        if result.lab[target] == result.lab[donor] {
                            result.lab[target][0] = result.lab[target][0].next_up();
                        }
                    }
                }
                step += 1;
            }
        }
        for c in 0..k {
            if closing_counts[c] > 0 {
                result.lab[c] = closing[c].map(|s| (s / closing_counts[c] as f64) as f32);
            }
        }
        result.passes = epochs;
    }
}

fn centers_for(result: &Palette) -> &[[f32; 3]] {
    &result.lab[..result.len]
}

/// The shipped low-dimensional (quality) law, cosine schedule and revival defaults.
pub fn ema_recipe(n: usize, k: usize, epochs: usize) -> (usize, f64) {
    let raw = 0.00439175
        * (k as f64).powf(0.134749)
        * (n as f64).powf(0.876473)
        * (epochs as f64).powf(0.813344);
    let low = 2.0f64.powf(raw.max(1.0).log2().floor());
    let rounded = if raw / low < 2.0 * low / raw.max(f64::MIN_POSITIVE) {
        low
    } else {
        2.0 * low
    };
    let batch = (rounded as usize).min(n / 2).max(1);
    let alpha = (0.8244
        * (k as f64).powf(-0.4617)
        * (batch as f64).powf(0.9003)
        * (n as f64).powf(-0.4498)
        * (epochs as f64).powf(-0.1057))
    .clamp(1e-4, 1.0);
    (batch, alpha)
}
