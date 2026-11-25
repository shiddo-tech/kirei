use std::time::Duration;

use gpui::{App, AsyncApp, Global, KeyBinding, Menu as AppMenu, MenuItem as AppMenuItem, actions};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::l10n::l;
use crate::settings::{self, CaptureAction};
use crate::{brand, editor, flow, pinned, platform};

actions!(
    kirei_app,
    [
        Quit,
        Hide,
        About,
        OpenSettings,
        OpenImage,
        NewFromClipboard,
        OpenHistory,
        CaptureArea,
        CaptureFullscreen,
        CaptureText
    ]
);

#[derive(Clone, Copy)]
enum TrayAction {
    Capture(CaptureAction),
    StopRecording,
    OpenImage,
    History,
    ClosePins,
    Settings,
    Quit,
}

struct Shell {
    tray: TrayIcon,
    menu: Vec<(MenuId, TrayAction)>,
    signature: String,
    recording: bool,
}

impl Global for Shell {}

/// Hotkey ids follow `CaptureAction::ALL` order, starting at 1.
fn hotkey_id(action: CaptureAction) -> u32 {
    CaptureAction::ALL
        .iter()
        .position(|a| *a == action)
        .unwrap_or(0) as u32
        + 1
}

static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn is_started() -> bool {
    STARTED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn register_hotkeys() {
    // Dev windows (`--show`) must never grab the user's global shortcuts.
    if !STARTED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    platform::hotkeys::unregister_all();
    let s = settings::get();
    for action in CaptureAction::ALL {
        if !flow::is_ported(action) {
            continue;
        }
        if let Some(hk) = s.hot_key(action)
            && let Err(e) =
                platform::hotkeys::register(hotkey_id(action), hk.key_code, hk.modifiers)
        {
            eprintln!("kirei: atalho {} não registrado: {e:#}", hk.display);
        }
    }
}

fn menu_signature(cx: &App) -> String {
    let s = settings::get();
    format!(
        "{:?}|{}|{}|{}",
        s.hot_keys,
        pinned::count(cx),
        crate::recorder::is_recording(cx),
        crate::l10n::current().code()
    )
}

fn build_menu(cx: &App) -> anyhow::Result<(Menu, Vec<(MenuId, TrayAction)>)> {
    let s = settings::get();
    let menu = Menu::new();
    let mut ids = Vec::new();
    let mut add = |menu: &Menu, title: String, action: TrayAction| -> anyhow::Result<()> {
        let item = MenuItem::new(title, true, None);
        ids.push((item.id().clone(), action));
        menu.append(&item)?;
        Ok(())
    };
    if crate::recorder::is_recording(cx) {
        add(&menu, l("Stop Recording"), TrayAction::StopRecording)?;
        menu.append(&PredefinedMenuItem::separator())?;
    }
    for action in CaptureAction::ALL {
        if !flow::is_ported(action) {
            continue;
        }
        let title = match s.hot_key(action) {
            Some(hk) => format!("{}   {}", action.title(), hk.display),
            None => action.title(),
        };
        add(&menu, title, TrayAction::Capture(action))?;
        if action == CaptureAction::Text {
            menu.append(&PredefinedMenuItem::separator())?;
        }
    }
    if !menu.items().is_empty()
        && !matches!(
            menu.items()
                .last()
                .map(|i| i.as_predefined_menuitem().is_some()),
            Some(true)
        )
    {
        menu.append(&PredefinedMenuItem::separator())?;
    }
    add(&menu, l("Open Image…"), TrayAction::OpenImage)?;
    add(&menu, l("Capture History…"), TrayAction::History)?;
    if pinned::count(cx) > 0 {
        add(&menu, l("Close Pinned Images"), TrayAction::ClosePins)?;
    }
    menu.append(&PredefinedMenuItem::separator())?;
    add(&menu, l("Settings…"), TrayAction::Settings)?;
    add(&menu, l("Quit Kirei"), TrayAction::Quit)?;
    Ok((menu, ids))
}

fn tray_glyph() -> Icon {
    const SIZE: u32 = 36;
    Icon::from_rgba(brand::mark_rgba(SIZE), SIZE, SIZE).expect("ícone da barra de menus")
}

/// While recording, the menu bar icon becomes a stop symbol, like the Swift app.
fn tray_icon(recording: bool) -> Icon {
    if recording
        && let Some((w, h, mut px)) =
            platform::render_symbol("stop.circle.fill", 16., 0., [0, 0, 0, 255], 2.)
    {
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        if let Ok(icon) = Icon::from_rgba(px, w, h) {
            return icon;
        }
    }
    tray_glyph()
}

pub fn start(cx: &mut App) -> anyhow::Result<()> {
    platform::hotkeys::install()?;
    STARTED.store(true, std::sync::atomic::Ordering::Relaxed);
    register_hotkeys();
    let (menu, ids) = build_menu(cx)?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(tray_glyph())
        .with_icon_as_template(true)
        .with_tooltip("Kirei")
        .build()?;
    let signature = menu_signature(cx);
    cx.set_global(Shell {
        tray,
        menu: ids,
        signature,
        recording: false,
    });
    install_app_menus(cx);
    pump_events(cx);
    Ok(())
}

fn refresh_menu(cx: &mut App) {
    let signature = menu_signature(cx);
    if cx.global::<Shell>().signature == signature {
        return;
    }
    let recording = crate::recorder::is_recording(cx);
    if let Ok((menu, ids)) = build_menu(cx) {
        let shell = cx.global_mut::<Shell>();
        if shell.recording != recording {
            shell.recording = recording;
            shell.tray.set_icon(Some(tray_icon(recording))).ok();
            shell.tray.set_icon_as_template(true);
        }
        shell.tray.set_menu(Some(Box::new(menu)));
        shell.menu = ids;
        shell.signature = signature;
    }
}

fn run(action: TrayAction, cx: &mut App) {
    match action {
        TrayAction::Capture(a) => flow::perform(a, cx),
        TrayAction::StopRecording => crate::recorder::stop(cx),
        TrayAction::OpenImage => flow::open_image(cx),
        TrayAction::History => crate::history_window::open(cx),
        TrayAction::ClosePins => pinned::close_all(cx),
        TrayAction::Settings => crate::settings_window::open(cx),
        TrayAction::Quit => cx.quit(),
    }
}

fn pump_events(cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
            let hotkeys = platform::hotkeys::drain();
            let mut chosen = Vec::new();
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                chosen.push(event.id);
            }
            let alive = cx.update(|cx| {
                for id in hotkeys {
                    if id == crate::thumbnail::COPY_HOTKEY {
                        crate::thumbnail::copy_current(cx);
                    } else if let Some(action) = CaptureAction::ALL.get(id as usize - 1) {
                        flow::perform(*action, cx);
                    }
                }
                for id in chosen {
                    let action = cx
                        .global::<Shell>()
                        .menu
                        .iter()
                        .find(|(m, _)| *m == id)
                        .map(|(_, a)| *a);
                    if let Some(a) = action {
                        run(a, cx);
                    }
                }
                refresh_menu(cx);
                update_activation_policy(cx);
            });
            if alive.is_err() {
                break;
            }
        }
    })
    .detach();
}

/// Like the Swift app: a Dock icon and menu bar only while a regular window is open.
fn update_activation_policy(cx: &mut App) {
    let regular = cx.windows().iter().any(|w| {
        w.downcast::<editor::Editor>().is_some()
            || crate::history_window::is(w)
            || crate::settings_window::is(w)
            || crate::welcome::is(w)
    });
    platform::set_regular_policy(regular);
}

fn install_app_menus(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &About, cx| crate::settings_window::open(cx));
    cx.on_action(|_: &OpenSettings, cx| crate::settings_window::open(cx));
    cx.on_action(|_: &OpenImage, cx| flow::open_image(cx));
    cx.on_action(|_: &NewFromClipboard, cx| flow::perform(CaptureAction::Clipboard, cx));
    cx.on_action(|_: &OpenHistory, cx| crate::history_window::open(cx));
    cx.on_action(|_: &CaptureArea, cx| flow::perform(CaptureAction::Area, cx));
    cx.on_action(|_: &CaptureFullscreen, cx| flow::perform(CaptureAction::Fullscreen, cx));
    cx.on_action(|_: &CaptureText, cx| flow::perform(CaptureAction::Text, cx));
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-o", OpenImage, None),
        KeyBinding::new("cmd-n", NewFromClipboard, None),
        KeyBinding::new("cmd-y", OpenHistory, None),
    ]);
    use editor::*;
    cx.set_menus(vec![
        AppMenu {
            name: "Kirei".into(),
            items: vec![
                AppMenuItem::action(l("About Kirei"), About),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Settings…"), OpenSettings),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Hide Kirei"), Hide),
                AppMenuItem::action(l("Quit Kirei"), Quit),
            ],
        },
        AppMenu {
            name: l("File").into(),
            items: vec![
                AppMenuItem::action(CaptureAction::Area.title(), CaptureArea),
                AppMenuItem::action(CaptureAction::Fullscreen.title(), CaptureFullscreen),
                AppMenuItem::action(CaptureAction::Text.title(), CaptureText),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Open Image…"), OpenImage),
                AppMenuItem::action(l("New from Clipboard"), NewFromClipboard),
                AppMenuItem::action(l("Capture History…"), OpenHistory),
                AppMenuItem::action(l("Pin to Screen"), Pin),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Save"), Save),
                AppMenuItem::action(l("Save As…"), SaveAs),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Close"), CloseWindow),
            ],
        },
        AppMenu {
            name: l("Edit").into(),
            items: vec![
                AppMenuItem::action(l("Undo"), Undo),
                AppMenuItem::action(l("Redo"), Redo),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Copy"), CopyImage),
                AppMenuItem::action(l("Select All"), SelectAll),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Duplicate"), Duplicate),
                AppMenuItem::action(l("Bring to Front"), BringToFront),
                AppMenuItem::action(l("Send to Back"), SendToBack),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Copy Text (OCR)"), CopyText),
                AppMenuItem::action(l("Hide Sensitive Data"), Redact),
            ],
        },
        AppMenu {
            name: l("View").into(),
            items: vec![
                AppMenuItem::action(l("Zoom In"), ZoomIn),
                AppMenuItem::action(l("Zoom Out"), ZoomOut),
                AppMenuItem::action(l("Actual Size"), ActualSize),
                AppMenuItem::action(l("Fit in Window"), ZoomToFit),
                AppMenuItem::separator(),
                AppMenuItem::action(l("Show or hide inspector"), ToggleInspector),
            ],
        },
    ]);
}
