use std::ffi::c_void;

use objc2::runtime::{AnyClass, AnyObject};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

type Id = *mut AnyObject;

const NIL: Id = std::ptr::null_mut();

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
    fn CGContextRelease(ctx: *mut c_void);
    fn CGColorSpaceRelease(space: *const c_void);
    fn CGContextScaleCTM(ctx: *mut c_void, sx: f64, sy: f64);
    fn CGContextSetBlendMode(ctx: *mut c_void, mode: i32);
    fn CGContextSetRGBFillColor(ctx: *mut c_void, r: f64, g: f64, b: f64, a: f64);
    fn CGContextFillRect(ctx: *mut c_void, rect: NSRect);
}

const PREMULTIPLIED_LAST: u32 = 1;
const BLEND_SOURCE_IN: i32 = 18;

fn ns(s: &str) -> objc2::rc::Retained<NSString> {
    NSString::from_str(s)
}

pub fn cls(name: &str) -> Option<&'static AnyClass> {
    AnyClass::get(&std::ffi::CString::new(name).ok()?)
}

/// Rasterizes an SF Symbol tinted with `rgba`; returns straight BGRA pixels.
pub fn render_symbol(
    name: &str,
    size: f64,
    weight: f64,
    rgba: [u8; 4],
    scale: f64,
) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let name = ns(name);
        let image: Id = msg_send![class!(NSImage), imageWithSystemSymbolName: &*name, accessibilityDescription: NIL];
        if image.is_null() {
            return None;
        }
        let config: Id = msg_send![class!(NSImageSymbolConfiguration), configurationWithPointSize: size, weight: weight];
        let image: Id = msg_send![image, imageWithSymbolConfiguration: config];
        if image.is_null() {
            return None;
        }
        let sz: NSSize = msg_send![image, size];
        let (w, h) = (
            (sz.width * scale).ceil().max(1.) as usize,
            (sz.height * scale).ceil().max(1.) as usize,
        );
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
        CGColorSpaceRelease(space);
        if ctx.is_null() {
            return None;
        }
        CGContextScaleCTM(ctx, scale, scale);
        let gctx: Id =
            msg_send![class!(NSGraphicsContext), graphicsContextWithCGContext: ctx, flipped: false];
        let _: () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
        let _: () = msg_send![class!(NSGraphicsContext), setCurrentContext: gctx];
        let rect = NSRect::new(NSPoint::new(0., 0.), sz);
        let _: () = msg_send![image, drawInRect: rect];
        let _: () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
        CGContextSetBlendMode(ctx, BLEND_SOURCE_IN);
        CGContextSetRGBFillColor(
            ctx,
            rgba[0] as f64 / 255.,
            rgba[1] as f64 / 255.,
            rgba[2] as f64 / 255.,
            rgba[3] as f64 / 255.,
        );
        CGContextFillRect(ctx, rect);
        CGContextRelease(ctx);
        for px in buf.chunks_exact_mut(4) {
            let a = px[3] as u32;
            let (r, g, b) = (px[0] as u32, px[1] as u32, px[2] as u32);
            let un = |c: u32| {
                if a == 0 {
                    0
                } else {
                    ((c * 255 + a / 2) / a).min(255) as u8
                }
            };
            px[0] = un(b);
            px[1] = un(g);
            px[2] = un(r);
        }
        Some((w as u32, h as u32, buf))
    }
}

fn srgb_components(color: Id) -> Option<[f32; 4]> {
    unsafe {
        if color.is_null() {
            return None;
        }
        let space: Id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        let c: Id = msg_send![color, colorUsingColorSpace: space];
        if c.is_null() {
            return None;
        }
        let r: f64 = msg_send![c, redComponent];
        let g: f64 = msg_send![c, greenComponent];
        let b: f64 = msg_send![c, blueComponent];
        let a: f64 = msg_send![c, alphaComponent];
        Some([r as f32, g as f32, b as f32, a as f32])
    }
}

pub fn accent_color() -> gpui::Rgba {
    let c = unsafe {
        let color: Id = msg_send![class!(NSColor), controlAccentColor];
        srgb_components(color)
    };
    let [r, g, b, a] = c.unwrap_or([0., 0.48, 1., 1.]);
    gpui::Rgba { r, g, b, a }
}

/// Trackpad tick when a dragged annotation snaps to a guide.
pub fn haptic_align() {
    unsafe {
        let performer: Id = msg_send![class!(NSHapticFeedbackManager), defaultPerformer];
        if !performer.is_null() {
            let _: () =
                msg_send![performer, performFeedbackPattern: 1isize, performanceTime: 1usize];
        }
    }
}

pub fn beep() {
    unsafe {
        #[link(name = "AppKit", kind = "framework")]
        unsafe extern "C" {
            fn NSBeep();
        }
        NSBeep();
    }
}

/// The shared color panel; `current()` polls it, so no Objective-C target is needed.
pub mod color_panel {
    use super::*;

    pub fn open(rgba: [f64; 3]) {
        unsafe {
            let panel: Id = msg_send![class!(NSColorPanel), sharedColorPanel];
            let _: () = msg_send![panel, setShowsAlpha: false];
            let color: Id = msg_send![class!(NSColor), colorWithSRGBRed: rgba[0], green: rgba[1], blue: rgba[2], alpha: 1.0f64];
            let _: () = msg_send![panel, setColor: color];
            let _: () = msg_send![panel, makeKeyAndOrderFront: NIL];
        }
    }

    pub fn is_visible() -> bool {
        unsafe {
            let panel: Id = msg_send![class!(NSColorPanel), sharedColorPanel];
            msg_send![panel, isVisible]
        }
    }

    pub fn current() -> Option<[f64; 3]> {
        unsafe {
            let panel: Id = msg_send![class!(NSColorPanel), sharedColorPanel];
            let color: Id = msg_send![panel, color];
            srgb_components(color).map(|c| [c[0] as f64, c[1] as f64, c[2] as f64])
        }
    }
}

pub fn desktop_image_path() -> Option<String> {
    unsafe {
        let ws: Id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let screen: Id = msg_send![class!(NSScreen), mainScreen];
        if screen.is_null() {
            return None;
        }
        let url: Id = msg_send![ws, desktopImageURLForScreen: screen];
        if url.is_null() {
            return None;
        }
        let path: *const NSString = msg_send![url, path];
        path.as_ref().map(|p| p.to_string())
    }
}

/// Shows the system share menu for a file, anchored at a point in the window (in points,
/// origin top-left).
pub fn share_file(ns_view: *mut c_void, path: &std::path::Path, x: f64, y: f64) {
    unsafe {
        let view = ns_view as Id;
        if view.is_null() {
            return;
        }
        let path = ns(&path.to_string_lossy());
        let url: Id = msg_send![class!(NSURL), fileURLWithPath: &*path];
        let items: Id = msg_send![class!(NSArray), arrayWithObject: url];
        let alloc: Id = msg_send![class!(NSSharingServicePicker), alloc];
        let picker: Id = msg_send![alloc, initWithItems: items];
        let bounds: NSRect = msg_send![view, bounds];
        let flipped: bool = msg_send![view, isFlipped];
        let vy = if flipped { y } else { bounds.size.height - y };
        let rect = NSRect::new(NSPoint::new(x - 1., vy - 1.), NSSize::new(2., 2.));
        let _: () =
            msg_send![picker, showRelativeToRect: rect, ofView: view, preferredEdge: 1usize];
    }
}

pub fn ns_view(window: &gpui::Window) -> Option<*mut c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr()),
        _ => None,
    }
}

pub fn ns_window(window: &gpui::Window) -> Option<Id> {
    let view = ns_view(window)? as Id;
    let w: Id = unsafe { msg_send![view, window] };
    (!w.is_null()).then_some(w)
}

fn ns_data(bytes: &[u8]) -> Id {
    unsafe {
        msg_send![class!(NSData), dataWithBytes: bytes.as_ptr().cast::<c_void>(), length: bytes.len()]
    }
}

/// Puts a PNG (with its DPI) and a TIFF sized in points on the pasteboard, like the Swift app.
pub fn copy_png(png: &[u8], width_pt: f64, height_pt: f64) -> anyhow::Result<()> {
    unsafe {
        let pb: Id = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: isize = msg_send![pb, clearContents];
        let data = ns_data(png);
        let alloc: Id = msg_send![class!(NSImage), alloc];
        let image: Id = msg_send![alloc, initWithData: data];
        anyhow::ensure!(!image.is_null(), "imagem inválida");
        let _: () = msg_send![image, setSize: NSSize::new(width_pt, height_pt)];
        let tiff: Id = msg_send![image, TIFFRepresentation];
        let item: Id = msg_send![class!(NSPasteboardItem), new];
        let png_type = ns("public.png");
        let tiff_type = ns("public.tiff");
        let _: bool = msg_send![item, setData: data, forType: &*png_type];
        if !tiff.is_null() {
            let _: bool = msg_send![item, setData: tiff, forType: &*tiff_type];
        }
        let items: Id = msg_send![class!(NSArray), arrayWithObject: item];
        let ok: bool = msg_send![pb, writeObjects: items];
        anyhow::ensure!(ok, "não copiou");
        Ok(())
    }
}

pub fn copy_image(pm: &tiny_skia::Pixmap, scale: f64, retina: bool) -> anyhow::Result<()> {
    let (pm, scale) = crate::export::sized(pm, scale, retina);
    let png = crate::export::png(&pm, scale)?;
    copy_png(&png, pm.width() as f64 / scale, pm.height() as f64 / scale)
}

/// Image on the pasteboard (a file URL or PNG/TIFF data) and its pixels per point.
pub fn pasteboard_image() -> Option<(image::RgbaImage, f64)> {
    unsafe {
        let pb: Id = msg_send![class!(NSPasteboard), generalPasteboard];
        let url_type = ns("public.file-url");
        let url_str: *const NSString = msg_send![pb, stringForType: &*url_type];
        if let Some(s) = url_str.as_ref() {
            let s = s.to_string();
            if let Some(path) = s.strip_prefix("file://") {
                let path = percent_decode(path);
                let p = std::path::Path::new(&path);
                if let Some(img) = crate::platform::decode_image_file(p) {
                    return Some((img, crate::platform::file_scale(p)));
                }
            }
        }
        for t in ["public.png", "public.tiff"] {
            let ty = ns(t);
            let data: Id = msg_send![pb, dataForType: &*ty];
            if data.is_null() {
                continue;
            }
            let len: usize = msg_send![data, length];
            let ptr: *const u8 = msg_send![data, bytes];
            let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
            if let Ok(img) = image::load_from_memory(&bytes) {
                return Some((img.to_rgba8(), main_backing_scale()));
            }
            let tmp = std::env::temp_dir().join(format!(
                "kirei-paste.{}",
                if t.ends_with("png") { "png" } else { "tiff" }
            ));
            if std::fs::write(&tmp, &bytes).is_ok()
                && let Some(img) = super::macos::decode_with_image_io(&tmp)
            {
                return Some((img, main_backing_scale()));
            }
        }
        None
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn main_backing_scale() -> f64 {
    unsafe {
        let screen: Id = msg_send![class!(NSScreen), mainScreen];
        if screen.is_null() {
            return 2.;
        }
        msg_send![screen, backingScaleFactor]
    }
}

objc2::define_class!(
    #[unsafe(super(objc2::runtime::NSObject))]
    #[name = "KireiDragSource"]
    struct DragSource;

    impl DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operation_mask(&self, _session: *mut AnyObject, _context: isize) -> usize {
            1
        }
    }
);

/// Starts dragging a file out of the window from the current mouse event.
pub fn drag_file(window: &gpui::Window, path: &std::path::Path) {
    let Some(view) = ns_view(window).map(|v| v as Id) else {
        return;
    };
    unsafe {
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let event: Id = msg_send![app, currentEvent];
        if event.is_null() {
            return;
        }
        let p = ns(&path.to_string_lossy());
        let url: Id = msg_send![class!(NSURL), fileURLWithPath: &*p];
        let alloc: Id = msg_send![class!(NSDraggingItem), alloc];
        let item: Id = msg_send![alloc, initWithPasteboardWriter: url];
        let alloc: Id = msg_send![class!(NSImage), alloc];
        let image: Id = msg_send![alloc, initWithContentsOfFile: &*p];
        let loc: NSPoint = msg_send![event, locationInWindow];
        let local: NSPoint = msg_send![view, convertPoint: loc, fromView: NIL];
        let (w, h) = (96., 64.);
        let frame = NSRect::new(
            NSPoint::new(local.x - w / 2., local.y - h / 2.),
            NSSize::new(w, h),
        );
        let _: () = msg_send![item, setDraggingFrame: frame, contents: image];
        let items: Id = msg_send![class!(NSArray), arrayWithObject: item];
        let source: objc2::rc::Retained<DragSource> = msg_send![DragSource::class(), new];
        let _: Id =
            msg_send![view, beginDraggingSessionWithItems: items, event: event, source: &*source];
        std::mem::forget(source);
    }
}

use objc2::ClassType;

thread_local! {
    static SOUNDS: std::cell::RefCell<std::collections::HashMap<u8, usize>> = Default::default();
}

/// Plays a WAV made on first use; NSSound objects are kept for the app's lifetime.
pub fn play_sound(key: u8, make: impl FnOnce() -> (Vec<u8>, f32)) {
    SOUNDS.with(|s| {
        let mut map = s.borrow_mut();
        let sound = *map.entry(key).or_insert_with(|| unsafe {
            let (bytes, volume) = make();
            let data = ns_data(&bytes);
            let alloc: Id = msg_send![class!(NSSound), alloc];
            let sound: Id = msg_send![alloc, initWithData: data];
            if !sound.is_null() {
                let _: () = msg_send![sound, setVolume: volume];
            }
            sound as usize
        }) as Id;
        if sound.is_null() {
            return;
        }
        unsafe {
            let _: bool = msg_send![sound, stop];
            let _: bool = msg_send![sound, play];
        }
    });
}

pub fn set_window_level(window: &gpui::Window, level: isize) {
    if let Some(w) = ns_window(window) {
        unsafe {
            let _: () = msg_send![w, setLevel: level];
        }
    }
}

/// Makes a window borderless and moves it over `bounds`, menu bar included. gpui opens every
/// window as titled, and AppKit keeps titled windows below the menu bar.
pub fn cover_screen(window: &gpui::Window, bounds: gpui::Bounds<gpui::Pixels>) {
    if let Some(w) = ns_window(window) {
        unsafe {
            let nonactivating_panel: usize = 1 << 7;
            let _: () = msg_send![w, setStyleMask: nonactivating_panel];
        }
    }
    set_window_frame(window, bounds);
}

pub fn set_window_shadow(window: &gpui::Window, on: bool) {
    if let Some(w) = ns_window(window) {
        unsafe {
            let _: () = msg_send![w, setHasShadow: on];
        }
    }
}

pub fn set_window_alpha(window: &gpui::Window, alpha: f64) {
    if let Some(w) = ns_window(window) {
        unsafe {
            let _: () = msg_send![w, setAlphaValue: alpha];
        }
    }
}

pub fn set_ignores_mouse(window: &gpui::Window, on: bool) {
    if let Some(w) = ns_window(window) {
        unsafe {
            let _: () = msg_send![w, setIgnoresMouseEvents: on];
        }
    }
}

/// Resizes keeping the window's center in place.
pub fn set_window_frame_centered(window: &gpui::Window, width: f64, height: f64) {
    if let Some(w) = ns_window(window) {
        unsafe {
            let f: NSRect = msg_send![w, frame];
            let nf = NSRect::new(
                NSPoint::new(
                    f.origin.x + (f.size.width - width) / 2.,
                    f.origin.y + (f.size.height - height) / 2.,
                ),
                NSSize::new(width, height),
            );
            let _: () = msg_send![w, setFrame: nf, display: true];
        }
    }
}

/// Moves and resizes a window to `bounds` in gpui's global coordinates.
pub fn set_window_frame(window: &gpui::Window, bounds: gpui::Bounds<gpui::Pixels>) {
    let Some(w) = ns_window(window) else { return };
    unsafe {
        let screens: Id = msg_send![class!(NSScreen), screens];
        let main: Id = msg_send![screens, firstObject];
        if main.is_null() {
            return;
        }
        let mf: NSRect = msg_send![main, frame];
        let (x, y) = (
            f32::from(bounds.origin.x) as f64,
            f32::from(bounds.origin.y) as f64,
        );
        let (bw, bh) = (
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        );
        let nf = NSRect::new(
            NSPoint::new(x, mf.size.height - y - bh),
            NSSize::new(bw, bh),
        );
        let _: () = msg_send![w, setFrame: nf, display: true];
    }
}

/// Mouse position in gpui's global coordinates (points, origin at the top-left of the main
/// screen).
pub fn mouse_location() -> gpui::Point<gpui::Pixels> {
    unsafe {
        let p: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let screens: Id = msg_send![class!(NSScreen), screens];
        let main: Id = msg_send![screens, firstObject];
        let h = if main.is_null() {
            0.
        } else {
            let f: NSRect = msg_send![main, frame];
            f.size.height
        };
        gpui::point(gpui::px(p.x as f32), gpui::px((h - p.y) as f32))
    }
}

pub fn set_regular_policy(regular: bool) {
    unsafe {
        let app: Id = msg_send![class!(NSApplication), sharedApplication];
        let current: isize = msg_send![app, activationPolicy];
        let wanted: isize = if regular { 0 } else { 1 };
        if current != wanted {
            let _: bool = msg_send![app, setActivationPolicy: wanted];
        }
    }
}

/// "2 min ago" in the app's language, like RelativeDateTimeFormatter in the Swift app.
pub fn relative_date(unix_time: f64) -> String {
    unsafe {
        let f: Id = msg_send![class!(NSRelativeDateTimeFormatter), new];
        let _: () = msg_send![f, setUnitsStyle: 1isize];
        let code = ns(&crate::l10n::current().code().replace('-', "_"));
        let locale: Id = msg_send![class!(NSLocale), localeWithLocaleIdentifier: &*code];
        let _: () = msg_send![f, setLocale: locale];
        let date: Id = msg_send![class!(NSDate), dateWithTimeIntervalSince1970: unix_time];
        let now: Id = msg_send![class!(NSDate), date];
        let s: *const NSString = msg_send![f, localizedStringForDate: date, relativeToDate: now];
        let out = s.as_ref().map(|s| s.to_string()).unwrap_or_default();
        let _: () = msg_send![f, release];
        out
    }
}

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

/// SMAppService.mainApp, so "Open at login" shows up in System Settings like other apps.
pub fn launch_at_login() -> bool {
    unsafe {
        let service: Id = msg_send![class!(SMAppService), mainAppService];
        if service.is_null() {
            return false;
        }
        let status: isize = msg_send![service, status];
        status == 1
    }
}

pub fn set_launch_at_login(on: bool) {
    unsafe {
        let service: Id = msg_send![class!(SMAppService), mainAppService];
        if service.is_null() {
            return;
        }
        let mut err: Id = std::ptr::null_mut();
        let ok: bool = if on {
            msg_send![service, registerAndReturnError: &mut err]
        } else {
            msg_send![service, unregisterAndReturnError: &mut err]
        };
        if !ok {
            eprintln!("kirei: abrir ao iniciar falhou");
        }
    }
}

/// Visible frame (without menu bar and Dock) of the screen under the mouse, in gpui's global
/// coordinates.
pub fn visible_frame_at_mouse() -> gpui::Bounds<gpui::Pixels> {
    unsafe {
        let p: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let screens: Id = msg_send![class!(NSScreen), screens];
        let n: usize = msg_send![screens, count];
        let main: Id = msg_send![screens, firstObject];
        let main_h = if main.is_null() {
            0.
        } else {
            let f: NSRect = msg_send![main, frame];
            f.size.height
        };
        let mut chosen = main;
        for i in 0..n {
            let s: Id = msg_send![screens, objectAtIndex: i];
            let f: NSRect = msg_send![s, frame];
            if p.x >= f.origin.x
                && p.x <= f.origin.x + f.size.width
                && p.y >= f.origin.y
                && p.y <= f.origin.y + f.size.height
            {
                chosen = s;
            }
        }
        if chosen.is_null() {
            return gpui::Bounds::new(
                gpui::point(gpui::px(0.), gpui::px(0.)),
                gpui::size(gpui::px(1440.), gpui::px(900.)),
            );
        }
        let vf: NSRect = msg_send![chosen, visibleFrame];
        gpui::Bounds::new(
            gpui::point(
                gpui::px(vf.origin.x as f32),
                gpui::px((main_h - vf.origin.y - vf.size.height) as f32),
            ),
            gpui::size(
                gpui::px(vf.size.width as f32),
                gpui::px(vf.size.height as f32),
            ),
        )
    }
}

/// Top-left of a display in gpui's global coordinates. gpui 0.2 reports every display at the
/// origin, which breaks anything placed on a secondary screen.
pub fn display_origin(display_id: u32) -> Option<gpui::Point<gpui::Pixels>> {
    unsafe {
        let screens: Id = msg_send![class!(NSScreen), screens];
        let n: usize = msg_send![screens, count];
        let main: Id = msg_send![screens, firstObject];
        if main.is_null() {
            return None;
        }
        let main_frame: NSRect = msg_send![main, frame];
        let key = ns("NSScreenNumber");
        for i in 0..n {
            let s: Id = msg_send![screens, objectAtIndex: i];
            let desc: Id = msg_send![s, deviceDescription];
            let num: Id = msg_send![desc, objectForKey: &*key];
            if num.is_null() {
                continue;
            }
            let id: u32 = msg_send![num, unsignedIntValue];
            if id == display_id {
                let f: NSRect = msg_send![s, frame];
                return Some(gpui::point(
                    gpui::px(f.origin.x as f32),
                    gpui::px((main_frame.size.height - f.origin.y - f.size.height) as f32),
                ));
            }
        }
        None
    }
}
