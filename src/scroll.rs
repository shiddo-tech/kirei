use tiny_skia::{IntRect, Pixmap, PixmapPaint, Transform};

/// One captured frame of a scrolling capture with a hash per row.
pub struct Frame {
    pub image: Pixmap,
    pub hashes: Vec<u64>,
}

impl Frame {
    pub fn new(image: Pixmap) -> Self {
        let hashes = row_hashes(&image);
        Self { image, hashes }
    }
}

pub fn row_hashes(pm: &Pixmap) -> Vec<u64> {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    // Skip the right edge, where overlay scrollbars fade in and out.
    let usable = (w as i64 - (w as f64 * 0.03) as i64 - 16).max(1) as usize * 4;
    let data = pm.data();
    (0..h)
        .map(|y| {
            let row = &data[y * w * 4..y * w * 4 + usable];
            row.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, b| {
                (hash ^ *b as u64).wrapping_mul(0x100_0000_01b3)
            })
        })
        .collect()
}

/// How far the content moved up between two frames, plus the fixed bands at the top and bottom.
/// Frames are matched by exact row hashes, which works because scrolled pixels are identical.
pub fn matching(a: &[u64], b: &[u64]) -> Option<(usize, usize, usize)> {
    let h = a.len().min(b.len());
    if h <= 40 {
        return None;
    }
    let mut top = 0;
    while top < h && a[top] == b[top] {
        top += 1;
    }
    if top == h {
        return None;
    }
    let mut bottom = 0;
    while bottom < h - top && a[h - 1 - bottom] == b[h - 1 - bottom] {
        bottom += 1;
    }
    let (start, end) = (top, h - bottom);
    if end - start <= 30 {
        return None;
    }
    // Only rows that differ from the row above say anything about position: runs of identical
    // rows (blank space, solid bars) match at many shifts.
    let edges: Vec<usize> = (start + 1..end).filter(|&j| b[j] != b[j - 1]).collect();
    let mut best: Option<(usize, f64)> = None;
    for shift in 1..(end - start).saturating_sub(8) {
        let (mut hits, mut total) = (0, 0);
        for &j in edges.iter().filter(|&&j| j + shift < end) {
            total += 1;
            if a[j + shift] == b[j] {
                hits += 1;
            }
        }
        if total < 4 {
            break;
        }
        let score = hits as f64 / total as f64;
        if score > best.map(|b| b.1).unwrap_or(0.) + 1e-9 {
            best = Some((shift, score));
        }
    }
    let (shift, score) = best?;
    (score >= 0.9).then_some((shift, top, bottom))
}

/// Accumulates frames into one tall image. Each frame is placed at its scroll offset and later
/// frames paint over earlier ones, so a fixed footer is covered by the next frame's content and
/// only the last one survives. Fixed-band sizes can be overestimated when blank rows happen to
/// line up; the margin keeps that from losing rows.
#[derive(Default)]
pub struct Stitcher {
    first: Option<Pixmap>,
    previous: Vec<u64>,
    slices: Vec<(Pixmap, usize)>,
    offset: usize,
    frame_height: usize,
}

const MAX_HEIGHT: usize = 30_000;
const MARGIN: usize = 80;

impl Stitcher {
    pub fn height(&self) -> usize {
        self.offset + self.frame_height
    }

    /// Returns true when the frame moved the content.
    pub fn add(&mut self, frame: Frame) -> bool {
        let h = frame.hashes.len();
        if self.first.is_none() {
            self.first = Some(frame.image);
            self.previous = frame.hashes;
            self.frame_height = h;
            return true;
        }
        if h != self.frame_height {
            return false;
        }
        let Some((shift, top, bottom)) = matching(&self.previous, &frame.hashes) else {
            return false;
        };
        if shift == 0 || self.offset + shift + h >= MAX_HEIGHT {
            return false;
        }
        self.offset += shift;
        let start =
            top.max((h as i64 - bottom as i64 - shift as i64 - MARGIN as i64).max(0) as usize);
        if start >= h {
            return false;
        }
        let Some(slice) =
            IntRect::from_xywh(0, start as i32, frame.image.width(), (h - start) as u32)
                .and_then(|r| frame.image.clone_rect(r))
        else {
            return false;
        };
        self.slices.push((slice, self.offset + start));
        self.previous = frame.hashes;
        true
    }

    pub fn result(&self) -> Option<Pixmap> {
        let first = self.first.as_ref()?;
        let mut out = Pixmap::new(first.width(), self.height() as u32)?;
        let paint = PixmapPaint::default();
        out.draw_pixmap(0, 0, first.as_ref(), &paint, Transform::identity(), None);
        for (slice, at) in &self.slices {
            out.draw_pixmap(
                0,
                *at as i32,
                slice.as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        Some(out)
    }
}

/// `--scroll-test`: simulates a scroll with a sticky header and footer over a tall image and
/// stitches the frames back together, printing the same lines as the Swift app.
pub fn self_test(src: &Pixmap) -> Option<Pixmap> {
    let (w, sh) = (src.width(), src.height() as usize);
    let view_h = (sh / 2).min(700);
    let (header, footer) = (60usize, 40usize);
    let content_h = view_h - header - footer;
    let mut stitcher = Stitcher::default();
    let mut offset = 0usize;
    let mut prev: Option<Vec<u64>> = None;
    let fill = |pm: &mut Pixmap, y: usize, h: usize, c: (f32, f32, f32)| {
        let mut p = tiny_skia::Paint::default();
        p.set_color(tiny_skia::Color::from_rgba(c.0, c.1, c.2, 1.).unwrap());
        if let Some(r) = tiny_skia::Rect::from_xywh(0., y as f32, w as f32, h as f32) {
            pm.fill_rect(r, &p, Transform::identity(), None);
        }
    };
    for step in [0, 37, 120, 120, 0, 5, 300, 64, 220, 10, 400, 333] {
        offset = (offset + step).min(sh - content_h);
        let mut frame = Pixmap::new(w, view_h as u32)?;
        let content = src.clone_rect(IntRect::from_xywh(0, offset as i32, w, content_h as u32)?)?;
        fill(&mut frame, 0, header, (0.1, 0.4, 0.9));
        fill(&mut frame, view_h - footer, footer, (0.9, 0.3, 0.2));
        frame.draw_pixmap(
            0,
            header as i32,
            content.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        let frame = Frame::new(frame);
        let m = prev.as_ref().and_then(|p| matching(p, &frame.hashes));
        let hashes = frame.hashes.clone();
        let added = stitcher.add(frame);
        if added {
            prev = Some(hashes);
        }
        println!(
            "offset {offset} match {} added {added} height {}",
            m.map(|(a, b, c)| format!("({a}, {b}, {c})"))
                .unwrap_or("-".into()),
            stitcher.height()
        );
    }
    let out = stitcher.result()?;
    println!(
        "expected {} got {}",
        offset + content_h + header + footer,
        out.height()
    );
    Some(out)
}
