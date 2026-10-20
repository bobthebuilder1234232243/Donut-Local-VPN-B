use std::sync::Arc;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::controller::{Controller, Phase, Status};
use crate::i18n;
use crate::settings::Settings;

const ICON_OFF: &[u8] = include_bytes!("../icons/tray/off.png");
const ICON_BUSY: &[u8] = include_bytes!("../icons/tray/busy.png");
const ICON_ON: &[u8] = include_bytes!("../icons/tray/on.png");
const ICON_ERROR: &[u8] = include_bytes!("../icons/tray/error.png");

pub struct Tray {
    icon: TrayIcon,
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    open: MenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    quit: MenuItem<Wry>,
}

pub fn create(app: &AppHandle, settings: &Settings) -> tauri::Result<()> {
    let labels = i18n::labels(&settings.language);
    let status = MenuItem::with_id(app, "status", format!("FreeDPI · {}", labels.off), false, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", labels.connect, true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", labels.open, true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(app, "autostart", labels.autostart, true, settings.autostart, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &open,
            &PredefinedMenuItem::separator(app)?,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let icon = TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON_OFF)?)
        .tooltip("FreeDPI")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(Tray { icon, status, toggle, open, autostart, quit });
    Ok(())
}

fn handle_menu(app: &AppHandle, id: &str) {
    let Some(controller) = app.try_state::<Arc<Controller>>().map(|c| c.inner().clone()) else {
        return;
    };
    match id {
        "toggle" => {
            tauri::async_runtime::spawn(async move { controller.toggle().await });
        }
        "open" => show_window(app),
        "autostart" => {
            tauri::async_runtime::spawn(async move {
                let enabled = !controller.settings().autostart;
                let _ = controller.set_autostart(enabled).await;
            });
        }
        "quit" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                controller.disconnect().await;
                app.exit(0);
            });
        }
        _ => {}
    }
}

pub fn update(app: &AppHandle, status: &Status, settings: &Settings) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    let labels = i18n::labels(&settings.language);
    let phase = labels.phase(status.phase);
    let _ = tray.status.set_text(format!("FreeDPI · {phase}"));
    let _ = tray.toggle.set_text(labels.action(status.phase));
    let _ = tray.toggle.set_enabled(status.phase != Phase::Stopping);
    let _ = tray.open.set_text(labels.open);
    let _ = tray.autostart.set_text(labels.autostart);
    let _ = tray.autostart.set_checked(settings.autostart);
    let _ = tray.quit.set_text(labels.quit);
    let _ = tray.icon.set_tooltip(Some(format!("FreeDPI · {phase}")));
    let bytes = match status.phase {
        Phase::On => ICON_ON,
        Phase::Connecting | Phase::Stopping => ICON_BUSY,
        Phase::Error => ICON_ERROR,
        Phase::Off => ICON_OFF,
    };
    if let Ok(image) = Image::from_bytes(bytes) {
        let _ = tray.icon.set_icon(Some(image));
    }
}

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    crate::updater::check_soon(app);
}
