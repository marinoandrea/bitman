//! SIMD first: 4 points per NEON/SSE2 vector; no alignment or padding requirements.
//! Every unsafe block accesses only validated, complete vectors. Tails are scalar.
use super::{ImageView, cluster::nearest};

pub const fn backend() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "NEON"
    } else if cfg!(target_arch = "x86_64") {
        "SSE2"
    } else {
        "scalar"
    }
}

pub fn assign(points: &[[f32; 3]], centers: &[[f32; 3]], out: &mut [u8]) {
    assert_eq!(points.len(), out.len());
    assert!(!centers.is_empty() && centers.len() <= 32);
    let done;
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is mandatory on aarch64 and all vector loads are length checked below.
        done = unsafe { neon_assign(points, centers, out) };
    }
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: SSE2 is mandatory on x86_64.
        done = unsafe { sse_assign(points, centers, out) };
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        done = 0;
    }
    for i in done..out.len() {
        out[i] = nearest(points[i], centers).0 as u8;
    }
}

pub fn assign_indexed(points: &[[f32; 3]], order: &[usize], centers: &[[f32; 3]], out: &mut [u8]) {
    assert_eq!(order.len(), out.len());
    for (ids, labels) in order.chunks(16).zip(out.chunks_mut(16)) {
        let mut batch = [[0.0; 3]; 16];
        for (i, &id) in ids.iter().enumerate() {
            batch[i] = points[id];
        }
        assign(&batch[..ids.len()], centers, labels);
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn neon_assign(points: &[[f32; 3]], centers: &[[f32; 3]], out: &mut [u8]) -> usize {
    use std::arch::aarch64::*;
    let wide = out.len() / 16 * 16;
    // Four independent vectors hide the nearest-centroid reduction latency.
    // SAFETY: each load covers four complete triples; the narrow store covers 16 labels.
    unsafe {
        for i in (0..wide).step_by(16) {
            let p0 = vld3q_f32(points.as_ptr().add(i).cast());
            let p1 = vld3q_f32(points.as_ptr().add(i + 4).cast());
            let p2 = vld3q_f32(points.as_ptr().add(i + 8).cast());
            let p3 = vld3q_f32(points.as_ptr().add(i + 12).cast());
            let mut best = [vdupq_n_f32(f32::INFINITY); 4];
            let mut ids = [vdupq_n_u32(0); 4];
            for (c, center) in centers.iter().enumerate() {
                let cx = vdupq_n_f32(center[0]);
                let cy = vdupq_n_f32(center[1]);
                let cz = vdupq_n_f32(center[2]);
                let ci = vdupq_n_u32(c as u32);
                for (j, p) in [p0, p1, p2, p3].into_iter().enumerate() {
                    let x = vsubq_f32(p.0, cx);
                    let y = vsubq_f32(p.1, cy);
                    let z = vsubq_f32(p.2, cz);
                    let d = vaddq_f32(vaddq_f32(vmulq_f32(x, x), vmulq_f32(y, y)), vmulq_f32(z, z));
                    let closer = vcltq_f32(d, best[j]);
                    best[j] = vminq_f32(d, best[j]);
                    ids[j] = vbslq_u32(closer, ci, ids[j]);
                }
            }
            let lo = vmovn_u16(vcombine_u16(vmovn_u32(ids[0]), vmovn_u32(ids[1])));
            let hi = vmovn_u16(vcombine_u16(vmovn_u32(ids[2]), vmovn_u32(ids[3])));
            vst1q_u8(out.as_mut_ptr().add(i), vcombine_u8(lo, hi));
        }
    }
    let n = out.len() / 4 * 4;
    // SAFETY: the remaining complete groups load four triples and store four local lanes.
    unsafe {
        for i in (wide..n).step_by(4) {
            let p = vld3q_f32(points.as_ptr().add(i).cast());
            let mut best = vdupq_n_f32(f32::INFINITY);
            let mut ids = vdupq_n_u32(0);
            for (c, center) in centers.iter().enumerate() {
                let x = vsubq_f32(p.0, vdupq_n_f32(center[0]));
                let y = vsubq_f32(p.1, vdupq_n_f32(center[1]));
                let z = vsubq_f32(p.2, vdupq_n_f32(center[2]));
                let d = vaddq_f32(vaddq_f32(vmulq_f32(x, x), vmulq_f32(y, y)), vmulq_f32(z, z));
                let closer = vcltq_f32(d, best);
                best = vminq_f32(d, best);
                ids = vbslq_u32(closer, vdupq_n_u32(c as u32), ids);
            }
            let mut lanes = [0u32; 4];
            vst1q_u32(lanes.as_mut_ptr(), ids);
            for j in 0..4 {
                out[i + j] = lanes[j] as u8;
            }
        }
    }
    n
}

#[cfg(target_arch = "x86_64")]
unsafe fn sse_assign(points: &[[f32; 3]], centers: &[[f32; 3]], out: &mut [u8]) -> usize {
    use std::arch::x86_64::*;
    let n = out.len() / 4 * 4;
    // SAFETY: slice indices are bounded by n, and the unaligned store writes 4 u32s.
    unsafe {
        for i in (0..n).step_by(4) {
            let x = _mm_setr_ps(
                points[i][0],
                points[i + 1][0],
                points[i + 2][0],
                points[i + 3][0],
            );
            let y = _mm_setr_ps(
                points[i][1],
                points[i + 1][1],
                points[i + 2][1],
                points[i + 3][1],
            );
            let z = _mm_setr_ps(
                points[i][2],
                points[i + 1][2],
                points[i + 2][2],
                points[i + 3][2],
            );
            let mut best = _mm_set1_ps(f32::INFINITY);
            let mut ids = _mm_setzero_si128();
            for (c, center) in centers.iter().enumerate() {
                let a = _mm_sub_ps(x, _mm_set1_ps(center[0]));
                let b = _mm_sub_ps(y, _mm_set1_ps(center[1]));
                let d = _mm_sub_ps(z, _mm_set1_ps(center[2]));
                let dist = _mm_add_ps(
                    _mm_add_ps(_mm_mul_ps(a, a), _mm_mul_ps(b, b)),
                    _mm_mul_ps(d, d),
                );
                let mask = _mm_cmplt_ps(dist, best);
                best = _mm_or_ps(_mm_and_ps(mask, dist), _mm_andnot_ps(mask, best));
                let m = _mm_castps_si128(mask);
                ids = _mm_or_si128(
                    _mm_and_si128(m, _mm_set1_epi32(c as i32)),
                    _mm_andnot_si128(m, ids),
                );
            }
            let mut lanes = [0u32; 4];
            _mm_storeu_si128(lanes.as_mut_ptr().cast(), ids);
            for j in 0..4 {
                out[i + j] = lanes[j] as u8;
            }
        }
    }
    n
}

pub fn tone_in_place(values: &mut [f32], contrast: f32, brightness: f32, invert: bool) {
    let mut done = 0;
    #[cfg(target_arch = "aarch64")]
    // SAFETY: NEON exists on aarch64 and each iteration reads and writes 4 in-bounds floats.
    unsafe {
        use std::arch::aarch64::*;
        let zero = vdupq_n_f32(0.0);
        let one = vdupq_n_f32(1.0);
        let c = vdupq_n_f32(contrast);
        let b = vdupq_n_f32(brightness);
        while done + 4 <= values.len() {
            let p = values.as_mut_ptr().add(done);
            let mut v = vminq_f32(
                one,
                vmaxq_f32(zero, vsubq_f32(vmulq_f32(vld1q_f32(p), c), b)),
            );
            if invert {
                v = vsubq_f32(one, v);
            }
            vst1q_f32(p, v);
            done += 4;
        }
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is mandatory; unaligned loads/stores use complete vectors only.
    unsafe {
        use std::arch::x86_64::*;
        let zero = _mm_setzero_ps();
        let one = _mm_set1_ps(1.0);
        let c = _mm_set1_ps(contrast);
        let b = _mm_set1_ps(brightness);
        while done + 4 <= values.len() {
            let p = values.as_mut_ptr().add(done);
            let mut v = _mm_min_ps(
                one,
                _mm_max_ps(zero, _mm_sub_ps(_mm_mul_ps(_mm_loadu_ps(p), c), b)),
            );
            if invert {
                v = _mm_sub_ps(one, v);
            }
            _mm_storeu_ps(p, v);
            done += 4;
        }
    }
    for v in &mut values[done..] {
        *v = (*v * contrast - brightness).clamp(0.0, 1.0);
        if invert {
            *v = 1.0 - *v;
        }
    }
}

pub fn grayscale(image: ImageView<'_>, out: &mut [f32], offset: usize) {
    assert!(offset <= image.len() && out.len() <= image.len() - offset);
    #[cfg(target_arch = "aarch64")]
    let mut done = 0;
    #[cfg(not(target_arch = "aarch64"))]
    let done = 0;
    #[cfg(target_arch = "aarch64")]
    if image.stride == image.width * 4 {
        // SAFETY: the view validates the backing length; complete groups contain 8 BGRA pixels.
        unsafe {
            use std::arch::aarch64::*;
            while done + 8 <= out.len() {
                let pixels = vld4_u8(image.bytes.as_ptr().add((offset + done) * 4));
                let b = vmovl_u8(pixels.0);
                let g = vmovl_u8(pixels.1);
                let r = vmovl_u8(pixels.2);
                let a = vmovl_u8(pixels.3);
                for high in 0..2 {
                    let expand = |v: uint16x8_t| {
                        vcvtq_f32_u32(vmovl_u16(if high == 0 {
                            vget_low_u16(v)
                        } else {
                            vget_high_u16(v)
                        }))
                    };
                    let alpha = vmulq_n_f32(expand(a), 1.0 / 255.0);
                    let paper = vsubq_f32(vdupq_n_f32(255.0), expand(a));
                    // Composite transparency onto white before luminance conversion.
                    let rr = vrndnq_f32(vaddq_f32(vmulq_f32(expand(r), alpha), paper));
                    let gg = vrndnq_f32(vaddq_f32(vmulq_f32(expand(g), alpha), paper));
                    let bb = vrndnq_f32(vaddq_f32(vmulq_f32(expand(b), alpha), paper));
                    let value = vmulq_n_f32(
                        vaddq_f32(
                            vaddq_f32(vmulq_n_f32(rr, 0.299), vmulq_n_f32(gg, 0.587)),
                            vmulq_n_f32(bb, 0.114),
                        ),
                        1.0 / 255.0,
                    );
                    vst1q_f32(out.as_mut_ptr().add(done + high * 4), value);
                }
                done += 8;
            }
        }
    }
    for (i, v) in out[done..].iter_mut().enumerate() {
        *v = super::color::luma(image.rgb(offset + done + i));
    }
}
