#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod apps;
mod autostart;
mod catalog;
mod commands;
mod controller;
mod engine;
mod i18n;
mod log;
mod network;
mod notify;
mod probe;
mod settings;
mod system;
mod tray;
mod updater;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};

use controller::Controller;

fn main() {
    let start_in_tray = std::env::args().any(|arg| arg == autostart::TRAY_ARG);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(updater::Pending::default())
        .setup(move |app| {
            if let Ok(dir) = app.path().app_log_dir() {
                log::init(dir);
            }
            log::write(format!("FreeDPI {} started on {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS));
            updater::watch(app.handle().clone());
            let controller = Arc::new(Controller::new(app.handle())?);
            tray::create(app.handle(), &controller.settings())?;
            app.manage(controller.clone());
            network::watch(controller.clone());
            if start_in_tray {
                tauri::async_runtime::spawn(async move { controller.connect().await });
            } else {
                tray::show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::init,
            commands::connect,
            commands::disconnect,
            commands::save_preferences,
            commands::set_language,
            commands::set_autostart,
            commands::set_notifications,
            commands::set_after_vpn,
            commands::quit,
            commands::install_update,
            commands::open_discord,
            commands::open_repository,
            commands::report_issue,
            commands::diagnostics,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start FreeDPI");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(controller) = app.try_state::<Arc<Controller>>() {
                tauri::async_runtime::block_on(controller.disconnect());
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tray::show_window(app),
        _ => {}
    });
}
