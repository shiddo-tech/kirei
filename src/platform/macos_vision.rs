use std::ffi::c_void;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSRange, NSRect, NSString};

type Id = *mut AnyObject;

#[link(name = "Vision", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGColorSpaceCreateWithName(name: *const c_void) -> *const c_void;
    static kCGColorSpaceSRGB: *const c_void;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        w: usize,
        h: usize,
        bpc: usize,
        bpr: usize,
        space: *const c_void,
        info: u32,
    ) -> *mut c_void;
    fn CGBitmapContextCreateImage(ctx: *mut c_void) -> *mut c_void;
    fn CGContextRelease(ctx: *mut c_void);
    fn CGColorSpaceRelease(space: *const c_void);
    fn CGImageRelease(image: *mut c_void);
}

/// One recognized line: text plus a way to get the box of a UTF-16 range in it.
pub struct Line {
    pub text: String,
    /// Normalized box (origin bottom-left), like Vision returns it.
    pub bbox: [f64; 4],
    candidate: Id,
}

impl Line {
    /// Normalized box (origin bottom-left) of a UTF-16 range of the text.
    pub fn range_box(&self, start: usize, len: usize) -> Option<[f64; 4]> {
        unsafe {
            let mut err: Id = std::ptr::null_mut();
            let range = NSRange::new(start, len);
            let obs: Id = msg_send![self.candidate, boundingBoxForRange: range, error: &mut err];
            if obs.is_null() {
                return None;
            }
            let r: NSRect = msg_send![obs, boundingBox];
            Some([r.origin.x, r.origin.y, r.size.width, r.size.height])
        }
    }
}

pub struct Recognition {
    pub lines: Vec<Line>,
    _pool: Vec<Id>,
}

impl Drop for Recognition {
    fn drop(&mut self) {
        unsafe {
            for o in &self._pool {
                let _: () = msg_send![*o, release];
            }
        }
    }
}

fn cg_image(pm: &tiny_skia::Pixmap) -> Option<*mut c_void> {
    unsafe {
        let mut buf = pm.data().to_vec();
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        let ctx = CGBitmapContextCreate(buf.as_mut_ptr().cast(), w, h, 8, w * 4, space, 1);
        CGColorSpaceRelease(space);
        if ctx.is_null() {
            return None;
        }
        let img = CGBitmapContextCreateImage(ctx);
        CGContextRelease(ctx);
        (!img.is_null()).then_some(img)
    }
}

/// Runs VNRecognizeTextRequest (accurate, with language correction) on the image.
pub fn recognize(pm: &tiny_skia::Pixmap) -> Recognition {
    let mut out = Recognition {
        lines: Vec::new(),
        _pool: Vec::new(),
    };
    let Some(img) = cg_image(pm) else {
        return out;
    };
    unsafe {
        let request: Id = msg_send![class!(VNRecognizeTextRequest), new];
        let _: () = msg_send![request, setRecognitionLevel: 0isize];
        let _: () = msg_send![request, setUsesLanguageCorrection: true];
        let langs: Vec<objc2::rc::Retained<NSString>> = ["pt-BR", "en-US", "es-ES"]
            .iter()
            .map(|s| NSString::from_str(s))
            .collect();
        let refs: Vec<&NSString> = langs.iter().map(|s| &**s).collect();
        let array = objc2_foundation::NSArray::from_slice(&refs);
        let _: () = msg_send![request, setRecognitionLanguages: &*array];
        let responds: bool =
            msg_send![request, respondsToSelector: objc2::sel!(setAutomaticallyDetectsLanguage:)];
        if responds {
            let _: () = msg_send![request, setAutomaticallyDetectsLanguage: true];
        }
        let dict: Id = msg_send![class!(NSDictionary), dictionary];
        let alloc: Id = msg_send![class!(VNImageRequestHandler), alloc];
        let handler: Id = msg_send![alloc, initWithCGImage: img, options: dict];
        let requests: Id = msg_send![class!(NSArray), arrayWithObject: request];
        let mut err: Id = std::ptr::null_mut();
        let _: bool = msg_send![handler, performRequests: requests, error: &mut err];
        let results: Id = msg_send![request, results];
        if !results.is_null() {
            let n: usize = msg_send![results, count];
            for i in 0..n {
                let obs: Id = msg_send![results, objectAtIndex: i];
                let cands: Id = msg_send![obs, topCandidates: 1usize];
                let count: usize = msg_send![cands, count];
                if count == 0 {
                    continue;
                }
                let cand: Id = msg_send![cands, objectAtIndex: 0usize];
                let s: *const NSString = msg_send![cand, string];
                let Some(s) = s.as_ref() else { continue };
                let r: NSRect = msg_send![obs, boundingBox];
                let _: Id = msg_send![cand, retain];
                out._pool.push(cand);
                out.lines.push(Line {
                    text: s.to_string(),
                    bbox: [r.origin.x, r.origin.y, r.size.width, r.size.height],
                    candidate: cand,
                });
            }
        }
        let _: () = msg_send![handler, release];
        let _: () = msg_send![request, release];
        CGImageRelease(img);
    }
    out
}

/// Phone numbers found by NSDataDetector, as UTF-16 ranges.
pub fn phone_ranges(text: &str) -> Vec<(usize, usize)> {
    unsafe {
        let mut err: Id = std::ptr::null_mut();
        let detector: Id =
            msg_send![class!(NSDataDetector), dataDetectorWithTypes: 1u64 << 11, error: &mut err];
        if detector.is_null() {
            return vec![];
        }
        let s = NSString::from_str(text);
        let len: usize = msg_send![&*s, length];
        let matches: Id =
            msg_send![detector, matchesInString: &*s, options: 0usize, range: NSRange::new(0, len)];
        if matches.is_null() {
            return vec![];
        }
        let n: usize = msg_send![matches, count];
        (0..n)
            .map(|i| {
                let m: Id = msg_send![matches, objectAtIndex: i];
                let r: NSRange = msg_send![m, range];
                (r.location, r.length)
            })
            .collect()
    }
}
