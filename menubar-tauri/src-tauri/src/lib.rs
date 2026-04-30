mod codex;
mod quota;
mod tray_commands;
mod tray_icon;

use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

use codex::{get_codex_info, get_codex_rate_limits, get_codex_stats, open_chatgpt_quota};
use quota::get_quota;
use tray_commands::{resize_window, set_dock_visibility, update_tray_icon, TrayState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(TrayState {
            tray_id: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_quota,
            update_tray_icon,
            resize_window,
            set_dock_visibility,
            get_codex_info,
            get_codex_stats,
            open_chatgpt_quota,
            get_codex_rate_limits,
        ])
        .setup(|app| {
            // Start in accessory mode for menubar behavior consistency on macOS.
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                let _ = app.set_activation_policy(ActivationPolicy::Accessory);
            }

            let icon_bytes = tray_icon::generate_tray_icon_with_ring(0, 44);
            let initial_icon = Image::from_bytes(&icon_bytes)?;

            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&quit_item]).build()?;

            let tray = match TrayIconBuilder::with_id("quota-tray")
                .icon(initial_icon)
                .icon_as_template(false)
                .tooltip("Claude Quota Monitor")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id().as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                if let Ok(Some(rect)) = tray.rect() {
                                    let window_size = window.outer_size().unwrap_or_default();
                                    let pos = match rect.position {
                                        tauri::Position::Physical(p) => (p.x, p.y),
                                        tauri::Position::Logical(l) => (l.x as i32, l.y as i32),
                                    };
                                    let tray_size = match rect.size {
                                        tauri::Size::Physical(s) => s.height,
                                        tauri::Size::Logical(l) => l.height as u32,
                                    };
                                    let x = pos.0 - (window_size.width as i32 / 2);
                                    let y = pos.1 + tray_size as i32 + 4;
                                    let _ = window.set_position(tauri::Position::Physical(
                                        tauri::PhysicalPosition { x, y },
                                    ));
                                }
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)
            {
                Ok(tray) => Some(tray),
                Err(err) => {
                    eprintln!(
                        "[Tray] Failed to create tray icon, falling back to window mode: {}",
                        err
                    );
                    #[cfg(target_os = "macos")]
                    {
                        use tauri::ActivationPolicy;
                        let _ = app.set_activation_policy(ActivationPolicy::Regular);
                    }
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    None
                }
            };

            let tray_available = tray.is_some();

            if let (Some(tray), Some(state)) = (tray.as_ref(), app.try_state::<TrayState>()) {
                let _ = tray.set_visible(true);
                if let Ok(mut guard) = state.tray_id.lock() {
                    *guard = Some(tray.id().clone());
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    if let Err(err) =
                        apply_vibrancy(&window, NSVisualEffectMaterial::Popover, None, Some(8.0))
                    {
                        eprintln!("[Window] Failed to apply vibrancy: {}", err);
                    }
                }

                if tray_available {
                    let window_clone = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Focused(false) = event {
                            let _ = window_clone.hide();
                        }
                    });
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
