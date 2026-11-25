use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::l10n::l;
use crate::model::Style;

/// `KIREI_SUPPORT_DIR` keeps dev runs and tests away from the real settings and history.
pub fn support_dir() -> PathBuf {
    if cfg!(test) {
        return std::env::temp_dir().join(format!("kirei-test-{}", std::process::id()));
    }
    if let Some(dir) = std::env::var_os("KIREI_SUPPORT_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library/Application Support/Kirei")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureAction {
    Area,
    Window,
    Fullscreen,
    Scrolling,
    Record,
    Text,
    Clipboard,
}

impl CaptureAction {
    pub const ALL: [CaptureAction; 7] = [
        Self::Area,
        Self::Window,
        Self::Fullscreen,
        Self::Scrolling,
        Self::Record,
        Self::Text,
        Self::Clipboard,
    ];

    pub fn title(self) -> String {
        l(match self {
            Self::Area => "Capture Area",
            Self::Window => "Capture Window",
            Self::Fullscreen => "Capture Full Screen",
            Self::Scrolling => "Scrolling Capture",
            Self::Record => "Record Screen",
            Self::Text => "Capture Text (OCR)",
            Self::Clipboard => "Edit Image from Clipboard",
        })
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Area => "rectangle.dashed",
            Self::Window => "macwindow",
            Self::Fullscreen => "display",
            Self::Scrolling => "arrow.up.and.down.text.horizontal",
            Self::Record => "record.circle",
            Self::Text => "text.viewfinder",
            Self::Clipboard => "doc.on.clipboard",
        }
    }
}

pub const MOD_CMD: u32 = 1 << 8;
pub const MOD_SHIFT: u32 = 1 << 9;
pub const MOD_OPTION: u32 = 1 << 11;
pub const MOD_CONTROL: u32 = 1 << 12;
pub const KEY_CLEARED: u32 = 0xFFFF;

/// Carbon key code and modifier mask, the same encoding the Swift app stores.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotKey {
    pub key_code: u32,
    pub modifiers: u32,
    pub display: String,
}

impl HotKey {
    fn option_shift(key_code: u32, digit: char) -> Self {
        Self {
            key_code,
            modifiers: MOD_OPTION | MOD_SHIFT,
            display: format!("⌥⇧{digit}"),
        }
    }

    pub fn is_cleared(&self) -> bool {
        self.key_code == KEY_CLEARED
    }

    pub fn defaults() -> BTreeMap<CaptureAction, HotKey> {
        BTreeMap::from([
            (CaptureAction::Area, Self::option_shift(0x12, '1')),
            (CaptureAction::Window, Self::option_shift(0x13, '2')),
            (CaptureAction::Fullscreen, Self::option_shift(0x14, '3')),
            (CaptureAction::Text, Self::option_shift(0x15, '4')),
            (CaptureAction::Clipboard, Self::option_shift(0x17, '5')),
            (CaptureAction::Record, Self::option_shift(0x16, '6')),
            (CaptureAction::Scrolling, Self::option_shift(0x1A, '7')),
        ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AfterCapture {
    #[default]
    Preview,
    OpenEditor,
    CopyStyled,
}

impl AfterCapture {
    pub const ALL: [AfterCapture; 3] = [Self::Preview, Self::OpenEditor, Self::CopyStyled];

    pub fn title(self) -> String {
        l(match self {
            Self::Preview => "Show floating preview",
            Self::OpenEditor => "Open the editor",
            Self::CopyStyled => "Copy styled image directly",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureSound {
    #[default]
    Voice,
    Shutter,
}

impl CaptureSound {
    pub fn title(self) -> String {
        match self {
            Self::Voice => l("Voice “kirei”"),
            Self::Shutter => l("Shutter"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    #[default]
    Png,
    Jpeg,
    Heic,
}

impl Format {
    pub const ALL: [Format; 3] = [Self::Png, Self::Jpeg, Self::Heic];

    pub fn title(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Heic => "HEIC",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Heic => "heic",
        }
    }

    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "heic" => Some(Self::Heic),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub hot_keys: BTreeMap<CaptureAction, HotKey>,
    pub after_capture: AfterCapture,
    pub auto_redact: bool,
    pub format: Format,
    pub save_folder: Option<PathBuf>,
    pub play_sound: bool,
    pub capture_sound: CaptureSound,
    /// false exports at 1 pixel per point (half size on Retina).
    pub export_retina: bool,
    /// Seconds to wait before a capture; 0 = immediately.
    pub capture_delay: u32,
    pub hide_desktop_icons: bool,
    pub keep_history: bool,
    /// None follows the system.
    pub language: Option<String>,
    pub inspector_sections: BTreeMap<String, bool>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hot_keys: HotKey::defaults(),
            after_capture: AfterCapture::Preview,
            auto_redact: false,
            format: Format::Png,
            save_folder: None,
            play_sound: true,
            capture_sound: CaptureSound::Voice,
            export_retina: true,
            capture_delay: 0,
            hide_desktop_icons: false,
            keep_history: true,
            language: None,
            inspector_sections: BTreeMap::new(),
        }
    }
}

impl AppSettings {
    pub fn save_folder(&self) -> PathBuf {
        self.save_folder.clone().unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("Desktop")
        })
    }

    pub fn hot_key(&self, action: CaptureAction) -> Option<&HotKey> {
        self.hot_keys.get(&action).filter(|k| !k.is_cleared())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub style: Style,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Stored {
    settings: AppSettings,
    presets: Vec<Preset>,
    last_style: Option<Style>,
}

fn path() -> PathBuf {
    support_dir().join("settings.json")
}

fn store() -> &'static Mutex<Stored> {
    static STORE: OnceLock<Mutex<Stored>> = OnceLock::new();
    STORE.get_or_init(|| {
        let mut stored: Stored = std::fs::read(path())
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        let mut keys = HotKey::defaults();
        keys.append(&mut stored.settings.hot_keys);
        stored.settings.hot_keys = keys;
        Mutex::new(stored)
    })
}

fn persist(s: &Stored) {
    std::fs::create_dir_all(support_dir()).ok();
    if let Ok(data) = serde_json::to_vec_pretty(s) {
        let tmp = path().with_extension("json.tmp");
        if std::fs::write(&tmp, data).is_ok() {
            std::fs::rename(tmp, path()).ok();
        }
    }
}

pub fn get() -> AppSettings {
    store()
        .lock()
        .map(|s| s.settings.clone())
        .unwrap_or_default()
}

pub fn update(f: impl FnOnce(&mut AppSettings)) {
    if let Ok(mut s) = store().lock() {
        f(&mut s.settings);
        persist(&s);
    }
}

pub fn presets() -> Vec<Preset> {
    store()
        .lock()
        .map(|s| s.presets.clone())
        .unwrap_or_default()
}

pub fn update_presets(f: impl FnOnce(&mut Vec<Preset>)) {
    if let Ok(mut s) = store().lock() {
        f(&mut s.presets);
        persist(&s);
    }
}

/// The last style used becomes the default for the next capture.
pub fn last_style() -> Style {
    store()
        .lock()
        .ok()
        .and_then(|s| s.last_style.clone())
        .unwrap_or_default()
}

pub fn set_last_style(style: &Style) {
    if let Ok(mut s) = store().lock()
        && s.last_style.as_ref() != Some(style)
    {
        s.last_style = Some(style.clone());
        persist(&s);
    }
}
