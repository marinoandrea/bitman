//! Flat garment mock-ups. Geometry and placement use centimetres; preview and
//! JPEG export use exactly the same renderer. Size presets are illustrative.
use std::{io::Write, sync::Arc};
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke,
    Transform,
};

pub const CANVAS: [f32; 2] = [150.0, 105.0];
pub const PREVIEW_WIDTH: u32 = 1000;
pub const EXPORT_WIDTH: u32 = 3000;
pub const SIZES: [[f32; 2]; 6] = [
    [46.0, 64.0],
    [49.0, 67.0],
    [52.0, 70.0],
    [55.0, 73.0],
    [58.0, 76.0],
    [61.0, 79.0],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    File,
    Convert,
    Print,
}

#[derive(Clone)]
pub struct Artwork {
    pub name: String,
    pub origin: Origin,
    pub pixels: Pixmap,
}

impl Artwork {
    /// GPUI images and engine pixels are straight-alpha BGRA. Skia needs
    /// premultiplied RGBA; doing this once also preserves white ink.
    pub fn from_bgra(
        name: String,
        origin: Origin,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> Result<Self, String> {
        if width == 0 || height == 0 || width > 2048 || height > 2048 {
            return Err("Artwork must be between 1 and 2048 pixels per side".into());
        }
        if width as usize * height as usize * 4 != bytes.len() {
            return Err("Invalid artwork pixels".into());
        }
        let mut pixels = Pixmap::new(width, height).ok_or("Invalid artwork dimensions")?;
        for (dst, src) in pixels
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(bytes.as_chunks::<4>().0.iter())
        {
            let a = u16::from(src[3]);
            dst.copy_from_slice(&[
                ((u16::from(src[2]) * a + 127) / 255) as u8,
                ((u16::from(src[1]) * a + 127) / 255) as u8,
                ((u16::from(src[0]) * a + 127) / 255) as u8,
                src[3],
            ]);
        }
        Ok(Self {
            name,
            origin,
            pixels,
        })
    }
}

#[derive(Clone)]
pub struct Design {
    pub artwork: Arc<Artwork>,
    pub width: f32,
    pub height: f32,
    /// Horizontal centre relative to the garment centreline.
    pub x: f32,
    /// Top of the unrotated design, measured from the bottom of the collar.
    pub top: f32,
    /// Clockwise degrees, about the design centre.
    pub rotation: f32,
    pub keep_proportions: bool,
}

impl Design {
    pub fn centre(&self, collar: f32) -> [f32; 2] {
        [self.x, collar + self.top + self.height / 2.0]
    }
    pub fn local(&self, point: [f32; 2], collar: f32) -> [f32; 2] {
        let c = self.centre(collar);
        let (s, co) = self.rotation.to_radians().sin_cos();
        let (x, y) = (point[0] - c[0], point[1] - c[1]);
        [co * x + s * y, -s * x + co * y]
    }
    pub fn world(&self, local: [f32; 2], collar: f32) -> [f32; 2] {
        let c = self.centre(collar);
        let (s, co) = self.rotation.to_radians().sin_cos();
        [
            c[0] + co * local[0] - s * local[1],
            c[1] + s * local[0] + co * local[1],
        ]
    }
    pub fn contains(&self, point: [f32; 2], collar: f32) -> bool {
        let p = self.local(point, collar);
        p[0].abs() <= self.width / 2.0 && p[1].abs() <= self.height / 2.0
    }
    pub fn set_width(&mut self, width: f32) {
        let width = width.clamp(0.5, 100.0);
        if self.keep_proportions {
            let ratio = self.height / self.width;
            self.width = width.min(100.0 / ratio).max(0.5 / ratio);
            self.height = self.width * ratio;
        } else {
            self.width = width;
        }
    }
    pub fn set_height(&mut self, height: f32) {
        let height = height.clamp(0.5, 100.0);
        if self.keep_proportions {
            let ratio = self.width / self.height;
            self.height = height.min(100.0 / ratio).max(0.5 / ratio);
            self.width = self.height * ratio;
        } else {
            self.height = height;
        }
    }
}

#[derive(Clone)]
pub struct Simulation {
    pub size: usize,
    pub long_sleeve: bool,
    pub back: bool,
    pub color: [u8; 3],
    pub designs: [Vec<Design>; 2],
    pub selected: Option<usize>,
    pub crop: bool,
    pub measurements: bool,
    pub export_measurements: bool,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            size: 3,
            long_sleeve: false,
            back: false,
            color: [255; 3],
            designs: Default::default(),
            selected: None,
            crop: true,
            measurements: true,
            export_measurements: false,
        }
    }
}

impl Simulation {
    pub fn dimensions(&self) -> [f32; 2] {
        SIZES[self.size.min(5)]
    }
    pub fn collar(&self) -> f32 {
        if self.back { 2.0 } else { 7.0 }
    }
    pub fn current(&self) -> &[Design] {
        &self.designs[usize::from(self.back)]
    }
    pub fn current_mut(&mut self) -> &mut Vec<Design> {
        &mut self.designs[usize::from(self.back)]
    }
    pub fn selection(&self) -> Option<&Design> {
        self.selected.and_then(|i| self.current().get(i))
    }
    pub fn selection_mut(&mut self) -> Option<&mut Design> {
        let i = self.selected?;
        self.current_mut().get_mut(i)
    }
    pub fn set_side(&mut self, back: bool) {
        self.back = back;
        self.selected = None;
    }
    pub fn add(&mut self, artwork: Arc<Artwork>) {
        let ratio = artwork.pixels.height() as f32 / artwork.pixels.width() as f32;
        let width = 25.0_f32.min(35.0 / ratio);
        self.current_mut().push(Design {
            artwork,
            width,
            height: width * ratio,
            x: 0.0,
            top: 8.0,
            rotation: 0.0,
            keep_proportions: true,
        });
        self.selected = Some(self.current().len() - 1);
    }
    pub fn duplicate(&mut self) {
        if let Some(mut design) = self.selection().cloned() {
            design.x = (design.x + 2.0).clamp(-70.0, 70.0);
            design.top = (design.top + 2.0).clamp(-20.0, 90.0);
            self.current_mut().push(design);
            self.selected = Some(self.current().len() - 1);
        }
    }
    pub fn remove(&mut self) {
        if let Some(i) = self.selected.take()
            && i < self.current().len()
        {
            self.current_mut().remove(i);
        }
    }
    pub fn preset(&mut self, preset: usize) {
        let [w, h] = self.dimensions();
        if let Some(d) = self.selection_mut() {
            match preset {
                0 => {
                    d.x = 0.0;
                }
                1 => {
                    d.set_width(12.0);
                    d.x = -w * 0.22;
                    d.top = 10.0;
                    d.rotation = 0.0;
                }
                _ => {
                    d.set_width(w * 0.65);
                    if d.height > h * 0.65 {
                        d.set_height(h * 0.65);
                    }
                    d.x = 0.0;
                    d.top = 7.0;
                    d.rotation = 0.0;
                }
            }
        }
    }
    pub fn hit(&self, point: [f32; 2]) -> Option<usize> {
        self.current()
            .iter()
            .rposition(|d| d.contains(point, self.collar()))
    }
    pub fn shirt_path(&self) -> Path {
        let [w, h] = self.dimensions();
        let mut p = PathBuilder::new();
        p.move_to(-w * 0.16, 0.0);
        p.line_to(-w * 0.38, 2.0);
        let (reach, end, cuff) = if self.long_sleeve {
            (32.0, 50.0, 56.0)
        } else {
            (18.0, 20.0, 30.0)
        };
        p.line_to(-w / 2.0 - reach, end);
        p.line_to(-w / 2.0 - reach + 10.0, cuff);
        p.line_to(-w / 2.0, 22.0);
        p.line_to(-w / 2.0, h);
        p.line_to(w / 2.0, h);
        p.line_to(w / 2.0, 22.0);
        p.line_to(w / 2.0 + reach - 10.0, cuff);
        p.line_to(w / 2.0 + reach, end);
        p.line_to(w * 0.38, 2.0);
        p.line_to(w * 0.16, 0.0);
        p.quad_to(0.0, self.collar() * 2.0, -w * 0.16, 0.0);
        p.close();
        p.finish().expect("nonempty garment path")
    }

    pub fn render(
        &self,
        width: u32,
        measurements: bool,
        selection: bool,
    ) -> Result<Pixmap, String> {
        if !(100..=4096).contains(&width) {
            return Err("Mock-up width must be 100–4096 pixels".into());
        }
        let height = (width as f32 * CANVAS[1] / CANVAS[0]).round() as u32;
        let mut out = Pixmap::new(width, height).ok_or("Could not allocate mock-up")?;
        out.fill(Color::from_rgba8(242, 243, 245, 255));
        let scale = width as f32 / CANVAS[0];
        let transform =
            Transform::from_row(scale, 0.0, 0.0, scale, width as f32 / 2.0, 10.0 * scale);
        let shirt = self.shirt_path();
        let mut paint = Paint::default();
        paint.set_color_rgba8(self.color[0], self.color[1], self.color[2], 255);
        out.fill_path(&shirt, &paint, FillRule::Winding, transform, None);
        let mut mask = Mask::new(width, height).ok_or("Could not allocate garment mask")?;
        mask.fill_path(&shirt, FillRule::Winding, true, transform);
        for d in self.current() {
            let (s, c) = d.rotation.to_radians().sin_cos();
            let sx = d.width / d.artwork.pixels.width() as f32;
            let sy = d.height / d.artwork.pixels.height() as f32;
            let origin = d.world([-d.width / 2.0, -d.height / 2.0], self.collar());
            let t = Transform::from_row(
                c * sx * scale,
                s * sx * scale,
                -s * sy * scale,
                c * sy * scale,
                (origin[0] + CANVAS[0] / 2.0) * scale,
                (origin[1] + 10.0) * scale,
            );
            out.draw_pixmap(
                0,
                0,
                d.artwork.pixels.as_ref(),
                &PixmapPaint {
                    quality: FilterQuality::Bilinear,
                    ..Default::default()
                },
                t,
                self.crop.then_some(&mask),
            );
        }
        paint.set_color_rgba8(106, 112, 125, 255);
        out.stroke_path(
            &shirt,
            &paint,
            &Stroke {
                width: 0.22,
                ..Default::default()
            },
            transform,
            None,
        );
        let [w, h] = self.dimensions();
        let mut seams = PathBuilder::new();
        seams.move_to(-w / 2.0 + 1.0, h - 2.0);
        seams.line_to(w / 2.0 - 1.0, h - 2.0);
        seams.move_to(-w * 0.16, 1.0);
        seams.quad_to(0.0, self.collar() * 2.0 + 1.0, w * 0.16, 1.0);
        out.stroke_path(
            &seams.finish().unwrap(),
            &paint,
            &Stroke {
                width: 0.15,
                ..Default::default()
            },
            transform,
            None,
        );
        if measurements {
            // Vector numerals keep exported dimensions independent of installed fonts.
            let mut lines = PathBuilder::new();
            for (a, b) in [
                ([-w / 2.0, h + 5.0], [w / 2.0, h + 5.0]),
                ([w / 2.0 + 5.0, 0.0], [w / 2.0 + 5.0, h]),
            ] {
                lines.move_to(a[0], a[1]);
                lines.line_to(b[0], b[1]);
                for v in [a, b] {
                    lines.move_to(v[0] - 0.8, v[1] - 0.8);
                    lines.line_to(v[0] + 0.8, v[1] + 0.8);
                }
            }
            out.stroke_path(
                &lines.finish().unwrap(),
                &paint,
                &Stroke {
                    width: 0.18,
                    ..Default::default()
                },
                transform,
                None,
            );
            draw_measurement(&mut out, w, [-4.0, h + 7.0], &paint, transform);
            draw_measurement(&mut out, h, [w / 2.0 + 7.0, h / 2.0], &paint, transform);
        }
        if selection && let Some(d) = self.selection() {
            paint.set_color_rgba8(49, 94, 216, 255);
            let mut p = PathBuilder::new();
            for (i, local) in [
                [-d.width / 2.0, -d.height / 2.0],
                [d.width / 2.0, -d.height / 2.0],
                [d.width / 2.0, d.height / 2.0],
                [-d.width / 2.0, d.height / 2.0],
            ]
            .into_iter()
            .enumerate()
            {
                let v = d.world(local, self.collar());
                if i == 0 {
                    p.move_to(v[0], v[1]);
                } else {
                    p.line_to(v[0], v[1]);
                }
            }
            p.close();
            let top = d.world([0.0, -d.height / 2.0], self.collar());
            let rotate = d.world([0.0, -d.height / 2.0 - 4.0], self.collar());
            p.move_to(top[0], top[1]);
            p.line_to(rotate[0], rotate[1]);
            out.stroke_path(
                &p.finish().unwrap(),
                &paint,
                &Stroke {
                    width: 0.25,
                    ..Default::default()
                },
                transform,
                None,
            );
            for v in [
                rotate,
                d.world([d.width / 2.0, d.height / 2.0], self.collar()),
            ] {
                let path = PathBuilder::from_circle(v[0], v[1], 0.85).unwrap();
                out.fill_path(&path, &paint, FillRule::Winding, transform, None);
            }
        }
        Ok(out)
    }
    pub fn write_jpeg(&self, writer: impl Write, width: u32) -> Result<(), String> {
        let pixmap = self.render(width, self.export_measurements, false)?;
        let rgb: Vec<u8> = pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 95)
            .encode(
                &rgb,
                pixmap.width(),
                pixmap.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| e.to_string())
    }

    /// The native dialog confirms overwrites of explicit filenames. When adding
    /// a missing extension, never overwrite a different, unconfirmed destination.
    pub fn save_jpeg(&self, path: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let inferred = path.extension().is_none();
        if !inferred
            && !path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg")
            })
        {
            return Err("Choose a filename ending in .jpg or .jpeg".into());
        }
        let mut path = path.to_owned();
        if inferred {
            path.set_extension("jpg");
        }
        let mut bytes = Vec::new();
        self.write_jpeg(&mut bytes, EXPORT_WIDTH)?;
        if inferred {
            let mut file = std::fs::File::create_new(&path).map_err(|e| {
                format!(
                    "{}: {e}; choose the full .jpg filename to replace an existing file",
                    path.display()
                )
            })?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
        } else {
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        }
        Ok(path)
    }
}

/// Map a contain-fitted viewport to the same centimetre coordinates as rendering.
#[derive(Clone, Copy, Debug)]
pub struct ViewTransform {
    pub scale: f32,
    pub origin: [f32; 2],
}
impl ViewTransform {
    pub fn fit(origin: [f32; 2], size: [f32; 2]) -> Option<Self> {
        let scale = (size[0] / CANVAS[0]).min(size[1] / CANVAS[1]);
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        Some(Self {
            scale,
            origin: [
                origin[0] + (size[0] - CANVAS[0] * scale) / 2.0 + CANVAS[0] / 2.0 * scale,
                origin[1] + (size[1] - CANVAS[1] * scale) / 2.0 + 10.0 * scale,
            ],
        })
    }
    pub fn world(self, p: [f32; 2]) -> [f32; 2] {
        [
            (p[0] - self.origin[0]) / self.scale,
            (p[1] - self.origin[1]) / self.scale,
        ]
    }
    pub fn screen(self, p: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + p[0] * self.scale,
            self.origin[1] + p[1] * self.scale,
        ]
    }
}

fn draw_measurement(
    out: &mut Pixmap,
    value: f32,
    origin: [f32; 2],
    paint: &Paint<'_>,
    transform: Transform,
) {
    const SEGMENTS: [[f32; 4]; 7] = [
        [0., 0., 1., 0.],
        [1., 0., 1., 1.],
        [1., 1., 1., 2.],
        [0., 2., 1., 2.],
        [0., 1., 0., 2.],
        [0., 0., 0., 1.],
        [0., 1., 1., 1.],
    ];
    const DIGITS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    let mut p = PathBuilder::new();
    let mut x = origin[0];
    for ch in format!("{value:.0}").bytes() {
        for (i, [x1, y1, x2, y2]) in SEGMENTS.iter().enumerate() {
            if DIGITS[(ch - b'0') as usize] & (1 << i) != 0 {
                p.move_to(x + x1, origin[1] + y1);
                p.line_to(x + x2, origin[1] + y2);
            }
        }
        x += 1.7;
    }
    // Lowercase cm.
    p.move_to(x + 1., origin[1] + 0.7);
    p.line_to(x, origin[1] + 0.7);
    p.line_to(x, origin[1] + 2.);
    p.line_to(x + 1., origin[1] + 2.);
    x += 1.7;
    p.move_to(x, origin[1] + 2.);
    p.line_to(x, origin[1] + 0.7);
    p.line_to(x + 2., origin[1] + 0.7);
    p.line_to(x + 2., origin[1] + 2.);
    p.move_to(x + 1., origin[1] + 0.7);
    p.line_to(x + 1., origin[1] + 2.);
    out.stroke_path(
        &p.finish().unwrap(),
        paint,
        &Stroke {
            width: 0.18,
            ..Default::default()
        },
        transform,
        None,
    );
}
