//! Window/tray Tauri commands and the shared `TrayState` handle.

use std::sync::Mutex;
use tauri::{image::Image, tray::TrayIconId, AppHandle, LogicalSize, Manager, State};

use crate::tray_icon;

pub struct TrayState {
    pub tray_id: Mutex<Option<TrayIconId>>,
}

#[tauri::command]
pub async fn resize_window(app: AppHandle, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let size = LogicalSize::new(320.0, height);
        window.set_size(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_dock_visibility(app: AppHandle, visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri::ActivationPolicy;
        if visible {
            let _ = app.set_activation_policy(ActivationPolicy::Regular);
        } else {
            let _ = app.set_activation_policy(ActivationPolicy::Accessory);
        }
    }
    // Reference `app` and `visible` on non-macOS to keep the signature unified
    // without dead-code warnings.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, visible);
    }
    Ok(())
}

#[tauri::command]
pub async fn update_tray_icon(
    app: AppHandle,
    tray_state: State<'_, TrayState>,
    percentage: u8,
) -> Result<(), String> {
    println!("[Tray] Updating icon with percentage: {}", percentage);

    let tray_id = {
        let guard = tray_state.tray_id.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };

    if let Some(id) = tray_id {
        if let Some(tray) = app.tray_by_id(&id) {
            let icon_bytes = tray_icon::generate_tray_icon_with_ring(percentage, 44);
            let icon = Image::from_bytes(&icon_bytes).map_err(|e| e.to_string())?;
            tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;
            tray.set_icon_as_template(false).map_err(|e| e.to_string())?;

            let tooltip = format!("Claude Quota Monitor ({percentage}%)");
            tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;
            tray.set_visible(true).map_err(|e| e.to_string())?;
            println!("[Tray] Icon + tooltip updated: {}%", percentage);
        } else {
            println!("[Tray] Tray not found by ID");
        }
    } else {
        println!("[Tray] No tray ID stored");
    }

    Ok(())
}
