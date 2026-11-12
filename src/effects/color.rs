use super::ImageView;

pub fn rgb_to_lab(rgb: [u8; 3]) -> [f32; 3] {
    let linear = rgb.map(|v| {
        let x = v as f32 / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    });
    rgb_linear_to_lab(linear)
}

pub fn rgb_linear_to_lab([r, g, b]: [f32; 3]) -> [f32; 3] {
    let f = |t: f32| {
        if t > 0.008856452 {
            t.cbrt()
        } else {
            7.787037 * t + 16.0 / 116.0
        }
    };
    let x = f((0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047);
    let y = f(0.2126729 * r + 0.7151522 * g + 0.072175 * b);
    let z = f((0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883);
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}

pub fn lab_to_rgb([l, a, b]: [f32; 3]) -> [u8; 3] {
    let fy = (l + 16.0) / 116.0;
    let inv = |x: f32| {
        if x > 6.0 / 29.0 {
            x * x * x
        } else {
            (x - 16.0 / 116.0) / 7.787037
        }
    };
    let x = 0.95047 * inv(fy + a / 500.0);
    let y = inv(fy);
    let z = 1.08883 * inv(fy - b / 200.0);
    [
        3.2404542 * x - 1.5371385 * y - 0.4985314 * z,
        -0.969266 * x + 1.8760108 * y + 0.041556 * z,
        0.0556434 * x - 0.2040259 * y + 1.0572252 * z,
    ]
    .map(|v| {
        let s = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.max(0.0).powf(1.0 / 2.4) - 0.055
        };
        (s.clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

#[inline]
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

#[inline]
pub fn luma([r, g, b]: [u8; 3]) -> f32 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
}

pub fn is_lineart(image: ImageView<'_>) -> bool {
    let mut hues = [0usize; 180];
    let mut colorful = 0;
    for i in 0..image.len() {
        let [r, g, b] = image.rgb(i).map(|c| c as f32);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        if max <= 40.0 || d / max <= 45.0 / 255.0 {
            continue;
        }
        let hue = if max == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        hues[((hue * 30.0) as usize).min(179)] += 1;
        colorful += 1;
    }
    if colorful as f32 / (image.len() as f32) < 0.03 {
        return true;
    }
    (0..180).any(|h| {
        (0..25).map(|d| hues[(h + d) % 180]).sum::<usize>() as f32 >= colorful as f32 * 0.93
    })
}

/// resvg emits premultiplied RGBA; GPUI's RenderImage contract requires straight BGRA.
#[inline]
pub fn premultiplied_rgba_to_bgra([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    if a == 0 {
        return [0; 4];
    }
    if a == 255 {
        return [b, g, r, a];
    }
    let unpremultiply = |v: u8| ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
    [unpremultiply(b), unpremultiply(g), unpremultiply(r), a]
}
