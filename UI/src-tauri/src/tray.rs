use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

use crate::{utils::api::close_app, GLOBAL_TRAY};

pub fn create_tray_menu(
    app: &AppHandle<Wry>,
) -> Result<Menu<Wry>, Box<dyn std::error::Error>> {
    let show_window = MenuItem::with_id(app, "show-window", "显示窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::new(app)?;
    menu.append_items(&[&show_window, &quit])?;

    Ok(menu)
}

pub fn create_system_tray(
    app: &AppHandle<Wry>,
) -> Result<Arc<TrayIcon<Wry>>, Box<dyn std::error::Error>> {
    let menu = create_tray_menu(app)?;
    let tray = Arc::new(
        TrayIconBuilder::new()
            .icon(app.default_window_icon().unwrap().clone())
            .menu(&menu)
            .show_menu_on_left_click(false)
            .tooltip("facewinunlock-tauri")
            .build(app)?,
    );

    *GLOBAL_TRAY.lock().unwrap() = Some(tray.clone());

    let window = app.get_webview_window("main").unwrap();

    tray.on_menu_event(move |app, event| match event.id.as_ref() {
        "show-window" => {
            let _ = window.show();
            let _ = window.set_focus();
        }
        "quit" => {
            let _ = close_app(app.clone());
        }
        _ => {
            let _ = window.emit("menu-event", format!("unknow id {:?}", event.id().as_ref()));
        }
    });

    tray.on_tray_icon_event(|tray, event| match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            let app = tray.app_handle();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {}
    });
    Ok(tray)
}
