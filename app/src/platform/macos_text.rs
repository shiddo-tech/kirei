use std::ffi::c_void;

use tiny_skia::{Path, PathBuilder};

type CfRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CgPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
struct CfRange {
    location: isize,
    length: isize,
}

#[repr(C)]
struct CgPathElement {
    kind: i32,
    points: *const CgPoint,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    fn CFRelease(cf: CfRef);
    fn CFStringCreateWithBytes(
        alloc: CfRef,
        bytes: *const u8,
        len: isize,
        encoding: u32,
        external: bool,
    ) -> CfRef;
    fn CFNumberCreate(alloc: CfRef, kind: isize, value: *const c_void) -> CfRef;
    fn CFDictionaryCreate(
        alloc: CfRef,
        keys: *const CfRef,
        values: *const CfRef,
        count: isize,
        key_cb: *const c_void,
        value_cb: *const c_void,
    ) -> CfRef;
    fn CFDictionaryGetValue(dict: CfRef, key: CfRef) -> CfRef;
    fn CFArrayGetCount(array: CfRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CfRef, index: isize) -> CfRef;
    fn CFAttributedStringCreate(alloc: CfRef, s: CfRef, attrs: CfRef) -> CfRef;
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    static kCTFontAttributeName: CfRef;
    static kCTFontTraitsAttribute: CfRef;
    static kCTFontWeightTrait: CfRef;
    fn CTFontCreateUIFontForLanguage(kind: u32, size: f64, language: CfRef) -> CfRef;
    fn CTFontDescriptorCreateWithAttributes(attrs: CfRef) -> CfRef;
    fn CTFontCreateCopyWithAttributes(
        font: CfRef,
        size: f64,
        matrix: *const c_void,
        descriptor: CfRef,
    ) -> CfRef;
    fn CTLineCreateWithAttributedString(s: CfRef) -> CfRef;
    fn CTLineGetTypographicBounds(
        line: CfRef,
        ascent: *mut f64,
        descent: *mut f64,
        leading: *mut f64,
    ) -> f64;
    fn CTLineGetGlyphRuns(line: CfRef) -> CfRef;
    fn CTRunGetAttributes(run: CfRef) -> CfRef;
    fn CTRunGetGlyphCount(run: CfRef) -> isize;
    fn CTRunGetGlyphs(run: CfRef, range: CfRange, out: *mut u16);
    fn CTRunGetPositions(run: CfRef, range: CfRange, out: *mut CgPoint);
    fn CTFontCreatePathForGlyph(font: CfRef, glyph: u16, matrix: *const c_void) -> CfRef;
    fn CTFontGetAscent(font: CfRef) -> f64;
    fn CTFontGetDescent(font: CfRef) -> f64;
    fn CTFontGetLeading(font: CfRef) -> f64;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPathApply(
        path: CfRef,
        info: *mut c_void,
        f: extern "C" fn(*mut c_void, *const CgPathElement),
    );
}

const UTF8: u32 = 0x0800_0100;
const CF_NUMBER_F64: isize = 13;
const UI_FONT_SYSTEM: u32 = 2;

/// Maps CSS-like weights to NSFont.Weight values.
fn ct_weight(weight: f32) -> f64 {
    match weight as i32 {
        ..=450 => 0.,
        451..=650 => 0.3,
        651..=750 => 0.4,
        _ => 0.56,
    }
}

struct Font(CfRef);

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn system_font(size: f64, weight: f32) -> Option<Font> {
    unsafe {
        let base = CTFontCreateUIFontForLanguage(UI_FONT_SYSTEM, size, std::ptr::null());
        if base.is_null() {
            return None;
        }
        let w = ct_weight(weight);
        let num = CFNumberCreate(std::ptr::null(), CF_NUMBER_F64, (&w as *const f64).cast());
        let traits = CFDictionaryCreate(
            std::ptr::null(),
            &kCTFontWeightTrait,
            &num,
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        let attrs = CFDictionaryCreate(
            std::ptr::null(),
            &kCTFontTraitsAttribute,
            &traits,
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        let desc = CTFontDescriptorCreateWithAttributes(attrs);
        let font = CTFontCreateCopyWithAttributes(base, size, std::ptr::null(), desc);
        for r in [desc, attrs, traits, num, base] {
            CFRelease(r);
        }
        (!font.is_null()).then_some(Font(font))
    }
}

struct Line(CfRef);

impl Drop for Line {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn line(text: &str, font: &Font) -> Option<Line> {
    unsafe {
        let s = CFStringCreateWithBytes(
            std::ptr::null(),
            text.as_ptr(),
            text.len() as isize,
            UTF8,
            false,
        );
        if s.is_null() {
            return None;
        }
        let attrs = CFDictionaryCreate(
            std::ptr::null(),
            &kCTFontAttributeName,
            &font.0,
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        let attributed = CFAttributedStringCreate(std::ptr::null(), s, attrs);
        let line = CTLineCreateWithAttributedString(attributed);
        for r in [attributed, attrs, s] {
            CFRelease(r);
        }
        (!line.is_null()).then_some(Line(line))
    }
}

fn line_metrics(font: &Font) -> (f64, f64) {
    unsafe {
        let ascent = CTFontGetAscent(font.0);
        let height = ascent + CTFontGetDescent(font.0) + CTFontGetLeading(font.0);
        (ascent, height)
    }
}

pub fn measure(text: &str, size: f64, weight: f32) -> Option<(f64, f64)> {
    let font = system_font(size, weight)?;
    let (_, line_h) = line_metrics(&font);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut w: f64 = 0.;
    for l in &lines {
        if l.is_empty() {
            continue;
        }
        let line = line(l, &font)?;
        let lw = unsafe {
            CTLineGetTypographicBounds(
                line.0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        w = w.max(lw);
    }
    Some((w, line_h * lines.len() as f64))
}

struct Sink {
    pb: PathBuilder,
    dx: f64,
    baseline: f64,
}

extern "C" fn apply(info: *mut c_void, el: *const CgPathElement) {
    let sink = unsafe { &mut *(info as *mut Sink) };
    let el = unsafe { &*el };
    let p = |i: usize| {
        let pt = unsafe { *el.points.add(i) };
        ((sink.dx + pt.x) as f32, (sink.baseline - pt.y) as f32)
    };
    match el.kind {
        0 => {
            let (x, y) = p(0);
            sink.pb.move_to(x, y);
        }
        1 => {
            let (x, y) = p(0);
            sink.pb.line_to(x, y);
        }
        2 => {
            let ((x1, y1), (x, y)) = (p(0), p(1));
            sink.pb.quad_to(x1, y1, x, y);
        }
        3 => {
            let ((x1, y1), (x2, y2), (x, y)) = (p(0), p(1), p(2));
            sink.pb.cubic_to(x1, y1, x2, y2, x, y);
        }
        4 => sink.pb.close(),
        _ => {}
    }
}

/// Glyph outlines with the text box's top-left corner at the origin. CoreText picks fallback
/// fonts per character, like NSString drawing in the Swift app.
pub fn path(text: &str, size: f64, weight: f32) -> Option<Path> {
    let font = system_font(size, weight)?;
    let (ascent, line_h) = line_metrics(&font);
    let mut sink = Sink {
        pb: PathBuilder::new(),
        dx: 0.,
        baseline: 0.,
    };
    for (i, l) in text.split('\n').enumerate() {
        if l.is_empty() {
            continue;
        }
        let line = line(l, &font)?;
        let baseline = ascent + line_h * i as f64;
        unsafe {
            let runs = CTLineGetGlyphRuns(line.0);
            for r in 0..CFArrayGetCount(runs) {
                let run = CFArrayGetValueAtIndex(runs, r);
                let run_font = CFDictionaryGetValue(CTRunGetAttributes(run), kCTFontAttributeName);
                let n = CTRunGetGlyphCount(run);
                if run_font.is_null() || n <= 0 {
                    continue;
                }
                let mut glyphs = vec![0u16; n as usize];
                let mut positions = vec![CgPoint::default(); n as usize];
                let all = || CfRange {
                    location: 0,
                    length: 0,
                };
                CTRunGetGlyphs(run, all(), glyphs.as_mut_ptr());
                CTRunGetPositions(run, all(), positions.as_mut_ptr());
                for (g, pos) in glyphs.iter().zip(&positions) {
                    let gp = CTFontCreatePathForGlyph(run_font, *g, std::ptr::null());
                    if gp.is_null() {
                        continue;
                    }
                    sink.dx = pos.x;
                    sink.baseline = baseline - pos.y;
                    CGPathApply(gp, (&mut sink as *mut Sink).cast(), apply);
                    CFRelease(gp);
                }
            }
        }
    }
    sink.pb.finish()
}
