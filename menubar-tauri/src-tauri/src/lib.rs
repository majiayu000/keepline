mod codex;
mod cost;
mod panel_position;
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
use cost::get_cost_overview;
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
            get_cost_overview,
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
                                    let panel = window.outer_size().unwrap_or_default();
                                    // Tauri may return Logical coords on HiDPI / Wayland; scale
                                    // to physical so they match monitor positions and panel size.
                                    let scale = window.scale_factor().unwrap_or(1.0);
                                    let (tray_x, tray_y) = match rect.position {
                                        tauri::Position::Physical(p) => (p.x, p.y),
                                        tauri::Position::Logical(l) => {
                                            ((l.x * scale) as i32, (l.y * scale) as i32)
                                        }
                                    };
                                    let (tray_w, tray_h) = match rect.size {
                                        tauri::Size::Physical(s) => (s.width, s.height),
                                        tauri::Size::Logical(l) => {
                                            ((l.width * scale) as u32, (l.height * scale) as u32)
                                        }
                                    };

                                    // Pick the monitor whose area contains the tray center;
                                    // fall back to the first monitor if none matches.
                                    let tray_cx = tray_x + tray_w as i32 / 2;
                                    let tray_cy = tray_y + tray_h as i32 / 2;
                                    let monitors =
                                        app.available_monitors().unwrap_or_default();
                                    let monitor = monitors
                                        .iter()
                                        .find(|m| {
                                            let p = m.position();
                                            let s = m.size();
                                            tray_cx >= p.x
                                                && tray_cx < p.x + s.width as i32
                                                && tray_cy >= p.y
                                                && tray_cy < p.y + s.height as i32
                                        })
                                        .or_else(|| monitors.first());

                                    let (work_x, work_y, work_w, work_h) = monitor
                                        .map(|m| {
                                            let wa = m.work_area();
                                            (
                                                wa.position.x,
                                                wa.position.y,
                                                wa.size.width,
                                                wa.size.height,
                                            )
                                        })
                                        .unwrap_or((0, 0, u32::MAX / 2, u32::MAX / 2));

                                    let (x, y) = panel_position::compute_panel_position(
                                        panel_position::LayoutInput {
                                            tray_x,
                                            tray_y,
                                            tray_w,
                                            tray_h,
                                            panel_w: panel.width,
                                            panel_h: panel.height,
                                            work_x,
                                            work_y,
                                            work_w,
                                            work_h,
                                        },
                                    );
                                    if let Err(e) = window.set_position(
                                        tauri::Position::Physical(
                                            tauri::PhysicalPosition { x, y },
                                        ),
                                    ) {
                                        eprintln!(
                                            "[Tray] Failed to set panel position: {}",
                                            e
                                        );
                                    }
                                }
                                if let Err(e) = window.show() {
                                    eprintln!("[Tray] Failed to show panel: {}", e);
                                }
                                if let Err(e) = window.set_focus() {
                                    eprintln!("[Tray] Failed to focus panel: {}", e);
                                }
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
