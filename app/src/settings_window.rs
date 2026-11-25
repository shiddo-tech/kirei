use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, ElementId, FocusHandle, FontWeight, Global,
    Hsla, KeyDownEvent, Pixels, Point, SharedString, Stateful, TitlebarOptions, Window,
    WindowBounds, WindowHandle, WindowOptions, div, hsla, img, linear_color_stop, linear_gradient,
    point, prelude::*, px, rgb, size, transparent_black,
};

use crate::brand::surface;
use crate::l10n::{Language, l, lf};
use crate::settings::{
    self, AfterCapture, CaptureAction, CaptureSound, Format, HotKey, KEY_CLEARED, MOD_CMD,
    MOD_CONTROL, MOD_OPTION, MOD_SHIFT,
};
use crate::ui::controls::{MenuItem, menu};
use crate::ui::{Theme, icon, tooltip_view};
use crate::{brand, platform, welcome};

#[derive(Default)]
struct Current(Option<WindowHandle<SettingsWindow>>);

impl Global for Current {}

pub fn is(w: &gpui::AnyWindowHandle) -> bool {
    w.downcast::<SettingsWindow>().is_some()
}

pub fn open(cx: &mut App) {
    open_tab(Tab::General, cx)
}

pub fn open_tab(tab: Tab, cx: &mut App) {
    if let Some(h) = cx.default_global::<Current>().0
        && h.update(cx, |t, w, cx| {
            t.tab = tab;
            cx.notify();
            w.activate_window()
        })
        .is_ok()
    {
        crate::activate(cx);
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(540.), px(620.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(l("Settings…").trim_end_matches('…').to_string().into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        focus: !crate::dev_mode(),
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                SettingsWindow {
                    tab,
                    recording: None,
                    popup: None,
                    focus,
                    launch_language: crate::l10n::current(),
                }
            })
        })
        .ok();
    cx.set_global(Current(handle));
    crate::activate(cx);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    General,
    Shortcuts,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Popup {
    After,
    Sound,
    Delay,
    Format,
    Language,
}

pub struct SettingsWindow {
    tab: Tab,
    recording: Option<CaptureAction>,
    popup: Option<(Popup, Point<Pixels>)>,
    focus: FocusHandle,
    launch_language: Language,
}

/// Carbon virtual key codes for the keys gpui names.
fn carbon_key(key: &str) -> Option<u32> {
    const LETTERS: &[(&str, u32)] = &[
        ("a", 0x00),
        ("s", 0x01),
        ("d", 0x02),
        ("f", 0x03),
        ("h", 0x04),
        ("g", 0x05),
        ("z", 0x06),
        ("x", 0x07),
        ("c", 0x08),
        ("v", 0x09),
        ("b", 0x0B),
        ("q", 0x0C),
        ("w", 0x0D),
        ("e", 0x0E),
        ("r", 0x0F),
        ("y", 0x10),
        ("t", 0x11),
        ("1", 0x12),
        ("2", 0x13),
        ("3", 0x14),
        ("4", 0x15),
        ("6", 0x16),
        ("5", 0x17),
        ("=", 0x18),
        ("9", 0x19),
        ("7", 0x1A),
        ("-", 0x1B),
        ("8", 0x1C),
        ("0", 0x1D),
        ("]", 0x1E),
        ("o", 0x1F),
        ("u", 0x20),
        ("[", 0x21),
        ("i", 0x22),
        ("p", 0x23),
        ("l", 0x25),
        ("j", 0x26),
        ("'", 0x27),
        ("k", 0x28),
        (";", 0x29),
        ("\\", 0x2A),
        (",", 0x2B),
        ("/", 0x2C),
        ("n", 0x2D),
        ("m", 0x2E),
        (".", 0x2F),
        ("`", 0x32),
        ("enter", 0x24),
        ("tab", 0x30),
        ("space", 0x31),
        ("backspace", 0x33),
        ("escape", 0x35),
        ("f1", 0x7A),
        ("f2", 0x78),
        ("f3", 0x63),
        ("f4", 0x76),
        ("f5", 0x60),
        ("f6", 0x61),
        ("f7", 0x62),
        ("f8", 0x64),
        ("f9", 0x65),
        ("f10", 0x6D),
        ("f11", 0x67),
        ("f12", 0x6F),
        ("left", 0x7B),
        ("right", 0x7C),
        ("down", 0x7D),
        ("up", 0x7E),
    ];
    LETTERS.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
}

fn display_key(key: &str) -> String {
    match key {
        "space" => l("Space"),
        "enter" => "↩".into(),
        "tab" => "⇥".into(),
        "backspace" => "⌫".into(),
        "escape" => "⎋".into(),
        "left" => "←".into(),
        "right" => "→".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        k if k.starts_with('f') && k.len() > 1 && k[1..].parse::<u8>().is_ok() => k.to_uppercase(),
        k => k.to_uppercase(),
    }
}

impl SettingsWindow {
    fn record(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(action) = self.recording else { return };
        let k = &ev.keystroke;
        if k.key == "escape" {
            self.stop_recording(cx);
            return;
        }
        let m = k.modifiers;
        if !(m.platform || m.alt || m.control) {
            platform::beep();
            return;
        }
        let Some(code) = carbon_key(&k.key) else {
            platform::beep();
            return;
        };
        let mut mods = 0;
        let mut display = String::new();
        if m.control {
            mods |= MOD_CONTROL;
            display.push('⌃');
        }
        if m.alt {
            mods |= MOD_OPTION;
            display.push('⌥');
        }
        if m.shift {
            mods |= MOD_SHIFT;
            display.push('⇧');
        }
        if m.platform {
            mods |= MOD_CMD;
            display.push('⌘');
        }
        display.push_str(&display_key(&k.key));
        settings::update(|s| {
            s.hot_keys.insert(
                action,
                HotKey {
                    key_code: code,
                    modifiers: mods,
                    display,
                },
            );
        });
        self.stop_recording(cx);
        cx.stop_propagation();
    }

    fn start_recording(
        &mut self,
        action: CaptureAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Global hotkeys would swallow the keys being recorded.
        if crate::shell::is_started() {
            platform::hotkeys::unregister_all();
        }
        self.recording = Some(action);
        window.focus(&self.focus);
        cx.notify();
    }

    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        self.recording = None;
        crate::shell::register_hotkeys();
        cx.notify();
    }

    fn picker(
        &self,
        id: &'static str,
        value: String,
        popup: Popup,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        control(id, value)
            .child(icon(
                "chevron.up.chevron.down",
                9.,
                rgb(surface::MUTED).into(),
            ))
            .on_click(cx.listener(move |t, ev: &ClickEvent, _, cx| {
                t.popup = Some((popup, ev.position()));
                cx.notify()
            }))
    }

    fn popup_items(&self, popup: Popup, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let s = settings::get();
        let e = cx.entity();
        let set = move |f: Box<dyn Fn(&mut settings::AppSettings)>| {
            let e = e.clone();
            move |_: &mut Window, cx: &mut App| {
                settings::update(|s| f(s));
                e.update(cx, |_, cx| cx.notify());
            }
        };
        match popup {
            Popup::After => AfterCapture::ALL
                .into_iter()
                .map(|a| {
                    MenuItem::new(a.title(), set(Box::new(move |s| s.after_capture = a)))
                        .checked(s.after_capture == a)
                })
                .collect(),
            Popup::Sound => [CaptureSound::Voice, CaptureSound::Shutter]
                .into_iter()
                .map(|v| {
                    let f = set(Box::new(move |s| s.capture_sound = v));
                    MenuItem::new(v.title(), move |w, cx| {
                        f(w, cx);
                        crate::feedback::play_capture();
                    })
                    .checked(s.capture_sound == v)
                })
                .collect(),
            Popup::Delay => [0u32, 3, 5, 10]
                .into_iter()
                .map(|d| {
                    let title = if d == 0 {
                        l("Off")
                    } else {
                        lf("%d seconds", d)
                    };
                    MenuItem::new(title, set(Box::new(move |s| s.capture_delay = d)))
                        .checked(s.capture_delay == d)
                })
                .collect(),
            Popup::Format => Format::ALL
                .into_iter()
                .map(|f| {
                    MenuItem::new(f.title(), set(Box::new(move |s| s.format = f)))
                        .checked(s.format == f)
                })
                .collect(),
            Popup::Language => {
                let mut items = vec![
                    MenuItem::new(l("System default"), set(Box::new(|s| s.language = None)))
                        .checked(s.language.is_none()),
                ];
                for lang in [Language::English, Language::Portuguese, Language::Spanish] {
                    items.push(
                        MenuItem::new(
                            lang.native_name(),
                            set(Box::new(move |s| s.language = Some(lang.code().into()))),
                        )
                        .checked(s.language.as_deref() == Some(lang.code())),
                    );
                }
                items
            }
        }
    }

    fn general(&mut self, cx: &mut Context<Self>) -> Div {
        let s = settings::get();
        let setting = |id: &'static str,
                       on: bool,
                       f: fn(&mut settings::AppSettings, bool),
                       cx: &mut Context<Self>| {
            let e = cx.entity();
            toggle(id, on, move |v, _, cx| {
                settings::update(|s| f(s, v));
                e.update(cx, |_, cx| cx.notify());
            })
        };
        let folder = s.save_folder().to_string_lossy().to_string();
        let home = std::env::var("HOME").unwrap_or_default();
        let folder = folder.replacen(&home, "~", 1);
        let chosen_language = s.language.as_deref().and_then(Language::from_code);
        let language_label = chosen_language
            .map(|l| l.native_name().to_string())
            .unwrap_or_else(|| l("System default"));
        let effective = chosen_language.unwrap_or_else(|| {
            platform::preferred_languages()
                .iter()
                .find_map(|c| Language::from_code(c))
                .unwrap_or(Language::English)
        });
        let permission = platform::screen_capture_allowed();

        let after = group(
            l("After Capture"),
            vec![
                row(l("Action"))
                    .child(self.picker("after", s.after_capture.title(), Popup::After, cx))
                    .into_any_element(),
                row(l("Hide sensitive data automatically"))
                    .child(setting(
                        "redact",
                        s.auto_redact,
                        |s, v| s.auto_redact = v,
                        cx,
                    ))
                    .into_any_element(),
                row(l("Play sounds"))
                    .child(setting("sounds", s.play_sound, |s, v| s.play_sound = v, cx))
                    .into_any_element(),
                row(l("Capture sound"))
                    .when(!s.play_sound, |d| d.opacity(0.4))
                    .child(self.picker("sound", s.capture_sound.title(), Popup::Sound, cx))
                    .into_any_element(),
                row(l("Keep capture history"))
                    .child(setting(
                        "history",
                        s.keep_history,
                        |s, v| s.keep_history = v,
                        cx,
                    ))
                    .into_any_element(),
            ],
        );

        let delay = if s.capture_delay == 0 {
            l("Off")
        } else {
            lf("%d seconds", s.capture_delay)
        };
        let capture = group(
            l("Capture"),
            vec![
                row(l("Self-timer"))
                    .child(self.picker("delay", delay, Popup::Delay, cx))
                    .into_any_element(),
                row(l("Hide desktop icons"))
                    .child(setting(
                        "icons",
                        s.hide_desktop_icons,
                        |s, v| s.hide_desktop_icons = v,
                        cx,
                    ))
                    .into_any_element(),
            ],
        );

        let saving = group(
            l("Saving"),
            vec![
                row(l("Folder"))
                    .child(
                        div()
                            .max_w(px(190.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(rgb(surface::MUTED))
                            .child(folder),
                    )
                    .child(
                        control("folder", l("Change…")).on_click(cx.listener(|_, _, _, cx| {
                            let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                                files: false,
                                directories: true,
                                multiple: false,
                                prompt: None,
                            });
                            cx.spawn(async move |this, cx| {
                                let Ok(Ok(Some(paths))) = rx.await else {
                                    return;
                                };
                                if let Some(p) = paths.into_iter().next() {
                                    settings::update(|s| s.save_folder = Some(p));
                                    this.update(cx, |_, cx| cx.notify()).ok();
                                }
                            })
                            .detach();
                        })),
                    )
                    .into_any_element(),
                row(l("Format"))
                    .child(self.picker("format", s.format.title().into(), Popup::Format, cx))
                    .into_any_element(),
                row(l("Export at Retina size (@2x)"))
                    .id("retina")
                    .tooltip(|_, cx| {
                        tooltip_view(
                            l("Off exports at 1 pixel per point: smaller files, less sharp").into(),
                            cx,
                        )
                    })
                    .child(setting(
                        "retina-switch",
                        s.export_retina,
                        |s, v| s.export_retina = v,
                        cx,
                    ))
                    .into_any_element(),
            ],
        );

        let mut system = vec![
            row(l("Language"))
                .child(self.picker("language", language_label, Popup::Language, cx))
                .into_any_element(),
        ];
        if effective != self.launch_language {
            system.push(
                row(l("Restart Kirei to apply the new language."))
                    .text_color(rgb(surface::MUTED))
                    .child(
                        control("restart", l("Restart Now"))
                            .on_click(|_, _, cx| platform::relaunch(cx)),
                    )
                    .into_any_element(),
            );
        }
        system.push(
            row(l("Open at login"))
                .child({
                    let e = cx.entity();
                    toggle("login", platform::launch_at_login(), move |v, _, cx| {
                        platform::set_launch_at_login(v);
                        e.update(cx, |_, cx| cx.notify());
                    })
                })
                .into_any_element(),
        );
        system.push(
            row(l("Screen Recording"))
                .child(if permission {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_color(brand::green_light())
                        .child(welcome::status_dot(hsla(0.40, 0.62, 0.62, 1.)))
                        .child(l("Allowed"))
                        .into_any_element()
                } else {
                    control("grant", l("Grant Permission…"))
                        .on_click(|_, _, cx| welcome::open_at(welcome::Step::Permission, None, cx))
                        .into_any_element()
                })
                .into_any_element(),
        );

        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(after)
            .child(capture)
            .child(saving)
            .child(group(l("System"), system))
    }

    fn shortcut_row(&self, action: CaptureAction, cx: &mut Context<Self>) -> AnyElement {
        let recording = self.recording == Some(action);
        let current = settings::get()
            .hot_key(action)
            .filter(|k| !k.is_cleared())
            .cloned();
        let green_light: Hsla = brand::green_light().into();
        let keys = if recording {
            div()
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(7.))
                .bg(hsla(0.41, 0.4, 0.5, 0.12))
                .border_1()
                .border_color(green_light.opacity(0.7))
                .shadow(vec![welcome::shadow(green_light.opacity(0.25), 0., 10.)])
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(green_light)
                .child(welcome::status_dot(green_light))
                .child(l("Press the shortcut…"))
                .into_any_element()
        } else if let Some(k) = current.as_ref() {
            div()
                .flex()
                .gap(px(5.))
                .children(
                    welcome::keycaps(&k.display)
                        .into_iter()
                        .map(welcome::keycap),
                )
                .into_any_element()
        } else {
            div()
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(7.))
                .border_1()
                .border_color(rgb(surface::CONTROL_BORDER))
                .text_size(px(12.))
                .text_color(rgb(surface::FAINT))
                .child(l("None"))
                .into_any_element()
        };
        let target = div()
            .id(("hk", action as usize))
            .flex_none()
            .p(px(3.))
            .rounded(px(10.))
            .border_1()
            .border_color(transparent_black())
            .cursor_pointer()
            .when(!recording, |d| {
                d.hover(|s| {
                    s.bg(rgb(surface::CONTROL))
                        .border_color(rgb(surface::CARD_BORDER))
                })
            })
            .child(keys)
            .on_click(cx.listener(move |t, _, window, cx| {
                if t.recording == Some(action) {
                    t.stop_recording(cx)
                } else {
                    t.start_recording(action, window, cx)
                }
            }));
        let clear = div()
            .flex_none()
            .w(px(16.))
            .when(current.is_some() && !recording, |d| {
                d.child(
                    div()
                        .id(("clear", action as usize))
                        .cursor_pointer()
                        .opacity(0.7)
                        .hover(|s| s.opacity(1.))
                        .child(icon("xmark.circle.fill", 13., rgb(surface::FAINT).into()))
                        .tooltip(|_, cx| tooltip_view(l("Remove shortcut").into(), cx))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            settings::update(|s| {
                                s.hot_keys.insert(
                                    action,
                                    HotKey {
                                        key_code: KEY_CLEARED,
                                        modifiers: 0,
                                        display: String::new(),
                                    },
                                );
                            });
                            crate::shell::register_hotkeys();
                            cx.notify();
                        })),
                )
            });
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pl(px(14.))
            .pr(px(12.))
            .py(px(9.))
            .child(
                div()
                    .flex_none()
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(9.))
                    .bg(hsla(0.41, 0.4, 0.5, 0.12))
                    .border_1()
                    .border_color(hsla(0.41, 0.4, 0.6, 0.18))
                    .child(icon(action.symbol(), 14., green_light)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(surface::TEXT))
                    .child(action.title()),
            )
            .child(target)
            .child(clear)
            .into_any_element()
    }

    fn shortcuts(&mut self, cx: &mut Context<Self>) -> Div {
        let rows = CaptureAction::ALL
            .into_iter()
            .map(|a| self.shortcut_row(a, cx))
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(group(l("Global Shortcuts"), rows))
            .child(
                div()
                    .px(px(4.))
                    .text_size(px(12.))
                    .text_color(rgb(surface::FAINT))
                    .child(l("Click a shortcut and press the new keys. Esc cancels.")),
            )
    }

    fn about(&mut self) -> Div {
        let version = lf("Version %@", env!("CARGO_PKG_VERSION"));
        let version = version.replace(" (%@)", "");
        let banner = div()
            .relative()
            .flex_none()
            .w_full()
            .h(px(184.))
            .rounded(px(16.))
            .overflow_hidden()
            .border_1()
            .border_color(hsla(0.41, 0.4, 0.6, 0.25))
            .bg(linear_gradient(
                155.,
                linear_color_stop(rgb(0x6cc095), 0.),
                linear_color_stop(rgb(0x1d4a33), 1.),
            ))
            .shadow(vec![welcome::shadow(hsla(0.42, 0.6, 0.08, 0.5), 10., 30.)])
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .absolute()
                    .top(px(-30.))
                    .left(px(140.))
                    .size(px(210.))
                    .rounded_full()
                    .bg(welcome::white(0.07)),
            )
            .child(
                div()
                    .relative()
                    .w(px(150.))
                    .h(px(118.))
                    .rounded(px(3.))
                    .bg(welcome::white(0.07))
                    .border_1()
                    .border_color(welcome::white(0.22))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(welcome::corner(true, true))
                    .child(welcome::corner(true, false))
                    .child(welcome::corner(false, true))
                    .child(welcome::corner(false, false))
                    .child(
                        div()
                            .size(px(78.))
                            .rounded(px(18.))
                            .shadow(vec![
                                welcome::shadow(hsla(0.42, 0.6, 0.08, 0.55), 14., 28.),
                                welcome::shadow(hsla(0., 0., 0., 0.25), 3., 8.),
                            ])
                            .child(img(brand::app_icon()).size_full()),
                    ),
            );
        let heading = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(brand::green_light())
                    .child("綺麗")
                    .child(div().text_color(rgb(0x4d5650)).child("·"))
                    .child(l("beautiful, in Japanese")),
            )
            .child(
                div()
                    .text_size(px(30.))
                    .line_height(px(36.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(surface::TEXT_STRONG))
                    .child("Kirei"),
            )
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(rgb(surface::MUTED))
                    .child(l("Beautiful, share-ready screenshots in one shortcut.")),
            )
            .child(
                div()
                    .mt(px(6.))
                    .px(px(10.))
                    .py(px(3.))
                    .rounded_full()
                    .bg(rgb(surface::CARD))
                    .border_1()
                    .border_color(rgb(surface::CARD_BORDER))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(surface::MUTED))
                    .child(version),
            );
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(22.))
            .child(banner)
            .child(heading)
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(
                        welcome::button("tour", l("Show Welcome Tour"), false)
                            .on_click(|_, _, cx| welcome::open(cx)),
                    )
                    .child(
                        welcome::button("diag", l("Diagnostics Folder"), false).on_click(
                            |_, _, cx| {
                                let dir = crate::diagnostics::folder();
                                std::fs::create_dir_all(&dir).ok();
                                cx.open_with_system(&dir)
                            },
                        ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0x4d5650))
                    .child("© 2026 Shiddo"),
            )
    }
}

/// The app theme recolored to the brand surface, for the controls and menus shared with other
/// windows.
fn surface_theme() -> Theme {
    let mut t = Theme::new(true);
    t.window = rgb(surface::BG).into();
    t.text = rgb(surface::TEXT).into();
    t.secondary = rgb(surface::MUTED).into();
    t.tertiary = rgb(surface::FAINT).into();
    t.border = rgb(surface::CARD_BORDER).into();
    t.fill = rgb(surface::CONTROL).into();
    t.fill_strong = rgb(surface::CONTROL_HOVER).into();
    t.accent = brand::green().into();
    t.accent_soft = Hsla {
        a: 0.16,
        ..t.accent
    };
    t
}

fn control(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(28.))
        .px(px(11.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(7.))
        .rounded(px(8.))
        .bg(rgb(surface::CONTROL))
        .border_1()
        .border_color(rgb(surface::CONTROL_BORDER))
        .shadow(vec![welcome::shadow(hsla(0., 0., 0., 0.35), 1., 2.)])
        .text_size(px(13.))
        .text_color(rgb(0xdfe5e0))
        .cursor_pointer()
        .hover(|s| {
            s.bg(rgb(surface::CONTROL_HOVER))
                .border_color(rgb(0x3a3f3b))
        })
        .child(label.into())
}

fn toggle(
    id: impl Into<ElementId>,
    on: bool,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(36.))
        .h(px(21.))
        .p(px(2.))
        .rounded_full()
        .border_1()
        .flex()
        .when(on, |d| d.justify_end())
        .map(|d| {
            if on {
                d.bg(linear_gradient(
                    180.,
                    linear_color_stop(rgb(0x4fa77a), 0.),
                    linear_color_stop(brand::green(), 1.),
                ))
                .border_color(rgb(0x5cb487))
                .shadow(vec![welcome::shadow(hsla(0.41, 0.5, 0.35, 0.4), 2., 8.)])
            } else {
                d.bg(rgb(0x262a27))
                    .border_color(rgb(surface::CONTROL_BORDER))
            }
        })
        .cursor_pointer()
        .child(
            div()
                .size(px(15.))
                .rounded_full()
                .bg(hsla(0., 0., 1., 1.))
                .shadow(vec![welcome::shadow(hsla(0., 0., 0., 0.35), 1., 3.)]),
        )
        .on_click(move |_, window, cx| on_toggle(!on, window, cx))
}

fn group(title: String, rows: Vec<AnyElement>) -> Div {
    let mut card = div()
        .flex()
        .flex_col()
        .rounded(px(14.))
        .bg(rgb(surface::CARD))
        .border_1()
        .border_color(rgb(surface::CARD_BORDER))
        .shadow(vec![welcome::shadow(hsla(0., 0., 0., 0.3), 6., 18.)]);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            card = card.child(div().mx_4().h(px(1.)).bg(rgb(surface::DIVIDER)));
        }
        card = card.child(r);
    }
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .px(px(4.))
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(surface::FAINT))
                .child(title),
        )
        .child(card)
}

fn row(title: impl Into<SharedString>) -> Div {
    div()
        .min_h(px(46.))
        .px(px(16.))
        .py(px(8.))
        .flex()
        .items_center()
        .gap(px(10.))
        .text_size(px(13.))
        .text_color(rgb(surface::TEXT))
        .child(div().flex_1().child(title.into()))
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = surface_theme();
        let green_light: Hsla = brand::green_light().into();
        let tabs = [
            (Tab::General, l("General"), "gearshape"),
            (Tab::Shortcuts, l("Shortcuts"), "command"),
            (Tab::About, l("About"), "info.circle"),
        ];
        let mut bar = div()
            .flex()
            .gap(px(2.))
            .p(px(3.))
            .rounded(px(12.))
            .bg(rgb(surface::CARD))
            .border_1()
            .border_color(rgb(surface::CARD_BORDER))
            .shadow(vec![welcome::shadow(hsla(0., 0., 0., 0.3), 4., 14.)]);
        for (i, (tab, title, symbol)) in tabs.into_iter().enumerate() {
            let active = self.tab == tab;
            bar = bar.child(
                div()
                    .id(("tab", i))
                    .w(px(88.))
                    .py(px(6.))
                    .rounded(px(9.))
                    .border_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(3.))
                    .cursor_pointer()
                    .map(|d| {
                        if active {
                            d.bg(hsla(0.41, 0.4, 0.5, 0.14))
                                .border_color(hsla(0.41, 0.45, 0.6, 0.28))
                        } else {
                            d.border_color(transparent_black())
                                .hover(|s| s.bg(rgb(surface::CONTROL)))
                        }
                    })
                    .child(
                        icon(
                            symbol,
                            17.,
                            if active {
                                green_light
                            } else {
                                rgb(surface::FAINT).into()
                            },
                        )
                        .h(px(22.)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if active {
                                rgb(surface::TEXT)
                            } else {
                                rgb(surface::MUTED)
                            })
                            .child(title),
                    )
                    .on_click(cx.listener(move |t, _, _, cx| {
                        t.tab = tab;
                        t.popup = None;
                        cx.notify()
                    })),
            );
        }
        let body: AnyElement = match self.tab {
            Tab::General => self.general(cx).into_any_element(),
            Tab::Shortcuts => self.shortcuts(cx).into_any_element(),
            Tab::About => self.about().into_any_element(),
        };
        let popup = self.popup.map(|(p, at)| {
            let e = cx.entity();
            menu(at, self.popup_items(p, cx), &theme, move |_, cx| {
                e.update(cx, |t, cx| {
                    t.popup = None;
                    cx.notify()
                })
            })
        });
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|t, ev: &KeyDownEvent, _, cx| t.record(ev, cx)))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(surface::BG))
            .text_color(rgb(surface::TEXT))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(160.))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(hsla(0.41, 0.45, 0.45, 0.13), 0.),
                        linear_color_stop(hsla(0.41, 0.45, 0.45, 0.), 1.),
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .pt(px(12.))
                    .pb(px(6.))
                    .child(bar),
            )
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .pt(px(14.))
                    .pb(px(24.))
                    .child(body),
            )
            .when_some(popup, |d, p| d.child(p))
    }
}
