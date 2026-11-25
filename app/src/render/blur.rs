use rayon::prelude::*;
use tiny_skia::{Color, Pixmap, PixmapPaint, Transform};

/// Three box blurs approximate a gaussian (Kovesi). Returns the half-widths of the boxes.
fn boxes(sigma: f32) -> [usize; 3] {
    let n = 3.;
    let w_ideal = (12. * sigma * sigma / n + 1.).sqrt();
    let mut wl = w_ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m_ideal = (12. * sigma * sigma - n * (wl * wl) as f32 - 4. * n * wl as f32 - 3. * n)
        / (-4. * wl as f32 - 4.);
    let m = m_ideal.round() as i32;
    let mut out = [0; 3];
    for (i, b) in out.iter_mut().enumerate() {
        let w = if (i as i32) < m { wl } else { wu };
        *b = ((w.max(1) - 1) / 2) as usize;
    }
    out
}

fn box_line(src: &[f32], dst: &mut [f32], r: usize, clamp: bool) {
    let n = src.len();
    if r == 0 || n == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let at = |i: isize| -> f32 {
        if i < 0 {
            if clamp { src[0] } else { 0. }
        } else if i as usize >= n {
            if clamp { src[n - 1] } else { 0. }
        } else {
            src[i as usize]
        }
    };
    let scale = 1. / (2 * r + 1) as f32;
    let r = r as isize;
    let mut acc: f32 = (-r..=r).map(at).sum();
    for i in 0..n as isize {
        dst[i as usize] = acc * scale;
        acc += at(i + r + 1) - at(i - r);
    }
}

/// Blurs a single-channel plane in place.
pub fn blur_plane(data: &mut [f32], w: usize, h: usize, sigma: f32, clamp: bool) {
    if sigma < 0.3 || w == 0 || h == 0 {
        return;
    }
    for r in boxes(sigma) {
        data.par_chunks_mut(w).for_each(|row| {
            let src = row.to_vec();
            box_line(&src, row, r, clamp);
        });
        let cols: Vec<Vec<f32>> = (0..w)
            .into_par_iter()
            .map(|x| {
                let src: Vec<f32> = (0..h).map(|y| data[y * w + x]).collect();
                let mut dst = vec![0.; h];
                box_line(&src, &mut dst, r, clamp);
                dst
            })
            .collect();
        data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, v) in row.iter_mut().enumerate() {
                *v = cols[x][y];
            }
        });
    }
}

/// Paints the blurred silhouette of `layer` (placed at `origin` on `target`) like
/// CGContext.setShadow: offset in device pixels, gaussian with the given sigma.
pub fn draw_shadow(
    target: &mut Pixmap,
    layer: &Pixmap,
    origin: (i32, i32),
    sigma: f32,
    offset: (f32, f32),
    color: Color,
) {
    if color.alpha() <= 0. {
        return;
    }
    let pad = (sigma * 3.).ceil().max(0.) as usize + 1;
    let (lw, lh) = (layer.width() as usize, layer.height() as usize);
    let (w, h) = (lw + pad * 2, lh + pad * 2);
    let mut plane = vec![0f32; w * h];
    let src = layer.data();
    for y in 0..lh {
        let row = &mut plane[(y + pad) * w + pad..(y + pad) * w + pad + lw];
        for (x, v) in row.iter_mut().enumerate() {
            *v = src[(y * lw + x) * 4 + 3] as f32 / 255.;
        }
    }
    blur_plane(&mut plane, w, h, sigma, false);
    let Some(mut shadow) = Pixmap::new(w as u32, h as u32) else {
        return;
    };
    let c = color.premultiply();
    let (r, g, b, a) = (c.red(), c.green(), c.blue(), c.alpha());
    for (px, v) in shadow.data_mut().chunks_exact_mut(4).zip(plane.iter()) {
        let v = v.clamp(0., 1.);
        px[0] = (r * v * 255. + 0.5) as u8;
        px[1] = (g * v * 255. + 0.5) as u8;
        px[2] = (b * v * 255. + 0.5) as u8;
        px[3] = (a * v * 255. + 0.5) as u8;
    }
    target.draw_pixmap(
        0,
        0,
        shadow.as_ref(),
        &PixmapPaint::default(),
        Transform::from_translate(
            origin.0 as f32 - pad as f32 + offset.0,
            origin.1 as f32 - pad as f32 + offset.1,
        ),
        None,
    );
}

/// Gaussian blur of a whole premultiplied image with edges extended, like CIGaussianBlur over
/// a clamped image.
pub fn blur_image(src: &Pixmap, sigma: f32) -> Pixmap {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let mut out = src.clone();
    let mut planes: Vec<Vec<f32>> = (0..4)
        .map(|c| {
            src.data()
                .iter()
                .skip(c)
                .step_by(4)
                .map(|&v| v as f32)
                .collect()
        })
        .collect();
    planes
        .par_iter_mut()
        .for_each(|p| blur_plane(p, w, h, sigma, true));
    for (i, px) in out.data_mut().chunks_exact_mut(4).enumerate() {
        let a = planes[3][i].clamp(0., 255.);
        for c in 0..3 {
            px[c] = planes[c][i].clamp(0., a) as u8;
        }
        px[3] = a as u8;
    }
    out
}
