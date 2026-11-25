#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_appkit;
#[cfg(target_os = "macos")]
pub mod macos_hotkeys;
#[cfg(target_os = "macos")]
pub mod macos_text;
#[cfg(target_os = "macos")]
mod macos_video;
#[cfg(target_os = "macos")]
pub use macos_video::{copy_file, open_file, video_poster, video_to_gif};

#[cfg(target_os = "macos")]
pub use macos_appkit::{
    accent_color, beep, color_panel, copy_image, cover_screen, desktop_image_path, display_origin,
    drag_file, haptic_align, launch_at_login, mouse_location, ns_view, pasteboard_image,
    play_sound, relative_date, render_symbol, set_ignores_mouse, set_launch_at_login,
    set_regular_policy, set_window_alpha, set_window_frame, set_window_frame_centered,
    set_window_level, set_window_shadow, share_file, visible_frame_at_mouse,
};
#[cfg(target_os = "macos")]
#[path = "macos_vision.rs"]
pub mod vision;

#[cfg(target_os = "macos")]
pub use macos::{
    SCREEN_CAPTURE_SETTINGS_URL, encode_heic, preferred_languages, relaunch,
    request_screen_capture, screen_capture_allowed,
};

#[cfg(target_os = "macos")]
pub use macos_hotkeys as hotkeys;

pub fn hide_from_dock() {
    #[cfg(target_os = "macos")]
    macos::set_accessory_policy();
}

pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn local_time() -> LocalTime {
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    LocalTime {
        year: tm.tm_year + 1900,
        month: tm.tm_mon as u32 + 1,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        minute: tm.tm_min as u32,
        second: tm.tm_sec as u32,
    }
}

pub fn decode_image_file(path: &std::path::Path) -> Option<image::RgbaImage> {
    if let Ok(img) = image::open(path) {
        return Some(img.to_rgba8());
    }
    #[cfg(target_os = "macos")]
    {
        macos::decode_with_image_io(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Pixels per point stored in the file (PNG pHYs), like the Swift app reads from the DPI.
pub fn file_scale(path: &std::path::Path) -> f64 {
    let Ok(file) = std::fs::File::open(path) else {
        return 1.;
    };
    let Ok(reader) = png::Decoder::new(std::io::BufReader::new(file)).read_info() else {
        return 1.;
    };
    match reader.info().pixel_dims {
        Some(d) if d.unit == png::Unit::Meter => (d.xppu as f64 * 0.0254 / 72.).round().max(1.),
        _ => 1.,
    }
}
