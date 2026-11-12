use super::{ImageView, MAX_SIDE, parallel::Execution};

pub fn resize(
    image: ImageView<'_>,
    out: &mut [[u8; 4]],
    w: usize,
    h: usize,
    pixel: bool,
    exec: Execution,
) {
    exec.chunks(out, w, |offset, part| {
        for (j, p) in part.iter_mut().enumerate() {
            let i = offset + j;
            let x = i % w;
            let y = i / w;
            let sx = (x as f32 + 0.5) * image.width as f32 / w as f32 - 0.5;
            let sy = (y as f32 + 0.5) * image.height as f32 / h as f32 - 0.5;
            let rgb = if pixel && w >= image.width && h >= image.height {
                image.rgb_at(sx.round() as isize, sy.round() as isize)
            } else {
                // Area integration when shrinking, bicubic interpolation when enlarging.
                let dx = image.width as f32 / w as f32;
                let dy = image.height as f32 / h as f32;
                if dx > 1.0 || dy > 1.0 {
                    let (x0, x1) = (x as f32 * dx, (x + 1) as f32 * dx);
                    let (y0, y1) = (y as f32 * dy, (y + 1) as f32 * dy);
                    let mut sum = [0.0; 3];
                    let mut total = 0.0;
                    for yy in y0.floor() as usize..(y1.ceil() as usize).min(image.height) {
                        for xx in x0.floor() as usize..(x1.ceil() as usize).min(image.width) {
                            let wt = (x1.min((xx + 1) as f32) - x0.max(xx as f32))
                                * (y1.min((yy + 1) as f32) - y0.max(yy as f32));
                            let c = image.rgb(yy * image.width + xx);
                            total += wt;
                            for d in 0..3 {
                                sum[d] += wt * c[d] as f32;
                            }
                        }
                    }
                    sum.map(|v| (v / total.max(1e-9)).round().clamp(0.0, 255.0) as u8)
                } else {
                    let mut sum = [0.0; 3];
                    let mut total = 0.0;
                    for yy in -1..=2 {
                        for xx in -1..=2 {
                            let ix = sx.floor() as isize + xx;
                            let iy = sy.floor() as isize + yy;
                            let wt = cubic(sx - ix as f32) * cubic(sy - iy as f32);
                            let c = image.rgb_at(ix, iy);
                            total += wt;
                            for d in 0..3 {
                                sum[d] += wt * c[d] as f32;
                            }
                        }
                    }
                    sum.map(|v| (v / total.max(1e-9)).round().clamp(0.0, 255.0) as u8)
                }
            };
            *p = [rgb[2], rgb[1], rgb[0], 255];
        }
    });
}

fn cubic(x: f32) -> f32 {
    let a = x.abs();
    if a <= 1.0 {
        1.5 * a * a * a - 2.5 * a * a + 1.0
    } else if a < 2.0 {
        -0.5 * a * a * a + 2.5 * a * a - 4.0 * a + 2.0
    } else {
        0.0
    }
}

pub fn grayscale(image: ImageView<'_>, out: &mut [f32], exec: Execution) {
    exec.chunks(out, 8, |offset, part| {
        super::simd::grayscale(image, part, offset);
    });
}

pub fn median(input: &[f32], output: &mut [f32], w: usize, h: usize, exec: Execution) {
    exec.chunks(output, w, |offset, part| {
        for (i, p) in part.iter_mut().enumerate() {
            let x = (offset + i) % w;
            let y = (offset + i) / w;
            let mut values = [0.0f32; 9];
            let mut n = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    values[n] = input[(y as isize + dy).clamp(0, h as isize - 1) as usize * w
                        + (x as isize + dx).clamp(0, w as isize - 1) as usize];
                    n += 1;
                }
            }
            values.sort_unstable_by(f32::total_cmp);
            *p = values[4];
        }
    });
}

pub fn bilateral(image: ImageView<'_>, out: &mut [[u8; 4]], exec: Execution) {
    let mut spatial = [0.0; 25];
    for y in 0..5 {
        for x in 0..5 {
            let d = (x as f32 - 2.0).powi(2) + (y as f32 - 2.0).powi(2);
            spatial[y * 5 + x] = (-d / 8.0).exp();
        }
    }
    // A compact 5x5 bilateral kernel preserves edges without a second image-sized allocation.
    exec.chunks(out, image.width, |offset, part| {
        for (i, p) in part.iter_mut().enumerate() {
            let index = offset + i;
            let x = index % image.width;
            let y = index / image.width;
            let center = image.rgb(index);
            let mut sum = [0.0; 3];
            let mut total = 0.0;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let c = image.rgb_at(x as isize + dx, y as isize + dy);
                    let diff = (0..3)
                        .map(|d| (c[d] as f32 - center[d] as f32).powi(2))
                        .sum::<f32>();
                    let wt = spatial[((dy + 2) * 5 + dx + 2) as usize]
                        * (-diff / (2.0 * 40.0 * 40.0)).exp();
                    total += wt;
                    for d in 0..3 {
                        sum[d] += wt * c[d] as f32;
                    }
                }
            }
            let rgb = sum.map(|v| (v / total).round() as u8);
            *p = [rgb[2], rgb[1], rgb[0], 255];
        }
    });
}

pub fn gaussian(
    input: &[f32],
    output: &mut [f32],
    scratch: &mut [f32],
    w: usize,
    h: usize,
    sigma: f32,
    exec: Execution,
) {
    if sigma <= 0.0 {
        output.copy_from_slice(input);
        return;
    }
    let radius = (sigma * 3.0).ceil().clamp(1.0, 48.0) as usize;
    let mut kernel = [0.0; 97];
    let mut sum = 0.0;
    for (i, v) in kernel[..radius * 2 + 1].iter_mut().enumerate() {
        *v = (-((i as f32 - radius as f32) / sigma).powi(2) / 2.0).exp();
        sum += *v;
    }
    for v in &mut kernel[..radius * 2 + 1] {
        *v /= sum;
    }
    exec.chunks(scratch, w, |offset, part| {
        for (i, p) in part.iter_mut().enumerate() {
            let x = (offset + i) % w;
            let y = (offset + i) / w;
            *p = kernel[..radius * 2 + 1]
                .iter()
                .enumerate()
                .map(|(j, &v)| {
                    input[y * w
                        + (x as isize + j as isize - radius as isize).clamp(0, w as isize - 1)
                            as usize]
                        * v
                })
                .sum();
        }
    });
    exec.chunks(output, w, |offset, part| {
        for (i, p) in part.iter_mut().enumerate() {
            let x = (offset + i) % w;
            let y = (offset + i) / w;
            *p = kernel[..radius * 2 + 1]
                .iter()
                .enumerate()
                .map(|(j, &v)| {
                    scratch[(y as isize + j as isize - radius as isize).clamp(0, h as isize - 1)
                        as usize
                        * w
                        + x]
                        * v
                })
                .sum();
        }
    });
}

/// Linear-time rectangular morphology; deque storage is bounded on the stack.
#[allow(clippy::too_many_arguments)] // Explicit input/output/scratch slices avoid hidden allocation.
pub fn morphology(
    input: &[f32],
    out: &mut [f32],
    temp: &mut [f32],
    w: usize,
    h: usize,
    radius: usize,
    dilate: bool,
    exec: Execution,
) {
    if radius == 0 {
        out.copy_from_slice(input);
        return;
    }
    extrema_rows(input, temp, w, radius, dilate, exec);
    transpose(temp, out, w, h, exec);
    extrema_rows(out, temp, h, radius, dilate, exec);
    transpose(temp, out, h, w, exec);
}

fn extrema_rows(
    input: &[f32],
    out: &mut [f32],
    w: usize,
    radius: usize,
    max: bool,
    exec: Execution,
) {
    exec.chunks(out, w, |offset, part| {
        for (row, target) in part.chunks_mut(w).enumerate() {
            let src = &input[offset + row * w..offset + (row + 1) * w];
            let mut queue = [0usize; MAX_SIDE];
            let mut head = 0;
            let mut tail = 0;
            let mut right = 0;
            for (x, p) in target.iter_mut().enumerate() {
                let end = x.saturating_add(radius).min(w - 1);
                while right <= end {
                    while tail > head
                        && if max {
                            src[queue[tail - 1]] <= src[right]
                        } else {
                            src[queue[tail - 1]] >= src[right]
                        }
                    {
                        tail -= 1;
                    }
                    queue[tail] = right;
                    tail += 1;
                    right += 1;
                }
                while queue[head] < x.saturating_sub(radius) {
                    head += 1;
                }
                *p = src[queue[head]];
            }
        }
    });
}

fn transpose(src: &[f32], out: &mut [f32], w: usize, h: usize, exec: Execution) {
    exec.chunks(out, h, |offset, part| {
        for (i, p) in part.iter_mut().enumerate() {
            let index = offset + i;
            *p = src[(index % h) * w + index / h];
        }
    });
}

pub fn histogram(input: &[f32]) -> [usize; 256] {
    let mut hist = [0usize; 256];
    for &v in input {
        hist[(v.clamp(0.0, 1.0) * 255.0).round() as usize] += 1;
    }
    hist
}

pub fn otsu(hist: &[usize; 256]) -> f32 {
    let total = hist.iter().sum::<usize>() as f64;
    let sum = hist
        .iter()
        .enumerate()
        .map(|(i, &n)| i as f64 * n as f64)
        .sum::<f64>();
    let mut count = 0.0;
    let mut partial = 0.0;
    let mut best = -1.0;
    let mut threshold = 127;
    for (i, &n) in hist.iter().enumerate() {
        count += n as f64;
        partial += i as f64 * n as f64;
        if count == 0.0 || count == total {
            continue;
        }
        let delta = partial / count - (sum - partial) / (total - count);
        let score = count * (total - count) * delta * delta;
        if score > best {
            best = score;
            threshold = i;
        }
    }
    threshold as f32 / 255.0
}

pub fn auto_levels(hist: &[usize; 256]) -> (f32, f32) {
    let total = hist[..250].iter().sum::<usize>();
    if total < 100 {
        return (0.0, 1.0);
    }
    let mut count = 0;
    let mut black = 0;
    let mut white = 255;
    for (i, &n) in hist[..250].iter().enumerate() {
        count += n;
        if count <= total / 100 {
            black = i;
        }
        if count >= total * 99 / 100 {
            white = i;
            break;
        }
    }
    if white - black < 8 {
        (0.0, 1.0)
    } else {
        (black as f32 / 255.0, white as f32 / 255.0)
    }
}

pub fn clean_components(
    mask: &mut [u8],
    visited: &mut [u8],
    queue: &mut Vec<u32>,
    w: usize,
    h: usize,
    min_area: usize,
    fill_holes: bool,
) {
    assert!(mask.len() <= u32::MAX as usize);
    if min_area == 0 {
        return;
    }
    visited.fill(0);
    for start in 0..mask.len() {
        let target = if fill_holes { 0 } else { 1 };
        if mask[start] != target || visited[start] != 0 {
            continue;
        }
        queue.clear();
        queue.push(start as u32);
        visited[start] = 1;
        let mut cursor = 0;
        let mut border = false;
        while cursor < queue.len() {
            let p = queue[cursor] as usize;
            cursor += 1;
            let x = p % w;
            let y = p / w;
            border |= x == 0 || y == 0 || x + 1 == w || y + 1 == h;
            for dy in -1isize..=1 {
                for dx in -1isize..=1 {
                    if (dx == 0 && dy == 0) || (fill_holes && dx != 0 && dy != 0) {
                        continue;
                    }
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let next = ny as usize * w + nx as usize;
                    if visited[next] == 0 && mask[next] == target {
                        visited[next] = 1;
                        queue.push(next as u32);
                    }
                }
            }
        }
        if queue.len() < min_area && (!fill_holes || !border) {
            for &p in queue.iter() {
                mask[p as usize] = 1 - target;
            }
        }
    }
}

pub fn fill_holes(mask: &mut [f32], visited: &mut [u8], queue: &mut Vec<u32>, w: usize, h: usize) {
    assert!(mask.len() <= u32::MAX as usize);
    visited.fill(0);
    queue.clear();
    for i in 0..w * h {
        if (i < w || i >= w * (h - 1) || i % w == 0 || i % w == w - 1)
            && mask[i] < 0.5
            && visited[i] == 0
        {
            visited[i] = 1;
            queue.push(i as u32);
        }
    }
    let mut cursor = 0;
    while cursor < queue.len() {
        let i = queue[cursor] as usize;
        cursor += 1;
        let x = i % w;
        let y = i / w;
        for (nx, ny) in [
            (x.saturating_sub(1), y),
            ((x + 1).min(w - 1), y),
            (x, y.saturating_sub(1)),
            (x, (y + 1).min(h - 1)),
        ] {
            let n = ny * w + nx;
            if mask[n] < 0.5 && visited[n] == 0 {
                visited[n] = 1;
                queue.push(n as u32);
            }
        }
    }
    for (i, p) in mask.iter_mut().enumerate() {
        if visited[i] == 0 {
            *p = 1.0;
        }
    }
}
