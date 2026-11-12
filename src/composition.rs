//! Millimetre-based composition model shared by preview, SVG export and print handoff.
use std::{fmt::Write, sync::Arc};

pub type Point = [f32; 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Rectangle,
    Ellipse,
    Polygon,
    Star,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    File,
    Convert,
    Print,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Artwork {
    pub name: String,
    pub origin: Origin,
    pub aspect: f32,
    /// A self-contained image URI. SVG snapshots retain their vector contents.
    pub uri: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Shape(Shape),
    Path { points: Vec<Point>, closed: bool },
    Text(String),
    Artwork(Arc<Artwork>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub id: u64,
    pub group: Option<u64>,
    pub name: String,
    pub content: Content,
    pub center: Point,
    pub size: Point,
    pub rotation: f32,
    pub flip: [bool; 2],
    pub fill: Option<u32>,
    pub stroke: Option<u32>,
    pub stroke_width: f32,
}

impl Object {
    pub fn new(content: Content, center: Point, size: Point) -> Self {
        let name = match &content {
            Content::Shape(s) => format!("{s:?}"),
            Content::Path { .. } => "Path".into(),
            Content::Text(_) => "Text".into(),
            Content::Artwork(a) => a.name.clone(),
        };
        Self {
            id: 0,
            group: None,
            name,
            content,
            center,
            size: size.map(|v| v.max(0.1)),
            rotation: 0.,
            flip: [false; 2],
            fill: Some(0x222222),
            stroke: None,
            stroke_width: 0.5,
        }
    }
    /// Normalized object coordinates, including rotation and reflection.
    pub fn local(&self, p: Point) -> Point {
        let [x, y] = [p[0] - self.center[0], p[1] - self.center[1]];
        let (s, c) = self.rotation.to_radians().sin_cos();
        let q = [c * x + s * y, -s * x + c * y];
        std::array::from_fn(|i| q[i] / self.size[i] * if self.flip[i] { -1. } else { 1. } + 0.5)
    }
    pub fn world(&self, p: Point) -> Point {
        let [x, y] = std::array::from_fn(|i| {
            (p[i] - 0.5) * self.size[i] * if self.flip[i] { -1. } else { 1. }
        });
        let (s, c) = self.rotation.to_radians().sin_cos();
        [
            self.center[0] + c * x - s * y,
            self.center[1] + s * x + c * y,
        ]
    }
    pub fn vertices(&self) -> Option<Vec<Point>> {
        match &self.content {
            Content::Path { points, .. } => Some(points.clone()),
            Content::Shape(Shape::Rectangle) => Some(vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]),
            Content::Shape(Shape::Polygon | Shape::Star) => {
                let star = self.content == Content::Shape(Shape::Star);
                let count = if star { 10 } else { 5 };
                Some(
                    (0..count)
                        .map(|i| {
                            let a = -std::f32::consts::FRAC_PI_2
                                + i as f32 * std::f32::consts::TAU / count as f32;
                            let r = if star && i % 2 == 1 { 0.22 } else { 0.5 };
                            [0.5 + r * a.cos(), 0.5 + r * a.sin()]
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }
    pub fn normalize_path(&mut self) {
        let Content::Path { points, .. } = &self.content else {
            return;
        };
        if points.is_empty() {
            return;
        }
        let min: Point =
            std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min));
        let max: Point = std::array::from_fn(|k| {
            points
                .iter()
                .map(|p| p[k])
                .fold(f32::NEG_INFINITY, f32::max)
        });
        let extent: Point = std::array::from_fn(|k| (max[k] - min[k]).max(0.1 / self.size[k]));
        self.center = self.world(std::array::from_fn(|k| min[k] + extent[k] / 2.));
        self.size = std::array::from_fn(|k| self.size[k] * extent[k]);
        if let Content::Path { points, .. } = &mut self.content {
            for p in points {
                *p = std::array::from_fn(|k| (p[k] - min[k]) / extent[k]);
            }
        }
    }
    pub fn hit(&self, p: Point, tolerance: f32) -> bool {
        let q = self.local(p);
        if let Content::Path {
            points,
            closed: false,
        } = &self.content
        {
            return points.windows(2).any(|v| {
                segment_distance(p, self.world(v[0]), self.world(v[1]))
                    <= tolerance.max(self.stroke_width / 2.)
            });
        }
        if self.content == Content::Shape(Shape::Ellipse) {
            return (q[0] - 0.5).hypot(q[1] - 0.5) <= 0.5;
        }
        if let Some(vertices) = self.vertices() {
            let mut inside = false;
            for i in 0..vertices.len() {
                let a = vertices[i];
                let b = vertices[(i + 1) % vertices.len()];
                if segment_distance(p, self.world(a), self.world(b)) <= tolerance {
                    return true;
                }
                if (a[1] > q[1]) != (b[1] > q[1])
                    && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0]
                {
                    inside = !inside;
                }
            }
            return inside;
        }
        (0. ..=1.).contains(&q[0]) && (0. ..=1.).contains(&q[1])
    }
}

pub fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let n = d[0] * d[0] + d[1] * d[1];
    let t = if n > 0. {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / n
    } else {
        0.
    }
    .clamp(0., 1.);
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub size: Point,
    pub objects: Vec<Object>,
    pub selected: Vec<u64>,
    next_id: u64,
}
impl Default for Page {
    fn default() -> Self {
        Self {
            size: [297., 210.],
            objects: Vec::new(),
            selected: Vec::new(),
            next_id: 1,
        }
    }
}
impl Page {
    pub fn add(&mut self, mut object: Object) -> u64 {
        object.id = self.next_id;
        self.next_id += 1;
        let id = object.id;
        self.objects.push(object);
        self.selected = vec![id];
        id
    }
    pub fn selection(&self) -> Option<&Object> {
        self.objects.iter().find(|o| self.selected.contains(&o.id))
    }
    pub fn selection_frame(&self) -> Option<Object> {
        let objects: Vec<_> = self
            .objects
            .iter()
            .filter(|o| self.selected.contains(&o.id))
            .collect();
        if objects.len() == 1 {
            return Some(objects[0].clone());
        }
        if objects.is_empty() {
            return None;
        }
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for o in objects {
            for p in [[0., 0.], [1., 0.], [1., 1.], [0., 1.]].map(|p| o.world(p)) {
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
        }
        Some(Object::new(
            Content::Shape(Shape::Rectangle),
            std::array::from_fn(|k| (min[k] + max[k]) / 2.),
            std::array::from_fn(|k| max[k] - min[k]),
        ))
    }
    pub fn hit(&self, p: Point, tolerance: f32) -> Option<u64> {
        self.objects
            .iter()
            .rev()
            .find(|o| o.hit(p, tolerance))
            .map(|o| o.id)
    }
    pub fn select(&mut self, id: Option<u64>, extend: bool) {
        if !extend {
            self.selected.clear();
        }
        if let Some(id) = id {
            let group = self
                .objects
                .iter()
                .find(|o| o.id == id)
                .and_then(|o| o.group);
            let ids: Vec<_> = self
                .objects
                .iter()
                .filter(|o| o.id == id || group.is_some() && o.group == group)
                .map(|o| o.id)
                .collect();
            let remove = extend && self.selected.contains(&id);
            for id in ids {
                if remove {
                    self.selected.retain(|&v| v != id);
                } else if !self.selected.contains(&id) {
                    self.selected.push(id);
                }
            }
        }
    }
    pub fn edit_selected(&mut self, mut edit: impl FnMut(&mut Object)) {
        for o in &mut self.objects {
            if self.selected.contains(&o.id) {
                edit(o);
            }
        }
    }
    pub fn remove(&mut self) {
        self.objects.retain(|o| !self.selected.contains(&o.id));
        self.selected.clear();
    }
    pub fn duplicate(&mut self) {
        let mut copies: Vec<_> = self
            .objects
            .iter()
            .filter(|o| self.selected.contains(&o.id))
            .cloned()
            .collect();
        // A duplicate must not remain linked to its original group.
        let group = (copies.len() > 1).then_some(self.next_id);
        let mut ids = Vec::new();
        for o in &mut copies {
            o.center[0] += 5.;
            o.center[1] += 5.;
            o.group = group;
            ids.push(self.add(o.clone()));
        }
        self.selected = ids;
    }
    pub fn group(&mut self) {
        if self.selected.len() > 1 {
            let group = self.next_id;
            self.next_id += 1;
            self.edit_selected(|o| o.group = Some(group));
        }
    }
    pub fn ungroup(&mut self) {
        self.edit_selected(|o| o.group = None);
    }
    pub fn reorder(&mut self, front: bool) {
        let mut selected = Vec::new();
        let mut others = Vec::new();
        for o in self.objects.drain(..) {
            if self.selected.contains(&o.id) {
                selected.push(o);
            } else {
                others.push(o);
            }
        }
        if front {
            others.extend(selected);
            self.objects = others;
        } else {
            selected.extend(others);
            self.objects = selected;
        }
    }
    pub fn svg(&self) -> String {
        let [w, h] = self.size;
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w:.3}mm\" height=\"{h:.3}mm\" viewBox=\"0 0 {w:.3} {h:.3}\"><defs><clipPath id=\"page\"><rect width=\"{w}\" height=\"{h}\"/></clipPath></defs><g clip-path=\"url(#page)\">"
        );
        for o in &self.objects {
            write_object(&mut out, o);
        }
        out.push_str("</g></svg>");
        out
    }
}
fn color(c: Option<u32>) -> String {
    c.map_or_else(|| "none".into(), |c| format!("#{:06x}", c & 0xffffff))
}
fn write_object(out: &mut String, o: &Object) {
    let [x, y] = o.center;
    let [w, h] = o.size;
    let sx = if o.flip[0] { -1 } else { 1 };
    let sy = if o.flip[1] { -1 } else { 1 };
    // Geometry is expressed in millimetres, so stroke weight is independent of aspect ratio.
    write!(out, "<g id=\"object-{}\" transform=\"translate({x} {y}) rotate({}) scale({sx} {sy}) translate({} {})\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"><title>{}</title>", o.id, o.rotation, -w / 2., -h / 2., color(o.fill), color(o.stroke), o.stroke_width, escape(&o.name)).unwrap();
    match &o.content {
        Content::Shape(Shape::Ellipse) => {
            write!(
                out,
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"/>",
                w / 2.,
                h / 2.,
                w / 2.,
                h / 2.
            )
            .unwrap();
        }
        Content::Text(text) => {
            write!(out, "<text x=\"0\" y=\"{}\" font-family=\"monospace\" font-size=\"{}\" textLength=\"{w}\" lengthAdjust=\"spacingAndGlyphs\">{}</text>", h * 92. / 120., h * 100. / 120., escape(text)).unwrap();
        }
        Content::Artwork(a) => {
            write!(
                out,
                "<image width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"none\" xlink:href=\"{}\"/>",
                a.uri
            )
            .unwrap();
        }
        _ => {
            let vertices = o.vertices().unwrap_or_default();
            out.push_str("<path d=\"");
            for (i, [x, y]) in vertices.iter().enumerate() {
                write!(out, "{}{} {}", if i == 0 { "M" } else { "L" }, x * w, y * h).unwrap();
            }
            if !matches!(o.content, Content::Path { closed: false, .. }) {
                out.push('Z');
            }
            out.push_str("\"/>");
        }
    }
    out.push_str("</g>");
}
pub fn escape(text: &str) -> String {
    text.chars()
        .filter(|&c| c == '\n' || c == '\t' || c >= ' ')
        .fold(String::new(), |mut s, c| {
            s.push_str(match c {
                '&' => "&amp;",
                '<' => "&lt;",
                '>' => "&gt;",
                '"' => "&quot;",
                '\'' => "&apos;",
                _ => {
                    s.push(c);
                    return s;
                }
            });
            s
        })
}

pub fn data_uri(mime: &str, bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = format!("data:{mime};base64,");
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().fold(0u32, |n, &b| (n << 8) | u32::from(b)) << ((3 - chunk.len()) * 8);
        for i in 0..4 {
            s.push(if i > chunk.len() {
                '='
            } else {
                TABLE[((n >> (18 - i * 6)) & 63) as usize] as char
            });
        }
    }
    s
}

#[derive(Default)]
pub struct Document {
    pub page: Page,
    undo: Vec<Page>,
    redo: Vec<Page>,
}
impl Document {
    pub fn edit(&mut self, edit: impl FnOnce(&mut Page)) {
        let before = self.page.clone();
        edit(&mut self.page);
        if before != self.page {
            self.push_undo(before);
            self.redo.clear();
        }
    }
    fn push_undo(&mut self, page: Page) {
        if self.undo.last() != Some(&page) {
            self.undo.push(page);
            if self.undo.len() > 100 {
                self.undo.remove(0);
            }
        }
    }
    pub fn checkpoint(&mut self) {
        self.push_undo(self.page.clone());
        self.redo.clear();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.page, p));
        }
    }
    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.page, p));
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ViewTransform {
    pub origin: Point,
    pub scale: f32,
}
impl ViewTransform {
    pub fn fit(origin: Point, size: Point, page: Point, actual: bool) -> Option<Self> {
        if size.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return None;
        }
        let scale = if actual {
            96. / 25.4
        } else {
            (size[0] / page[0]).min(size[1] / page[1])
        };
        Some(Self {
            origin: std::array::from_fn(|i| origin[i] + (size[i] - page[i] * scale) / 2.),
            scale,
        })
    }
    pub fn world(self, p: Point) -> Point {
        std::array::from_fn(|i| (p[i] - self.origin[i]) / self.scale)
    }
    pub fn screen(self, p: Point) -> Point {
        std::array::from_fn(|i| self.origin[i] + p[i] * self.scale)
    }
}
