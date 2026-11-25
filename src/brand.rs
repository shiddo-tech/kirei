use std::sync::Arc;

use gpui::{Image, ImageFormat, Rgba, rgb};

pub const MARK_SVG_PATH: &str = "M8 60L8 36C8 20 20 8 36 8L60 8L54 24L40 24C31 24 24 31 24 40L24 54Z\
     M92 40L92 64C92 80 80 92 64 92L40 92L46 76L60 76C69 76 76 69 76 60L76 46Z";

pub fn green() -> Rgba {
    rgb(0x3e8c63)
}

pub fn green_light() -> Rgba {
    rgb(0x7ccb9e)
}

/// The dark surface the welcome and settings windows share, whatever the system appearance.
pub mod surface {
    pub const BG: u32 = 0x0e100f;
    pub const CARD: u32 = 0x151816;
    pub const CARD_BORDER: u32 = 0x232724;
    pub const DIVIDER: u32 = 0x222623;
    pub const CONTROL: u32 = 0x1c1f1d;
    pub const CONTROL_HOVER: u32 = 0x252926;
    pub const CONTROL_BORDER: u32 = 0x30342f;
    pub const TEXT: u32 = 0xeef2ef;
    pub const TEXT_STRONG: u32 = 0xf4f7f5;
    pub const MUTED: u32 = 0x8f9891;
    pub const FAINT: u32 = 0x6f7771;
}

pub fn app_icon() -> Arc<Image> {
    Arc::new(Image::from_bytes(
        ImageFormat::Png,
        include_bytes!("../assets/icon-256.png").to_vec(),
    ))
}

pub fn mark_rgba(size: u32) -> Vec<u8> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 104 104"><path d="{MARK_SVG_PATH}" fill="black"/></svg>"#
    );
    let tree =
        resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).expect("SVG da marca");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}
