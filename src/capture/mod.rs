#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Recording, RegionGrabber};

use std::sync::Arc;

use anyhow::Result;
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use smallvec::smallvec;

pub struct DisplayShot {
    pub display_id: u32,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl DisplayShot {
    pub fn render_image(&self) -> Arc<RenderImage> {
        let buffer = RgbaImage::from_raw(self.width, self.height, self.bgra.clone())
            .expect("buffer BGRA com tamanho inconsistente");
        Arc::new(RenderImage::new(smallvec![Frame::new(buffer)]))
    }

    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32, scale: f32) -> Option<Captured> {
        let x = x.min(self.width);
        let y = y.min(self.height);
        let w = w.min(self.width - x);
        let h = h.min(self.height - y);
        if w == 0 || h == 0 {
            return None;
        }
        let stride = self.width as usize * 4;
        let mut bgra = Vec::with_capacity(w as usize * h as usize * 4);
        for row in y..y + h {
            let start = row as usize * stride + x as usize * 4;
            bgra.extend_from_slice(&self.bgra[start..start + w as usize * 4]);
        }
        Some(Captured {
            width: w,
            height: h,
            scale,
            bgra,
            screen_rect: None,
            window_id: None,
        })
    }

    pub fn whole(&self, scale: f32) -> Captured {
        Captured {
            width: self.width,
            height: self.height,
            scale,
            bgra: self.bgra.clone(),
            screen_rect: None,
            window_id: None,
        }
    }
}

pub struct Captured {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub bgra: Vec<u8>,
    /// Where it was on screen, in global points; drives the flash.
    pub screen_rect: Option<gpui::Bounds<gpui::Pixels>>,
    /// Set when the user picked a window instead of an area.
    pub window_id: Option<u32>,
}

impl Captured {
    pub fn render_image(&self) -> Arc<RenderImage> {
        let buffer = RgbaImage::from_raw(self.width, self.height, self.bgra.clone())
            .expect("buffer BGRA com tamanho inconsistente");
        Arc::new(RenderImage::new(smallvec![Frame::new(buffer)]))
    }

    pub fn rgba(&self) -> Vec<u8> {
        let mut rgba = self.bgra.clone();
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        rgba
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PickableWindow {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn pickable_windows() -> Vec<PickableWindow> {
    #[cfg(target_os = "macos")]
    {
        macos::pickable_windows()
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![]
    }
}

pub fn capture_window(id: u32) -> Result<Captured> {
    #[cfg(target_os = "macos")]
    {
        macos::capture_window(id)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = id;
        anyhow::bail!("captura ainda não implementada nesta plataforma")
    }
}

pub fn freeze_displays() -> Result<Vec<DisplayShot>> {
    #[cfg(target_os = "macos")]
    {
        macos::freeze_displays(crate::settings::get().hide_desktop_icons)
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("captura ainda não implementada nesta plataforma")
    }
}

/// Saves an image of the desktop wallpaper when the system can't give its file.
pub fn wallpaper_snapshot() -> Option<String> {
    None
}
