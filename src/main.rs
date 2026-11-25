mod analysis;
mod brand;
mod capture;
mod countdown;
mod diagnostics;
mod editor;
mod export;
mod feedback;
mod flash;
mod flow;
mod history;
mod history_window;
mod hud;
mod l10n;
mod model;
mod ocr;
mod overlay;
mod pinned;
mod platform;
mod recorder;
mod render;
mod scroll;
mod scrolling;
mod selftest;
mod settings;
mod settings_window;
mod shell;
mod thumbnail;
mod ui;
mod welcome;

use gpui::{App, Application};

/// `--edit` and `--show` runs must not take focus from whatever the user is doing.
pub fn dev_mode() -> bool {
    std::env::args().any(|a| a == "--edit" || a == "--show")
}

pub fn activate(cx: &mut App) {
    if !dev_mode() {
        cx.activate(true);
    }
}

fn main() {
    let dev = std::env::args().any(|a| a == "--edit" || a == "--show");
    if dev && std::env::var_os("KIREI_SUPPORT_DIR").is_none() {
        let dir = std::env::temp_dir().join("kirei-dev");
        // SAFETY: still single-threaded; nothing has read the environment yet.
        unsafe { std::env::set_var("KIREI_SUPPORT_DIR", &dir) };
    }
    diagnostics::start();
    l10n::init(settings::get().language.as_deref());
    if selftest::run_from_args() {
        return;
    }
    let app = Application::new();
    app.on_reopen(welcome::open);
    let edit_path = std::env::args()
        .skip_while(|a| a != "--edit")
        .nth(1)
        .map(std::path::PathBuf::from);
    let show = std::env::args().skip_while(|a| a != "--show").nth(1);
    app.run(move |cx: &mut App| {
        ui::text_input::bind_keys(cx);
        editor::bind_keys(cx);
        platform::hide_from_dock();
        // Dev mode: only the requested window, without hotkeys, tray icon or welcome window.
        if let Some(name) = show.as_deref() {
            match name {
                "settings" => settings_window::open(cx),
                "settings-shortcuts" => {
                    settings_window::open_tab(settings_window::Tab::Shortcuts, cx)
                }
                "settings-about" => settings_window::open_tab(settings_window::Tab::About, cx),
                "history" => history_window::open(cx),
                "welcome" => welcome::open(cx),
                "welcome-permission" => welcome::open_at(
                    welcome::Step::Permission,
                    Some(settings::CaptureAction::Area),
                    cx,
                ),
                "welcome-shortcuts" => welcome::open_at(welcome::Step::Shortcuts, None, cx),
                "welcome-denied" => {
                    welcome::FORCE_DENIED.store(true, std::sync::atomic::Ordering::Relaxed);
                    welcome::open_at(
                        welcome::Step::Permission,
                        Some(settings::CaptureAction::Area),
                        cx,
                    )
                }
                "recording-controls" => recorder::show_chrome_preview(cx),
                "recording-preview" => match std::env::args().nth(3) {
                    Some(p) => recorder::preview::show(p.into(), cx),
                    None => cx.quit(),
                },
                "thumbnail" => {
                    if let Some(p) = edit_path.as_deref().or(std::env::args()
                        .nth(3)
                        .map(std::path::PathBuf::from)
                        .as_deref())
                        && let Some((src, scale)) = selftest::load(p)
                    {
                        thumbnail::show(std::sync::Arc::new(src), scale, None, cx);
                    }
                }
                _ => cx.quit(),
            }
            return;
        }
        if let Some(path) = edit_path.as_deref() {
            match selftest::load(path) {
                Some((src, scale)) => {
                    editor::open_source(std::sync::Arc::new(src), scale, cx);
                    selftest::apply_ui_state(cx);
                }
                None => cx.quit(),
            }
            return;
        }
        match shell::start(cx) {
            Ok(()) => welcome::open_on_first_launch(cx),
            Err(e) => {
                eprintln!("kirei: não iniciou a barra de menus: {e:#}");
                cx.quit();
            }
        }
    });
}
