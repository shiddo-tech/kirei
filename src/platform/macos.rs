use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

pub const SCREEN_CAPTURE_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

// O gpui força a política Regular ao iniciar, então o LSUIElement do Info.plist não basta.
pub fn set_accessory_policy() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

pub fn screen_capture_allowed() -> bool {
    if cfg!(debug_assertions) && std::env::var_os("KIREI_SIMULATE_NO_PERMISSION").is_some() {
        return false;
    }
    unsafe { CGPreflightScreenCaptureAccess() }
}

pub fn request_screen_capture() {
    unsafe {
        CGRequestScreenCaptureAccess();
    }
}

pub fn relaunch(cx: &mut gpui::App) {
    let bundle = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.ancestors().nth(3).map(|p| p.to_path_buf()))
        .filter(|p| p.extension().is_some_and(|e| e == "app"));
    if let Some(bundle) = bundle {
        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("sleep 0.6; open \"$0\"")
            .arg(bundle)
            .spawn()
            .ok();
    }
    cx.quit();
}

pub fn preferred_languages() -> Vec<String> {
    objc2_foundation::NSLocale::preferredLanguages()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

pub(super) mod image_io {
    use std::ffi::c_void;

    pub type CfRef = *const c_void;

    #[repr(C)]
    pub struct CfRange {
        pub location: isize,
        pub length: isize,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        pub static kCFTypeDictionaryKeyCallBacks: c_void;
        pub static kCFTypeDictionaryValueCallBacks: c_void;
        pub fn CFRelease(cf: CfRef);
        pub fn CFURLCreateFromFileSystemRepresentation(
            alloc: CfRef,
            buf: *const u8,
            len: isize,
            is_dir: bool,
        ) -> CfRef;
        pub fn CFDataCreateMutable(alloc: CfRef, capacity: isize) -> CfRef;
        pub fn CFDataGetLength(data: CfRef) -> isize;
        pub fn CFDataGetBytes(data: CfRef, range: CfRange, out: *mut u8);
        pub fn CFStringCreateWithCString(alloc: CfRef, s: *const i8, encoding: u32) -> CfRef;
        pub fn CFNumberCreate(alloc: CfRef, kind: isize, value: *const c_void) -> CfRef;
        pub fn CFDictionaryCreate(
            alloc: CfRef,
            keys: *const CfRef,
            values: *const CfRef,
            count: isize,
            key_cb: *const c_void,
            value_cb: *const c_void,
        ) -> CfRef;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        pub fn CGColorSpaceCreateWithName(name: CfRef) -> CfRef;
        pub static kCGColorSpaceSRGB: CfRef;
        pub fn CGBitmapContextCreate(
            data: *mut c_void,
            w: usize,
            h: usize,
            bpc: usize,
            bpr: usize,
            space: CfRef,
            info: u32,
        ) -> CfRef;
        pub fn CGBitmapContextCreateImage(ctx: CfRef) -> CfRef;
        pub fn CGContextDrawImage(ctx: CfRef, rect: CgRect, image: CfRef);
        pub fn CGImageGetWidth(image: CfRef) -> usize;
        pub fn CGImageGetHeight(image: CfRef) -> usize;
    }

    #[link(name = "ImageIO", kind = "framework")]
    unsafe extern "C" {
        pub fn CGImageSourceCreateWithURL(url: CfRef, options: CfRef) -> CfRef;
        pub fn CGImageSourceCreateImageAtIndex(src: CfRef, index: usize, options: CfRef) -> CfRef;
        pub fn CGImageDestinationCreateWithData(
            data: CfRef,
            kind: CfRef,
            count: usize,
            options: CfRef,
        ) -> CfRef;
        pub fn CGImageDestinationAddImage(dest: CfRef, image: CfRef, props: CfRef);
        pub fn CGImageDestinationFinalize(dest: CfRef) -> bool;
        pub fn CGImageDestinationCreateWithURL(
            url: CfRef,
            kind: CfRef,
            count: usize,
            options: CfRef,
        ) -> CfRef;
        pub fn CGImageDestinationSetProperties(dest: CfRef, props: CfRef);
        pub static kCGImagePropertyGIFDictionary: CfRef;
        pub static kCGImagePropertyGIFLoopCount: CfRef;
        pub static kCGImagePropertyGIFDelayTime: CfRef;
        pub static kCGImageDestinationLossyCompressionQuality: CfRef;
        pub static kCGImagePropertyDPIWidth: CfRef;
        pub static kCGImagePropertyDPIHeight: CfRef;
    }

    #[repr(C)]
    pub struct CgRect {
        pub x: f64,
        pub y: f64,
        pub w: f64,
        pub h: f64,
    }

    pub const PREMULTIPLIED_LAST: u32 = 1;
    pub const UTF8: u32 = 0x0800_0100;
    pub const CF_NUMBER_F64: isize = 13;
}

pub fn decode_with_image_io(path: &std::path::Path) -> Option<image::RgbaImage> {
    use image_io::*;
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            false,
        );
        if url.is_null() {
            return None;
        }
        let src = CGImageSourceCreateWithURL(url, std::ptr::null());
        CFRelease(url);
        if src.is_null() {
            return None;
        }
        let img = CGImageSourceCreateImageAtIndex(src, 0, std::ptr::null());
        CFRelease(src);
        if img.is_null() {
            return None;
        }
        let out = cg_image_rgba(img);
        CFRelease(img);
        out
    }
}

/// Straight RGBA pixels of a CGImage (the caller keeps ownership of `img`).
pub(super) fn cg_image_rgba(img: image_io::CfRef) -> Option<image::RgbaImage> {
    use image_io::*;
    unsafe {
        let (w, h) = (CGImageGetWidth(img), CGImageGetHeight(img));
        let mut buf = vec![0u8; w * h * 4];
        let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        let ctx = CGBitmapContextCreate(
            buf.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            space,
            PREMULTIPLIED_LAST,
        );
        CFRelease(space);
        if ctx.is_null() {
            return None;
        }
        CGContextDrawImage(
            ctx,
            CgRect {
                x: 0.,
                y: 0.,
                w: w as f64,
                h: h as f64,
            },
            img,
        );
        CFRelease(ctx);
        for px in buf.chunks_exact_mut(4) {
            let a = px[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        image::RgbaImage::from_raw(w as u32, h as u32, buf)
    }
}

pub fn encode_heic(pm: &tiny_skia::Pixmap, scale: f64) -> anyhow::Result<Vec<u8>> {
    use image_io::*;
    unsafe {
        let mut buf = pm.data().to_vec();
        let (w, h) = (pm.width() as usize, pm.height() as usize);
        let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        let ctx = CGBitmapContextCreate(
            buf.as_mut_ptr().cast(),
            w,
            h,
            8,
            w * 4,
            space,
            PREMULTIPLIED_LAST,
        );
        CFRelease(space);
        anyhow::ensure!(!ctx.is_null(), "CGBitmapContextCreate");
        let img = CGBitmapContextCreateImage(ctx);
        CFRelease(ctx);
        anyhow::ensure!(!img.is_null(), "CGBitmapContextCreateImage");
        let data = CFDataCreateMutable(std::ptr::null(), 0);
        let kind = CFStringCreateWithCString(std::ptr::null(), c"public.heic".as_ptr(), UTF8);
        let dest = CGImageDestinationCreateWithData(data, kind, 1, std::ptr::null());
        CFRelease(kind);
        if dest.is_null() {
            CFRelease(img);
            CFRelease(data);
            anyhow::bail!("HEIC não disponível");
        }
        let quality = 0.92f64;
        let dpi = 72. * scale;
        let values = [
            CFNumberCreate(
                std::ptr::null(),
                CF_NUMBER_F64,
                (&quality as *const f64).cast(),
            ),
            CFNumberCreate(std::ptr::null(), CF_NUMBER_F64, (&dpi as *const f64).cast()),
            CFNumberCreate(std::ptr::null(), CF_NUMBER_F64, (&dpi as *const f64).cast()),
        ];
        let keys = [
            kCGImageDestinationLossyCompressionQuality,
            kCGImagePropertyDPIWidth,
            kCGImagePropertyDPIHeight,
        ];
        let props = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            3,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        CGImageDestinationAddImage(dest, img, props);
        let ok = CGImageDestinationFinalize(dest);
        CFRelease(props);
        values.iter().for_each(|v| CFRelease(*v));
        CFRelease(dest);
        CFRelease(img);
        let len = CFDataGetLength(data);
        let mut out = vec![0u8; len as usize];
        CFDataGetBytes(
            data,
            CfRange {
                location: 0,
                length: len,
            },
            out.as_mut_ptr(),
        );
        CFRelease(data);
        anyhow::ensure!(ok && !out.is_empty(), "falha ao gerar HEIC");
        Ok(out)
    }
}
