use std::ffi::c_void;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSSize, NSString};

use super::macos::image_io::*;

type Id = *mut AnyObject;

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {}

#[repr(C)]
#[derive(Clone, Copy)]
struct CmTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

unsafe impl Encode for CmTime {
    const ENCODING: Encoding = Encoding::Struct(
        "?",
        &[i64::ENCODING, i32::ENCODING, u32::ENCODING, i64::ENCODING],
    );
}

unsafe impl RefEncode for CmTime {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
}

impl CmTime {
    const ZERO: Self = Self {
        value: 0,
        timescale: 1,
        flags: 1,
        epoch: 0,
    };

    fn seconds(s: f64) -> Self {
        Self {
            value: (s * 600.).round() as i64,
            timescale: 600,
            flags: 1,
            epoch: 0,
        }
    }

    fn as_seconds(self) -> f64 {
        if self.flags & 1 == 0 || self.timescale == 0 {
            0.
        } else {
            self.value as f64 / self.timescale as f64
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct CgImageRef(*const c_void);

unsafe impl Encode for CgImageRef {
    const ENCODING: Encoding = Encoding::Pointer(&Encoding::Struct("CGImage", &[]));
}

fn file_url(path: &Path) -> Id {
    let p = NSString::from_str(&path.to_string_lossy());
    unsafe { msg_send![class!(NSURL), fileURLWithPath: &*p] }
}

struct Generator {
    generator: Id,
    duration: f64,
}

impl Generator {
    fn new(path: &Path, max: NSSize, exact: bool) -> Option<Self> {
        unsafe {
            let asset: Id = msg_send![class!(AVURLAsset), URLAssetWithURL: file_url(path), options: std::ptr::null_mut::<AnyObject>()];
            if asset.is_null() {
                return None;
            }
            let duration: CmTime = msg_send![asset, duration];
            let generator: Id =
                msg_send![class!(AVAssetImageGenerator), assetImageGeneratorWithAsset: asset];
            if generator.is_null() {
                return None;
            }
            let _: () = msg_send![generator, setAppliesPreferredTrackTransform: true];
            let _: () = msg_send![generator, setMaximumSize: max];
            if exact {
                let _: () = msg_send![generator, setRequestedTimeToleranceBefore: CmTime::ZERO];
                let _: () = msg_send![generator, setRequestedTimeToleranceAfter: CmTime::ZERO];
            }
            Some(Self {
                generator,
                duration: duration.as_seconds(),
            })
        }
    }

    /// A +1 CGImage the caller releases.
    fn image(&self, at: f64) -> Option<CfRef> {
        unsafe {
            let img: CgImageRef = msg_send![
                self.generator,
                copyCGImageAtTime: CmTime::seconds(at),
                actualTime: std::ptr::null_mut::<CmTime>(),
                error: std::ptr::null_mut::<Id>()
            ];
            (!img.0.is_null()).then_some(img.0)
        }
    }
}

/// First frame of a video, at most 600 px on its longest side.
pub fn video_poster(path: &Path) -> Option<image::RgbaImage> {
    objc2::rc::autoreleasepool(|_| {
        let g = Generator::new(path, NSSize::new(600., 600.), false)?;
        let img = g.image(0.)?;
        let out = super::macos::cg_image_rgba(img);
        unsafe { CFRelease(img) };
        out
    })
}

fn cf_number(v: f64) -> CfRef {
    unsafe { CFNumberCreate(std::ptr::null(), CF_NUMBER_F64, (&v as *const f64).cast()) }
}

/// `{outer: {inner: value}}`, the shape ImageIO wants for GIF properties.
fn nested(outer: CfRef, inner: CfRef, value: f64) -> CfRef {
    unsafe {
        let n = cf_number(value);
        let d = CFDictionaryCreate(
            std::ptr::null(),
            &inner,
            &n,
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        CFRelease(n);
        let out = CFDictionaryCreate(
            std::ptr::null(),
            &outer,
            &d,
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        CFRelease(d);
        out
    }
}

/// Animated GIF next to the video: `fps` frames per second, at most `max_width` px wide, looping
/// forever.
pub fn video_to_gif(path: &Path, fps: f64, max_width: f64) -> Option<PathBuf> {
    objc2::rc::autoreleasepool(|_| unsafe {
        let g = Generator::new(path, NSSize::new(max_width, max_width * 4.), true)?;
        if g.duration <= 0. {
            return None;
        }
        let count = ((g.duration * fps) as usize).max(1);
        let out = path.with_extension("gif");
        let bytes = out.as_os_str().as_bytes();
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            false,
        );
        if url.is_null() {
            return None;
        }
        let kind =
            CFStringCreateWithCString(std::ptr::null(), c"com.compuserve.gif".as_ptr(), UTF8);
        let dest = CGImageDestinationCreateWithURL(url, kind, count, std::ptr::null());
        CFRelease(kind);
        CFRelease(url);
        if dest.is_null() {
            return None;
        }
        let file_props = nested(
            kCGImagePropertyGIFDictionary,
            kCGImagePropertyGIFLoopCount,
            0.,
        );
        CGImageDestinationSetProperties(dest, file_props);
        CFRelease(file_props);
        let frame_props = nested(
            kCGImagePropertyGIFDictionary,
            kCGImagePropertyGIFDelayTime,
            1. / fps,
        );
        for i in 0..count {
            objc2::rc::autoreleasepool(|_| {
                if let Some(img) = g.image(i as f64 / fps) {
                    CGImageDestinationAddImage(dest, img, frame_props);
                    CFRelease(img);
                }
            });
        }
        CFRelease(frame_props);
        let ok = CGImageDestinationFinalize(dest);
        CFRelease(dest);
        ok.then_some(out)
    })
}

/// Puts a file on the pasteboard, like Finder's Copy.
pub fn copy_file(path: &Path) -> bool {
    unsafe {
        let pb: Id = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: isize = msg_send![pb, clearContents];
        let items: Id = msg_send![class!(NSArray), arrayWithObject: file_url(path)];
        msg_send![pb, writeObjects: items]
    }
}

pub fn open_file(path: &Path) {
    unsafe {
        let ws: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let _: bool = msg_send![ws, openURL: file_url(path)];
    }
}
