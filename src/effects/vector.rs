use super::{Engine, Method, Operation, Params, Render};
use std::fmt::Write;

#[derive(Default)]
pub struct Tracer {
    edges: Vec<u8>,
    points: Vec<[f32; 2]>,
    simplified: Vec<[f32; 2]>,
    keep: Vec<u8>,
    stack: Vec<(usize, usize)>,
}

impl Tracer {
    fn path(
        &mut self,
        mask: &[u8],
        w: usize,
        h: usize,
        epsilon: f32,
        corner: f32,
        out: &mut String,
    ) -> (usize, usize) {
        let stride = w + 1;
        self.edges.resize(stride * (h + 1), 0);
        self.edges.fill(0);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if mask[i] == 0 {
                    continue;
                }
                if y == 0 || mask[i - w] == 0 {
                    self.edges[y * stride + x] |= 1;
                }
                if x + 1 == w || mask[i + 1] == 0 {
                    self.edges[y * stride + x + 1] |= 2;
                }
                if y + 1 == h || mask[i + w] == 0 {
                    self.edges[(y + 1) * stride + x + 1] |= 4;
                }
                if x == 0 || mask[i - 1] == 0 {
                    self.edges[(y + 1) * stride + x] |= 8;
                }
            }
        }
        let mut paths = 0;
        let mut nodes = 0;
        for start in 0..self.edges.len() {
            while self.edges[start] != 0 {
                self.points.clear();
                let mut vertex = start;
                let mut direction = self.edges[start].trailing_zeros() as usize;
                loop {
                    self.points
                        .push([(vertex % stride) as f32, (vertex / stride) as f32]);
                    self.edges[vertex] &= !(1 << direction);
                    vertex = match direction {
                        0 => vertex + 1,
                        1 => vertex + stride,
                        2 => vertex - 1,
                        _ => vertex - stride,
                    };
                    if vertex == start {
                        break;
                    }
                    let bits = self.edges[vertex];
                    let Some(next) = [
                        (direction + 1) % 4,
                        direction,
                        (direction + 3) % 4,
                        (direction + 2) % 4,
                    ]
                    .into_iter()
                    .find(|&d| bits & (1 << d) != 0) else {
                        break;
                    };
                    direction = next;
                }
                self.simplify(epsilon);
                let pts = &self.simplified;
                let n = pts.len();
                if n < 3 {
                    continue;
                }
                write!(out, "M{:.2} {:.2}", pts[0][0], pts[0][1]).unwrap();
                nodes += 1;
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (a, ha) = handle(pts, i, corner);
                    let (b, hb) = handle(pts, j, corner);
                    if ha == 0.0 && hb == 0.0 {
                        write!(out, "L{:.2} {:.2}", pts[j][0], pts[j][1]).unwrap();
                    } else {
                        write!(
                            out,
                            "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                            pts[i][0] + a[0] * ha,
                            pts[i][1] + a[1] * ha,
                            pts[j][0] - b[0] * hb,
                            pts[j][1] - b[1] * hb,
                            pts[j][0],
                            pts[j][1]
                        )
                        .unwrap();
                    }
                    nodes += 1;
                }
                out.push('Z');
                paths += 1;
            }
        }
        (paths, nodes)
    }

    fn simplify(&mut self, epsilon: f32) {
        let n = self.points.len();
        self.simplified.clear();
        if n < 3 {
            return;
        }
        self.keep.resize(n, 0);
        self.keep.fill(0);
        self.stack.clear();
        let a = self.points[0];
        let far = (1..n)
            .max_by(|&i, &j| {
                length_squared(sub(self.points[i], a))
                    .total_cmp(&length_squared(sub(self.points[j], a)))
            })
            .unwrap();
        self.keep[0] = 1;
        self.keep[far] = 1;
        self.stack.push((0, far));
        self.stack.push((far, n));
        while let Some((start, end)) = self.stack.pop() {
            let a = self.points[start];
            let b = self.points[end % n];
            let delta = sub(b, a);
            let len = length_squared(delta).max(1e-12);
            let mut max = epsilon * epsilon;
            let mut index = None;
            for i in start + 1..end {
                let p = self.points[i];
                let v = sub(p, a);
                let t = ((v[0] * delta[0] + v[1] * delta[1]) / len).clamp(0.0, 1.0);
                let d = length_squared([v[0] - t * delta[0], v[1] - t * delta[1]]);
                if d > max {
                    max = d;
                    index = Some(i);
                }
            }
            if let Some(i) = index {
                self.keep[i] = 1;
                self.stack.push((start, i));
                self.stack.push((i, end));
            }
        }
        for (i, &p) in self.points.iter().enumerate() {
            if self.keep[i] != 0 {
                self.simplified.push(p);
            }
        }
    }
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn length_squared(a: [f32; 2]) -> f32 {
    a[0] * a[0] + a[1] * a[1]
}
fn handle(pts: &[[f32; 2]], i: usize, corner: f32) -> ([f32; 2], f32) {
    let n = pts.len();
    let before = sub(pts[(i + n - 1) % n], pts[i]);
    let after = sub(pts[(i + 1) % n], pts[i]);
    let l1 = length_squared(before).sqrt();
    let l2 = length_squared(after).sqrt();
    let angle = ((before[0] * after[0] + before[1] * after[1]) / (l1 * l2).max(1e-12))
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees();
    let tangent = sub(pts[(i + 1) % n], pts[(i + n - 1) % n]);
    let length = length_squared(tangent).sqrt().max(1e-12);
    (
        [tangent[0] / length, tangent[1] / length],
        if angle < corner {
            0.0
        } else {
            l1.min(l2) / 3.0
        },
    )
}

pub fn write(engine: &mut Engine, p: Params, colored: bool) {
    let info = engine.info;
    let (w, h) = (info.width, info.height);
    let n = w * h;
    if p.operation == Operation::Convert {
        let dw = if p.upscale < 1.0 {
            info.original_width
        } else {
            w
        };
        let dh = if p.upscale < 1.0 {
            info.original_height
        } else {
            h
        };
        write!(engine.svg,"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{dw}\" height=\"{dh}\" viewBox=\"0 0 {w} {h}\">").unwrap();
        if info.lineart {
            path_element(engine, p, 0, colored);
        } else {
            let mut areas = [0usize; 32];
            for &label in &engine.labels {
                areas[label as usize] += 1;
            }
            let mut order = [0usize; 32];
            for (i, v) in order.iter_mut().enumerate() {
                *v = i;
            }
            order[..info.palette_len].sort_unstable_by_key(|&i| std::cmp::Reverse(areas[i]));
            let c = info.palette[order[0]];
            write!(
                engine.svg,
                "<rect width=\"{w}\" height=\"{h}\" fill=\"#{:02x}{:02x}{:02x}\"/>",
                c[0], c[1], c[2]
            )
            .unwrap();
            for layer in 1..info.palette_len {
                for i in 0..n {
                    engine.masks[i] = u8::from(
                        order[layer..info.palette_len].contains(&(engine.labels[i] as usize)),
                    );
                }
                super::clean(
                    &mut engine.masks,
                    &mut engine.visited,
                    &mut engine.queue,
                    w,
                    h,
                    p.speckle,
                );
                super::smooth_mask(
                    &mut engine.masks,
                    &mut engine.a,
                    &mut engine.b,
                    &mut engine.c,
                    w,
                    h,
                    p.edge,
                    engine.execution,
                );
                path_element(engine, p, order[layer], colored);
            }
        }
        engine.svg.push_str("</svg>");
        return;
    }
    let [aw, ah] = info.art_mm;
    let pw = p.page_width.max(aw + 2.0 * p.margin);
    let ph = p.page_height.max(ah + 2.0 * p.margin);
    let x = (pw - aw) / 2.0;
    let y = (ph - ah) / 2.0;
    write!(engine.svg,"<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" width=\"{pw:.3}mm\" height=\"{ph:.3}mm\" viewBox=\"0 0 {pw:.3} {ph:.3}\">").unwrap();
    for plate in 0..info.plates {
        let c = if colored { info.palette[plate] } else { [0; 3] };
        write!(engine.svg,"<g id=\"plate-{}\" inkscape:groupmode=\"layer\" inkscape:label=\"Ink {}\" fill=\"#{:02x}{:02x}{:02x}\"><g transform=\"translate({x:.3} {y:.3}) scale({:.6} {:.6})\">",plate+1,plate+1,c[0],c[1],c[2],aw/w as f32,ah/h as f32).unwrap();
        if p.render == Render::Dither && !(p.method == Method::Spot && !p.photo) {
            engine.svg.push_str("<path d=\"");
            for yy in 0..h {
                let mut xx = 0;
                while xx < w {
                    if engine.masks[plate * n + yy * w + xx] == 0 {
                        xx += 1;
                        continue;
                    }
                    let start = xx;
                    while xx < w && engine.masks[plate * n + yy * w + xx] != 0 {
                        xx += 1;
                    }
                    let len = xx - start;
                    write!(engine.svg, "M{start} {yy}h{len}v1h-{len}z").unwrap();
                    engine.info.paths += 1;
                    engine.info.nodes += 5;
                }
            }
            engine.svg.push_str("\"/>");
        } else {
            engine.svg.push_str("<path fill-rule=\"evenodd\" d=\"");
            let (paths, nodes) = engine.trace.path(
                &engine.masks[plate * n..(plate + 1) * n],
                w,
                h,
                p.epsilon,
                p.corner,
                &mut engine.svg,
            );
            engine.info.paths += paths;
            engine.info.nodes += nodes;
            engine.svg.push_str("\"/>");
        }
        engine.svg.push_str("</g>");
        if p.marks {
            for yy in [12.0, ph - 12.0] {
                for (i, xx) in [14.0, pw / 2.0, pw - 14.0].into_iter().enumerate() {
                    if !p.six_marks && i == 1 {
                        continue;
                    }
                    write!(engine.svg,"<g fill=\"none\" stroke=\"#000\" stroke-width=\"0.706\"><rect x=\"{:.3}\" y=\"{:.3}\" width=\"4.5\" height=\"4.5\"/><path d=\"M{:.3} {yy:.3}h10M{xx:.3} {:.3}v10\"/></g>",xx-2.25,yy-2.25,xx-5.0,yy-5.0).unwrap();
                }
            }
        }
        if p.ink_names {
            write!(engine.svg,"<text x=\"22\" y=\"13.4\" font-size=\"4.2\" fill=\"#000\">PLATE {} — INK {}</text>",plate+1,plate+1).unwrap();
        }
        engine.svg.push_str("</g>");
    }
    engine.svg.push_str("</svg>");
}

fn path_element(engine: &mut Engine, p: Params, color: usize, colored: bool) {
    let c = if colored {
        engine.info.palette[color]
    } else {
        [0; 3]
    };
    write!(
        engine.svg,
        "<path fill=\"#{:02x}{:02x}{:02x}\" fill-rule=\"evenodd\" d=\"",
        c[0], c[1], c[2]
    )
    .unwrap();
    let (paths, nodes) = engine.trace.path(
        &engine.masks,
        engine.info.width,
        engine.info.height,
        p.epsilon,
        p.corner,
        &mut engine.svg,
    );
    engine.info.paths += paths;
    engine.info.nodes += nodes;
    engine.svg.push_str("\"/>");
}
