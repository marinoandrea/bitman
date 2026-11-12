//! Borrowed BGRA input → reusable working planes → reusable masks and BGRA output.
//! SIMD kernels run first. Only large, independent slices use scoped worker chunks.
pub mod cluster;
pub mod color;
pub mod filters;
pub mod parallel;
mod pattern;
pub mod simd;
mod vector;

use cluster::{Algorithm, ClusterOptions, Palette, Quantizer};
use parallel::Execution;
use pattern::PatternCache;

pub const MAX_COLORS: usize = 32;
pub const MAX_SIDE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidImage,
    InvalidParameters,
    EmptyPattern,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidImage => "Invalid or oversized image",
            Self::InvalidParameters => "Invalid effect parameters",
            Self::EmptyPattern => "Draw a custom screen pattern first",
        })
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy)]
pub struct ImageView<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) stride: usize,
}
impl<'a> ImageView<'a> {
    pub fn new(bytes: &'a [u8], width: usize, height: usize, stride: usize) -> Result<Self, Error> {
        let row = width.checked_mul(4).ok_or(Error::InvalidImage)?;
        let required = height
            .checked_sub(1)
            .and_then(|h| h.checked_mul(stride))
            .and_then(|v| v.checked_add(row))
            .ok_or(Error::InvalidImage)?;
        if width == 0
            || height == 0
            || stride < row
            || bytes.len() < required
            || width > 65535
            || height > 65535
        {
            return Err(Error::InvalidImage);
        }
        Ok(Self {
            bytes,
            width,
            height,
            stride,
        })
    }
    pub fn len(self) -> usize {
        self.width * self.height
    }
    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
    #[inline]
    pub fn rgb(self, i: usize) -> [u8; 3] {
        let p = (i / self.width) * self.stride + (i % self.width) * 4;
        let a = self.bytes[p + 3] as u32;
        [self.bytes[p + 2], self.bytes[p + 1], self.bytes[p]]
            .map(|v| ((v as u32 * a + 255 * (255 - a) + 127) / 255) as u8)
    }
    pub fn rgb_at(self, x: isize, y: isize) -> [u8; 3] {
        self.rgb(
            y.clamp(0, self.height as isize - 1) as usize * self.width
                + x.clamp(0, self.width as isize - 1) as usize,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Operation {
    #[default]
    Convert,
    Print,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Auto,
    Lineart,
    Color,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Method {
    #[default]
    One,
    Cmyk,
    Spot,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Render {
    #[default]
    Screen,
    Dither,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Round,
    Diamond,
    Ellipse,
    Line,
    Square,
    Cross,
    Custom,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dither {
    #[default]
    Floyd,
    Atkinson,
    Stucki,
    Bayer,
    Blue,
    Serpentine,
    Random,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Mazes,
    Coral,
    Fingerprints,
    Loops,
    Spots,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub operation: Operation,
    pub mode: Mode,
    pub upscale: f32,
    pub pixel: bool,
    pub max_side: usize,
    pub denoise: bool,
    pub normalize: bool,
    pub adaptive: bool,
    pub threshold: f32,
    pub speckle: usize,
    pub edge: f32,
    pub epsilon: f32,
    pub corner: f32,
    pub colors: usize,
    pub algorithm: Algorithm,
    pub ink: [u8; 3],
    pub sample_ink: bool,
    pub method: Method,
    pub photo: bool,
    pub render: Render,
    pub black_amount: f32,
    pub black: f32,
    pub white: f32,
    pub gamma: f32,
    pub contrast: f32,
    pub brightness: f32,
    pub smoothing: f32,
    pub invert: bool,
    pub lpi: f32,
    pub angle: f32,
    pub ppi: f32,
    pub shape: Shape,
    pub stamp: [u16; 16],
    pub dither: Dither,
    pub grain: f32,
    pub pattern: Pattern,
    pub pattern_size: f32,
    pub inks: usize,
    pub paper: bool,
    pub trap: f32,
    pub picked: usize,
    pub ink_colors: [[u8; 3]; MAX_COLORS],
    pub keep_solid: bool,
    pub solid_detect: f32,
    pub dark: bool,
    pub dark_bg: [u8; 3],
    pub dark_invert: bool,
    pub outline_mm: f32,
    pub underbase: bool,
    pub page_width: f32,
    pub page_height: f32,
    pub margin: f32,
    pub art_width: f32,
    pub marks: bool,
    pub six_marks: bool,
    pub ink_names: bool,
}
impl Default for Params {
    fn default() -> Self {
        let mut stamp = [0u16; 16];
        for (y, row) in stamp.iter_mut().enumerate() {
            *row = if (6..=9).contains(&y) {
                0x3ffc
            } else if (2..=13).contains(&y) {
                0x03c0
            } else {
                0
            };
        }
        Self {
            operation: Operation::Convert,
            mode: Mode::Auto,
            upscale: 1.0,
            pixel: false,
            max_side: 1024,
            denoise: true,
            normalize: true,
            adaptive: false,
            threshold: 0.0,
            speckle: 6,
            edge: 0.6,
            epsilon: 0.75,
            corner: 125.0,
            colors: 8,
            algorithm: Algorithm::KMeans,
            ink: [0; 3],
            sample_ink: false,
            method: Method::One,
            photo: false,
            render: Render::Screen,
            black_amount: 0.6,
            black: 0.0,
            white: 1.0,
            gamma: 1.0,
            contrast: 1.0,
            brightness: 0.0,
            smoothing: 0.0,
            invert: false,
            lpi: 30.0,
            angle: 22.5,
            ppi: 300.0,
            shape: Shape::Round,
            stamp,
            dither: Dither::Floyd,
            grain: 0.3,
            pattern: Pattern::Mazes,
            pattern_size: 9.0,
            inks: 4,
            paper: true,
            trap: 0.2,
            picked: 0,
            ink_colors: [[0; 3]; MAX_COLORS],
            keep_solid: false,
            solid_detect: 0.5,
            dark: false,
            dark_bg: [27, 27, 29],
            dark_invert: false,
            outline_mm: 5.0,
            underbase: false,
            page_width: 297.0,
            page_height: 210.0,
            margin: 28.0,
            art_width: 0.0,
            marks: true,
            six_marks: true,
            ink_names: true,
        }
    }
}
impl Params {
    fn valid(self) -> bool {
        let floats = [
            self.upscale,
            self.threshold,
            self.edge,
            self.epsilon,
            self.corner,
            self.black_amount,
            self.black,
            self.white,
            self.gamma,
            self.contrast,
            self.brightness,
            self.smoothing,
            self.lpi,
            self.angle,
            self.ppi,
            self.grain,
            self.pattern_size,
            self.trap,
            self.solid_detect,
            self.outline_mm,
            self.page_width,
            self.page_height,
            self.margin,
            self.art_width,
        ];
        floats.iter().all(|v| v.is_finite())
            && self.max_side > 0
            && self.max_side <= MAX_SIDE
            && self.upscale > 0.0
            && self.upscale <= 8.0
            && (0.0..=1.0).contains(&self.black_amount)
            && self.epsilon >= 0.0
            && (0.0..=180.0).contains(&self.corner)
            && (0.0..=1.0).contains(&self.threshold)
            && self.colors > 0
            && self.colors <= MAX_COLORS
            && (1..=10).contains(&self.inks)
            && self.picked <= 10
            && self.white > self.black
            && self.gamma > 0.0
            && (0.0..=1.0).contains(&self.black)
            && (0.0..=1.0).contains(&self.white)
            && self.edge >= 0.0
            && self.edge <= 8.0
            && self.smoothing >= 0.0
            && self.smoothing <= 8.0
            && self.lpi > 0.0
            && self.ppi > 0.0
            && self.grain >= 0.01
            && self.pattern_size >= 3.0
            && self.pattern_size <= 40.0
            && self.trap >= 0.0
            && self.outline_mm >= 0.0
            && self.page_width > 2.0 * self.margin
            && self.page_height > 2.0 * self.margin
            && self.margin >= 0.0
            && self.art_width >= 0.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Info {
    pub width: usize,
    pub height: usize,
    pub original_width: usize,
    pub original_height: usize,
    pub lineart: bool,
    pub plates: usize,
    pub palette: [[u8; 3]; MAX_COLORS],
    pub palette_len: usize,
    pub paper: Option<usize>,
    pub clustering_mse: f64,
    pub art_mm: [f32; 2],
    pub paths: usize,
    pub nodes: usize,
}

#[derive(Default)]
pub struct Engine {
    pub execution: Execution,
    pub info: Info,
    pub pixels: Vec<[u8; 4]>,
    pub masks: Vec<u8>,
    pub labels: Vec<u8>,
    pub svg: String,
    pub bmp: Vec<u8>,
    resized: Vec<[u8; 4]>,
    filtered: Vec<[u8; 4]>,
    darkened: Vec<[u8; 4]>,
    lab: Vec<[f32; 3]>,
    samples: Vec<[f32; 3]>,
    a: Vec<f32>,
    b: Vec<f32>,
    c: Vec<f32>,
    d: Vec<f32>,
    denom: Vec<f32>,
    range: Vec<f32>,
    small_a: Vec<f32>,
    small_b: Vec<f32>,
    small_c: Vec<f32>,
    visited: Vec<u8>,
    queue: Vec<u32>,
    errors: Vec<f32>,
    quantizer: Quantizer,
    patterns: PatternCache,
    trace: vector::Tracer,
}

impl Engine {
    pub fn with_execution(execution: Execution) -> Self {
        Self {
            execution,
            ..Self::default()
        }
    }
    pub fn render(&mut self, source: ImageView<'_>, p: Params) -> Result<(), Error> {
        if !p.valid() {
            return Err(Error::InvalidParameters);
        }
        if p.operation == Operation::Print
            && p.shape == Shape::Custom
            && p.render == Render::Screen
            && p.stamp.iter().all(|&v| v == 0)
        {
            return Err(Error::EmptyPattern);
        }
        let ratio = source.height as f32 / source.width as f32;
        let mut aw = p.art_width;
        if aw == 0.0 {
            aw = (p.page_width - 2.0 * p.margin).min((p.page_height - 2.0 * p.margin) / ratio);
        }
        let ah = aw * ratio;
        let (mut width, mut height) =
            if p.operation == Operation::Convert || (p.method == Method::Spot && !p.photo) {
                (
                    source.width as f32 * p.upscale,
                    source.height as f32 * p.upscale,
                )
            } else {
                let mm = if p.render == Render::Screen {
                    25.4 / p.ppi
                } else {
                    p.grain
                };
                (aw / mm, ah / mm)
            };
        let scale = (p.max_side as f32 / width.max(height)).min(1.0);
        width = (width * scale).round().max(1.0);
        height = (height * scale).round().max(1.0);
        let (w, h) = (width as usize, height as usize);
        let n = w * h;
        self.info = Info {
            width: w,
            height: h,
            original_width: source.width,
            original_height: source.height,
            art_mm: [aw, ah],
            ..Info::default()
        };
        self.pixels.resize(n, [255; 4]);
        self.labels.resize(n, 0);
        self.visited.resize(n, 0);
        self.a.resize(n, 0.0);
        self.b.resize(n, 0.0);
        self.c.resize(n, 0.0);
        self.d.resize(n, 0.0);
        let needs_components = (p.operation == Operation::Convert && p.speckle > 0)
            || (p.operation == Operation::Print
                && p.method == Method::Spot
                && !p.photo
                && p.speckle > 0)
            || (p.operation == Operation::Print && p.dark && p.dark_invert);
        if needs_components && self.queue.capacity() < n {
            self.queue.reserve(n - self.queue.len());
        }
        let work = if (w, h) == (source.width, source.height) {
            source
        } else {
            self.resized.resize(n, [0; 4]);
            filters::resize(source, &mut self.resized, w, h, p.pixel, self.execution);
            ImageView::new(self.resized.as_flattened(), w, h, w * 4)?
        };
        self.info.lineart = p.operation == Operation::Convert
            && (p.mode == Mode::Lineart || (p.mode == Mode::Auto && color::is_lineart(work)));
        if self.info.lineart {
            self.masks.resize(n, 0);
            filters::grayscale(work, &mut self.a, self.execution);
            let mut border = [0usize; 256];
            for i in 0..n {
                if i < w || i >= n - w || i % w == 0 || i % w == w - 1 {
                    border[(self.a[i] * 255.0).round() as usize] += 1;
                }
            }
            let half = border.iter().sum::<usize>().div_ceil(2);
            let mut count = 0;
            let mut median = 0;
            for (i, &v) in border.iter().enumerate() {
                count += v;
                if count >= half {
                    median = i;
                    break;
                }
            }
            if median < 110 {
                for v in &mut self.a {
                    *v = 1.0 - *v;
                }
                median = 255 - median;
            }
            if p.denoise {
                filters::median(&self.a, &mut self.b, w, h, self.execution);
                std::mem::swap(&mut self.a, &mut self.b);
            }
            threshold(
                &self.a,
                &mut self.masks,
                &mut self.b,
                &mut self.c,
                w,
                h,
                p,
                self.execution,
            );
            if p.normalize {
                let sw = w.div_ceil(4);
                let sh = h.div_ceil(4);
                let sn = sw * sh;
                self.small_a.resize(sn, 0.0);
                self.small_b.resize(sn, 0.0);
                self.small_c.resize(sn, 0.0);
                for yy in 0..sh {
                    for xx in 0..sw {
                        let mut sum = 0.0;
                        let mut count = 0;
                        for y in yy * 4..((yy + 1) * 4).min(h) {
                            for x in xx * 4..((xx + 1) * 4).min(w) {
                                sum += self.a[y * w + x];
                                count += 1;
                            }
                        }
                        self.small_a[yy * sw + xx] = sum / count as f32;
                    }
                }
                let r = (sw.max(sh) / 12).max(2);
                filters::morphology(
                    &self.small_a,
                    &mut self.small_b,
                    &mut self.small_c,
                    sw,
                    sh,
                    r,
                    true,
                    self.execution,
                );
                filters::morphology(
                    &self.small_b,
                    &mut self.small_a,
                    &mut self.small_c,
                    sw,
                    sh,
                    r,
                    false,
                    self.execution,
                );
                filters::gaussian(
                    &self.small_a,
                    &mut self.small_b,
                    &mut self.small_c,
                    sw,
                    sh,
                    (r as f32 / 4.0).clamp(1.0, 8.0),
                    self.execution,
                );
                // A locally dark estimate inside a broad solid is ink, not paper.
                let paper_floor = (median as f32 / 255.0 * 0.5).max(1.0 / 255.0);
                for i in 0..n {
                    self.d[i] = (self.a[i]
                        / self.small_b[((i / w) / 4) * sw + (i % w) / 4].max(paper_floor))
                    .min(1.0);
                }
                threshold(
                    &self.d,
                    &mut self.visited,
                    &mut self.b,
                    &mut self.c,
                    w,
                    h,
                    p,
                    self.execution,
                );
                if self.visited.iter().map(|&v| v as usize).sum::<usize>() as f32
                    >= 0.6 * self.masks.iter().map(|&v| v as usize).sum::<usize>() as f32
                {
                    self.masks.copy_from_slice(&self.visited);
                }
            }
            if p.sample_ink {
                let mut sum = [0u64; 3];
                let mut count = 0;
                for i in 0..n {
                    if self.masks[i] != 0 {
                        let rgb = work.rgb(i);
                        for d in 0..3 {
                            sum[d] += rgb[d] as u64;
                        }
                        count += 1;
                    }
                }
                self.info.palette[0] = if count > 0 {
                    sum.map(|s| (s / count) as u8)
                } else {
                    p.ink
                };
            } else {
                self.info.palette[0] = p.ink;
            }
            clean(
                &mut self.masks,
                &mut self.visited,
                &mut self.queue,
                w,
                h,
                p.speckle,
            );
            smooth_mask(
                &mut self.masks,
                &mut self.a,
                &mut self.b,
                &mut self.c,
                w,
                h,
                p.edge,
                self.execution,
            );
            self.info.plates = 1;
            self.info.palette_len = 1;
            for (dst, &mask) in self.pixels.iter_mut().zip(&self.masks) {
                *dst = bgra(if mask != 0 {
                    self.info.palette[0]
                } else {
                    [255; 3]
                });
            }
            return Ok(());
        }
        // Keep image borrows separate from the scratch planes: no hidden source clone.
        let work = if p.denoise
            && (p.operation == Operation::Convert || (p.method == Method::Spot && !p.photo))
        {
            self.filtered.resize(n, [0; 4]);
            filters::bilateral(work, &mut self.filtered, self.execution);
            ImageView::new(self.filtered.as_flattened(), w, h, w * 4)?
        } else {
            work
        };
        let mut lut = [0u8; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = (((i as f32 / 255.0 - p.black) / (p.white - p.black))
                .clamp(0.0, 1.0)
                .powf(1.0 / p.gamma)
                * 255.0)
                .round() as u8;
        }
        let work = if p.operation == Operation::Print
            && p.method == Method::Spot
            && !p.photo
            && p.dark
            && p.dark_invert
        {
            for i in 0..n {
                self.a[i] =
                    f32::from(1.0 - color::luma(work.rgb(i).map(|c| lut[c as usize])) > 0.08);
            }
            let radius = (0.8 * w as f32 / aw).round().max(1.0) as usize;
            filters::morphology(
                &self.a,
                &mut self.b,
                &mut self.c,
                w,
                h,
                radius,
                true,
                self.execution,
            );
            filters::morphology(
                &self.b,
                &mut self.a,
                &mut self.c,
                w,
                h,
                radius,
                false,
                self.execution,
            );
            filters::fill_holes(&mut self.a, &mut self.visited, &mut self.queue, w, h);
            filters::morphology(
                &self.a,
                &mut self.b,
                &mut self.c,
                w,
                h,
                (p.outline_mm * w as f32 / aw).round() as usize,
                true,
                self.execution,
            );
            self.darkened.resize(n, [0; 4]);
            self.execution
                .chunks(&mut self.darkened, 4, |offset, part| {
                    for (j, dst) in part.iter_mut().enumerate() {
                        let i = offset + j;
                        let rgb = work.rgb(i).map(|c| lut[c as usize]);
                        let gray = color::luma(rgb);
                        let chroma = rgb.iter().max().unwrap() - rgb.iter().min().unwrap();
                        let color = if self.b[i] <= 0.5 || (gray < 0.3 && chroma < 60) {
                            p.dark_bg
                        } else if (gray > 0.85 && chroma < 40) || gray > 0.92 {
                            [255; 3]
                        } else {
                            rgb
                        };
                        *dst = bgra(color);
                    }
                });
            for (i, v) in lut.iter_mut().enumerate() {
                *v = i as u8;
            }
            ImageView::new(self.darkened.as_flattened(), w, h, w * 4)?
        } else {
            work
        };
        if p.operation == Operation::Convert || p.method == Method::Spot {
            self.lab.resize(n, [0.0; 3]);
            let mut linear = [0.0f32; 256];
            for (i, v) in linear.iter_mut().enumerate() {
                let x = lut[i] as f32 / 255.0;
                *v = if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                };
            }
            self.execution.chunks(&mut self.lab, 4, |offset, part| {
                for (j, v) in part.iter_mut().enumerate() {
                    let rgb = work.rgb(offset + j);
                    *v = color::rgb_linear_to_lab(rgb.map(|c| linear[c as usize]));
                }
            });
            let paper_rgb = border_color(work).map(|c| lut[c as usize]);
            let paper_lab = color::rgb_to_lab(paper_rgb);
            let k = if p.operation == Operation::Convert {
                p.colors
            } else {
                p.inks + usize::from(p.paper && !p.photo)
            };
            let mut palette = Palette::default();
            if p.operation == Operation::Print && p.picked > 0 {
                palette.len = p.picked;
                for i in 0..p.picked {
                    palette.lab[i] = color::rgb_to_lab(p.ink_colors[i]);
                }
            } else {
                self.samples.clear();
                let sample_count = n.min(60000);
                if self.samples.capacity() < sample_count {
                    self.samples.reserve(sample_count - self.samples.len());
                }
                let mut rng = cluster::Rng(0);
                for i in 0..sample_count {
                    let lo = i * n / sample_count;
                    let hi = ((i + 1) * n / sample_count).max(lo + 1);
                    let j = lo + (rng.next() as usize) % (hi - lo);
                    let lab = self.lab[j];
                    if !(p.operation == Operation::Print
                        && p.photo
                        && p.paper
                        && color::distance(lab, paper_lab) < 64.0)
                    {
                        self.samples.push(lab);
                    }
                }
                if self.samples.is_empty() {
                    self.samples.push(self.lab[0]);
                }
                palette = self.quantizer.fit(
                    &self.samples,
                    ClusterOptions {
                        k: k.min(MAX_COLORS),
                        algorithm: p.algorithm,
                        ..ClusterOptions::default()
                    },
                    self.execution,
                )?;
                if p.operation == Operation::Print {
                    palette.lab[..palette.len].sort_unstable_by(|a, b| b[0].total_cmp(&a[0]));
                }
            }
            if p.operation == Operation::Print && p.paper && p.picked > 0 && !p.photo {
                palette.lab[palette.len] = paper_lab;
                palette.len += 1;
            }
            self.info.palette_len = palette.len;
            self.info.clustering_mse = palette.mse;
            for i in 0..palette.len {
                self.info.palette[i] = color::lab_to_rgb(palette.lab[i]);
            }
            if p.picked > 0 {
                self.info.palette[..p.picked].copy_from_slice(&p.ink_colors[..p.picked]);
            }
            self.execution
                .chunks(&mut self.labels, 4, |offset, labels| {
                    simd::assign(
                        &self.lab[offset..offset + labels.len()],
                        &palette.lab[..palette.len],
                        labels,
                    )
                });
            if p.operation == Operation::Convert {
                self.masks.resize(n, 0);
                self.info.plates = palette.len;
                self.execution.chunks(&mut self.pixels, 4, |offset, part| {
                    for (j, dst) in part.iter_mut().enumerate() {
                        *dst = bgra(self.info.palette[self.labels[offset + j] as usize]);
                    }
                });
                return Ok(());
            }
            if p.paper && !p.photo {
                let mut counts = [0usize; MAX_COLORS];
                for i in 0..n {
                    if i < w || i >= n - w || i % w == 0 || i % w == w - 1 {
                        counts[self.labels[i] as usize] += 1;
                    }
                }
                self.info.paper = Some(if p.picked > 0 {
                    palette.len - 1
                } else {
                    (0..palette.len).max_by_key(|&i| counts[i]).unwrap()
                });
            }
            self.info.plates = palette.len - usize::from(self.info.paper.is_some());
            self.masks.resize(n * self.info.plates, 0);
            if p.photo {
                let mut sigma = 20.0;
                if palette.len > 1 {
                    sigma = (0..palette.len)
                        .map(|i| {
                            (0..palette.len)
                                .filter(|&j| j != i)
                                .map(|j| color::distance(palette.lab[i], palette.lab[j]).sqrt())
                                .fold(f32::INFINITY, f32::min)
                        })
                        .sum::<f32>()
                        / palette.len as f32
                        * 0.55;
                    sigma = sigma.max(6.0);
                }
                self.denom.resize(n, 0.0);
                let variance = 2.0 * sigma * sigma;
                self.execution.chunks(&mut self.denom, 4, |offset, part| {
                    for (j, v) in part.iter_mut().enumerate() {
                        let lab = self.lab[offset + j];
                        *v = palette.lab[..palette.len]
                            .iter()
                            .map(|&c| (-color::distance(lab, c) / variance).exp() + 1e-6)
                            .sum::<f32>()
                            + if p.paper {
                                (-color::distance(lab, paper_lab) / variance).exp() + 1e-6
                            } else {
                                0.0
                            };
                    }
                });
                if p.keep_solid {
                    self.range.resize(n, 0.0);
                    self.range.fill(0.0);
                    for dim in 0..3 {
                        for i in 0..n {
                            self.a[i] = self.lab[i][dim];
                        }
                        filters::morphology(
                            &self.a,
                            &mut self.b,
                            &mut self.c,
                            w,
                            h,
                            4,
                            true,
                            self.execution,
                        );
                        filters::morphology(
                            &self.a,
                            &mut self.d,
                            &mut self.c,
                            w,
                            h,
                            4,
                            false,
                            self.execution,
                        );
                        for i in 0..n {
                            self.range[i] += self.b[i] - self.d[i];
                        }
                    }
                }
                prepare_pattern(&mut self.patterns, p);
                for plate in 0..palette.len {
                    self.execution.chunks(&mut self.a, 4, |offset, part| {
                        for (j, v) in part.iter_mut().enumerate() {
                            let i = offset + j;
                            let lab = self.lab[i];
                            let paper_fade = if p.paper {
                                let t = ((color::distance(lab, paper_lab).sqrt() - 3.0) / 6.0)
                                    .clamp(0.0, 1.0);
                                t * t * (3.0 - 2.0 * t)
                            } else {
                                1.0
                            };
                            *v = ((-color::distance(lab, palette.lab[plate]) / variance).exp()
                                + 1e-6)
                                / self.denom[i]
                                * paper_fade;
                        }
                    });
                    render_plane(
                        &mut self.a,
                        &mut self.b,
                        &mut self.c,
                        &mut self.masks[plate * n..(plate + 1) * n],
                        &mut self.errors,
                        &self.patterns,
                        w,
                        h,
                        plate,
                        p,
                        aw,
                        self.execution,
                    );
                    if p.keep_solid {
                        for i in 0..n {
                            let winner = self.labels[i] as usize;
                            if self.range[i] < 4.0 + 20.0 * p.solid_detect
                                && color::distance(self.lab[i], palette.lab[winner]).sqrt()
                                    < 12.0 + 20.0 * p.solid_detect
                                && (!p.paper || color::distance(self.lab[i], paper_lab) > 64.0)
                            {
                                self.masks[plate * n + i] = u8::from(winner == plate);
                            }
                        }
                    }
                }
            } else {
                let mut print_colors = [[0u8; 3]; MAX_COLORS];
                let mut plate = 0;
                for label in 0..palette.len {
                    if Some(label) == self.info.paper {
                        continue;
                    }
                    print_colors[plate] = self.info.palette[label];
                    for i in 0..n {
                        self.a[i] = f32::from(self.labels[i] as usize == label);
                    }
                    let radius = (p.trap * w as f32 / aw).round() as usize;
                    if radius > 0 && plate + 1 < self.info.plates {
                        filters::morphology(
                            &self.a,
                            &mut self.b,
                            &mut self.c,
                            w,
                            h,
                            radius,
                            true,
                            self.execution,
                        );
                        for i in 0..n {
                            let l = self.labels[i] as usize;
                            if l > label && Some(l) != self.info.paper {
                                self.a[i] = self.a[i].max(self.b[i]);
                            }
                        }
                    }
                    let mask = &mut self.masks[plate * n..(plate + 1) * n];
                    for (v, &a) in mask.iter_mut().zip(&self.a) {
                        *v = u8::from(a > 0.5);
                    }
                    clean(mask, &mut self.visited, &mut self.queue, w, h, p.speckle);
                    smooth_mask(
                        mask,
                        &mut self.a,
                        &mut self.b,
                        &mut self.c,
                        w,
                        h,
                        p.edge,
                        self.execution,
                    );
                    plate += 1;
                }
                self.info.palette = print_colors;
            }
        } else {
            self.info.plates = if p.method == Method::Cmyk { 4 } else { 1 };
            self.info.palette_len = self.info.plates;
            self.info.palette = if p.method == Method::Cmyk {
                let mut c = [[0; 3]; MAX_COLORS];
                c[..4].copy_from_slice(&[[0, 174, 239], [236, 0, 140], [255, 242, 0], [0, 0, 0]]);
                c
            } else {
                let mut c = [[0; 3]; MAX_COLORS];
                c[0] = p.ink;
                c
            };
            self.masks.resize(n * self.info.plates, 0);
            prepare_pattern(&mut self.patterns, p);
            for plate in 0..self.info.plates {
                self.execution.chunks(&mut self.a, 4, |offset, part| {
                    for (j, v) in part.iter_mut().enumerate() {
                        let rgb = work.rgb(offset + j).map(|c| lut[c as usize]);
                        *v = if p.method == Method::One {
                            1.0 - color::luma(rgb)
                        } else {
                            let f = rgb.map(|c| c as f32 / 255.0);
                            let k = (1.0 - f[0].max(f[1]).max(f[2])) * p.black_amount;
                            if plate == 3 {
                                k
                            } else {
                                ((1.0 - f[plate] - k) / (1.0 - k).max(1e-3)).clamp(0.0, 1.0)
                            }
                        };
                    }
                });
                if p.dark && p.dark_invert && p.method == Method::One {
                    for i in 0..n {
                        self.b[i] = f32::from(self.a[i] > 0.08);
                    }
                    let r = (0.8 * w as f32 / aw).round().max(1.0) as usize;
                    filters::morphology(
                        &self.b,
                        &mut self.c,
                        &mut self.d,
                        w,
                        h,
                        r,
                        true,
                        self.execution,
                    );
                    filters::morphology(
                        &self.c,
                        &mut self.b,
                        &mut self.d,
                        w,
                        h,
                        r,
                        false,
                        self.execution,
                    );
                    filters::fill_holes(&mut self.b, &mut self.visited, &mut self.queue, w, h);
                    filters::morphology(
                        &self.b,
                        &mut self.c,
                        &mut self.d,
                        w,
                        h,
                        (p.outline_mm * w as f32 / aw).round() as usize,
                        true,
                        self.execution,
                    );
                    for i in 0..n {
                        self.a[i] = (1.0 - self.a[i]) * self.c[i];
                    }
                }
                render_plane(
                    &mut self.a,
                    &mut self.b,
                    &mut self.c,
                    &mut self.masks[plate * n..(plate + 1) * n],
                    &mut self.errors,
                    &self.patterns,
                    w,
                    h,
                    plate,
                    p,
                    aw,
                    self.execution,
                );
            }
        }
        if p.dark && p.underbase && !(self.info.plates == 1 && self.info.palette[0] == [255; 3]) {
            for i in 0..n {
                self.a[i] = f32::from((0..self.info.plates).any(|c| self.masks[c * n + i] != 0));
            }
            let radius = (0.8 * w as f32 / aw).round().max(1.0) as usize;
            filters::morphology(
                &self.a,
                &mut self.b,
                &mut self.c,
                w,
                h,
                radius,
                true,
                self.execution,
            );
            filters::morphology(
                &self.b,
                &mut self.a,
                &mut self.c,
                w,
                h,
                radius,
                false,
                self.execution,
            );
            filters::morphology(
                &self.a,
                &mut self.b,
                &mut self.c,
                w,
                h,
                (0.25 * w as f32 / aw).round().max(1.0) as usize,
                false,
                self.execution,
            );
            let old = self.info.plates;
            self.masks.resize((old + 1) * n, 0);
            self.masks.copy_within(0..old * n, n);
            for i in 0..n {
                self.masks[i] = u8::from(self.b[i] > 0.5);
            }
            self.info.palette.copy_within(0..old, 1);
            self.info.palette[0] = [255; 3];
            self.info.plates += 1;
        }
        self.composite(None, p);
        Ok(())
    }

    pub fn composite(&mut self, plate: Option<usize>, p: Params) {
        let n = self.info.width * self.info.height;
        let info = self.info;
        let masks = &self.masks;
        self.execution.chunks(&mut self.pixels, 4, |offset, part| {
            for (j, target) in part.iter_mut().enumerate() {
                let i = offset + j;
                let mut c = if p.dark && plate.is_none() {
                    p.dark_bg
                } else {
                    [255; 3]
                };
                for k in 0..info.plates {
                    if masks[k * n + i] == 0 {
                        continue;
                    }
                    if let Some(wanted) = plate {
                        if k == wanted {
                            c = [0; 3];
                        }
                    } else if p.dark || (p.method == Method::Spot && !p.photo) {
                        c = info.palette[k];
                    } else {
                        for (v, &ink) in c.iter_mut().zip(&info.palette[k]) {
                            *v = ((*v as u32 * ink as u32) / 255) as u8;
                        }
                    }
                }
                *target = bgra(c);
            }
        });
    }

    pub fn encode_bmp(&mut self) {
        let (w, h) = (self.info.width, self.info.height);
        let len = 122 + w * h * 4;
        self.bmp.resize(len, 0);
        self.bmp[..122].fill(0);
        self.bmp[..2].copy_from_slice(b"BM");
        self.bmp[2..6].copy_from_slice(&(len as u32).to_le_bytes());
        self.bmp[10..14].copy_from_slice(&122u32.to_le_bytes());
        self.bmp[14..18].copy_from_slice(&108u32.to_le_bytes());
        self.bmp[18..22].copy_from_slice(&(w as i32).to_le_bytes());
        self.bmp[22..26].copy_from_slice(&(-(h as i32)).to_le_bytes());
        self.bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        self.bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
        // BITMAPV4HEADER with explicit RGBA bit masks preserves preview transparency.
        self.bmp[30..34].copy_from_slice(&3u32.to_le_bytes());
        for (i, mask) in [0x00ff0000u32, 0x0000ff00, 0x000000ff, 0xff000000]
            .into_iter()
            .enumerate()
        {
            self.bmp[54 + i * 4..58 + i * 4].copy_from_slice(&mask.to_le_bytes());
        }
        self.bmp[70..74].copy_from_slice(&0x73524742u32.to_le_bytes());
        self.bmp[122..].copy_from_slice(self.pixels.as_flattened());
    }

    pub fn write_svg(&mut self, p: Params, colored: bool) {
        self.svg.clear();
        self.info.paths = 0;
        self.info.nodes = 0;
        vector::write(self, p, colored);
    }
}

fn bgra(rgb: [u8; 3]) -> [u8; 4] {
    [rgb[2], rgb[1], rgb[0], 255]
}
fn clean(
    mask: &mut [u8],
    visited: &mut [u8],
    queue: &mut Vec<u32>,
    w: usize,
    h: usize,
    size: usize,
) {
    filters::clean_components(mask, visited, queue, w, h, size, false);
    filters::clean_components(mask, visited, queue, w, h, size, true);
}
#[allow(clippy::too_many_arguments)] // Borrow disjoint reusable planes without an allocating wrapper.
fn smooth_mask(
    mask: &mut [u8],
    a: &mut [f32],
    b: &mut [f32],
    c: &mut [f32],
    w: usize,
    h: usize,
    sigma: f32,
    exec: Execution,
) {
    if sigma <= 0.0 {
        return;
    }
    for (p, &v) in a.iter_mut().zip(mask.iter()) {
        *p = v as f32;
    }
    filters::gaussian(a, b, c, w, h, sigma, exec);
    for (p, &v) in mask.iter_mut().zip(b.iter()) {
        *p = u8::from(v >= 0.5);
    }
}
#[allow(clippy::too_many_arguments)] // Borrow disjoint reusable planes without an allocating wrapper.
fn threshold(
    gray: &[f32],
    mask: &mut [u8],
    temp: &mut [f32],
    scratch: &mut [f32],
    w: usize,
    h: usize,
    p: Params,
    exec: Execution,
) {
    let threshold = if p.threshold > 0.0 {
        p.threshold
    } else {
        filters::otsu(&filters::histogram(gray))
    };
    if p.adaptive && p.threshold == 0.0 {
        filters::gaussian(
            gray,
            temp,
            scratch,
            w,
            h,
            (w.max(h) as f32 / 120.0).clamp(1.0, 8.0),
            exec,
        );
        for i in 0..mask.len() {
            mask[i] = u8::from(gray[i] <= temp[i] - 8.0 / 255.0);
        }
    } else {
        exec.chunks(mask, 4, |offset, part| {
            for (i, v) in part.iter_mut().enumerate() {
                *v = u8::from(gray[offset + i] <= threshold);
            }
        });
    }
}
fn border_color(image: ImageView<'_>) -> [u8; 3] {
    let mut hist = [[0usize; 256]; 3];
    let mut count = 0usize;
    for i in 0..image.len() {
        if i < image.width
            || i >= image.len() - image.width
            || i % image.width == 0
            || i % image.width == image.width - 1
        {
            let rgb = image.rgb(i);
            for d in 0..3 {
                hist[d][rgb[d] as usize] += 1;
            }
            count += 1;
        }
    }
    let mut out = [0; 3];
    for d in 0..3 {
        let mut sum = 0;
        for (i, &bin_count) in hist[d].iter().enumerate() {
            sum += bin_count;
            if sum >= count.div_ceil(2) {
                out[d] = i as u8;
                break;
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)] // Borrow disjoint reusable planes without an allocating wrapper.
fn render_plane(
    a: &mut [f32],
    b: &mut [f32],
    c: &mut [f32],
    mask: &mut [u8],
    errors: &mut Vec<f32>,
    patterns: &PatternCache,
    w: usize,
    h: usize,
    plate: usize,
    p: Params,
    aw: f32,
    exec: Execution,
) {
    if p.smoothing > 0.0 {
        filters::gaussian(a, b, c, w, h, p.smoothing, exec);
        a.copy_from_slice(b);
    }
    exec.chunks(a, 4, |_, part| {
        simd::tone_in_place(part, p.contrast, p.brightness, p.invert)
    });
    if p.render == Render::Screen {
        let angle = if p.method == Method::One {
            p.angle
        } else {
            [15.0, 75.0, 0.0, 45.0, 22.5, 67.5, 30.0, 60.0, 7.5, 52.5][plate % 10]
        };
        patterns.screen_mask(a, mask, w, (25.4 / p.lpi) / (aw / w as f32), angle, exec);
    } else {
        patterns.dither_mask(a, mask, errors, w, p.dither, plate, exec);
    }
}

fn prepare_pattern(patterns: &mut PatternCache, p: Params) {
    if p.render == Render::Screen {
        patterns.prepare_screen(p.shape, p.stamp);
    } else {
        patterns.prepare_dither(p.dither, p.pattern, p.pattern_size);
    }
}
