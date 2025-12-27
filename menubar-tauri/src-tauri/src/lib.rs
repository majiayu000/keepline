mod tray_icon;

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent, TrayIconId},
    AppHandle, LogicalSize, Manager, State,
};
use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

// Global state to store tray icon ID
struct TrayState {
    tray_id: Mutex<Option<TrayIconId>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageInfo {
    pub used: f64,
    pub limit: f64,
    pub percentage: f64,
    #[serde(rename = "resetTime")]
    pub reset_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuotaData {
    pub connected: bool,
    pub session: Option<UsageInfo>,
    #[serde(rename = "weeklyTotal")]
    pub weekly_total: Option<UsageInfo>,
    #[serde(rename = "weeklyOpus")]
    pub weekly_opus: Option<UsageInfo>,
    #[serde(rename = "weeklySonnet")]
    pub weekly_sonnet: Option<UsageInfo>,
    pub error: Option<String>,
}

// Read OAuth token from macOS Keychain
fn get_oauth_token() -> Result<String, String> {
    // Try multiple possible credential names in macOS Keychain
    let credential_names = [
        "Claude Code-credentials",
        "claude-credentials",
        "Claude-credentials",
        "claudecode-credentials",
    ];

    for cred_name in credential_names {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", cred_name, "-w"])
            .output();

        if let Ok(result) = output {
            if result.status.success() {
                let creds_json = String::from_utf8_lossy(&result.stdout).trim().to_string();
                if !creds_json.is_empty() {
                    // Parse credentials JSON to extract access token
                    if let Ok(creds) = serde_json::from_str::<serde_json::Value>(&creds_json) {
                        if let Some(token) = creds["claudeAiOauth"]["accessToken"].as_str() {
                            return Ok(token.to_string());
                        }
                    }
                }
            }
        }
    }

    Err("OAuth token not found. Please ensure you are logged into Claude Code.".to_string())
}

#[tauri::command]
async fn get_quota() -> Result<QuotaData, String> {
    // Step 1: Get OAuth token from Keychain
    let access_token = match get_oauth_token() {
        Ok(token) => token,
        Err(e) => {
            return Ok(QuotaData {
                connected: false,
                session: None,
                weekly_total: None,
                weekly_opus: None,
                weekly_sonnet: None,
                error: Some(e),
            });
        }
    };

    // Step 2: Call Anthropic API directly
    let client = reqwest::Client::new();

    match client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        // Check for API error
                        if data["error"].is_object() {
                            let error_msg = data["error"]["message"]
                                .as_str()
                                .unwrap_or("API error");
                            return Ok(QuotaData {
                                connected: false,
                                session: None,
                                weekly_total: None,
                                weekly_opus: None,
                                weekly_sonnet: None,
                                error: Some(format!("{} (Token may be expired)", error_msg)),
                            });
                        }

                        // Parse Anthropic API structure
                        let session = parse_quota_window(&data["five_hour"]);
                        let weekly_total = parse_quota_window(&data["seven_day"]);
                        let weekly_opus = parse_quota_window(&data["seven_day_opus"]);
                        let weekly_sonnet = parse_quota_window(&data["seven_day_sonnet"]);

                        Ok(QuotaData {
                            connected: true,
                            session,
                            weekly_total,
                            weekly_opus,
                            weekly_sonnet,
                            error: None,
                        })
                    }
                    Err(e) => Ok(QuotaData {
                        connected: false,
                        session: None,
                        weekly_total: None,
                        weekly_opus: None,
                        weekly_sonnet: None,
                        error: Some(format!("Failed to parse response: {}", e)),
                    }),
                }
            } else if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
                Ok(QuotaData {
                    connected: false,
                    session: None,
                    weekly_total: None,
                    weekly_opus: None,
                    weekly_sonnet: None,
                    error: Some("Token expired. Please re-login to Claude Code.".to_string()),
                })
            } else {
                Ok(QuotaData {
                    connected: false,
                    session: None,
                    weekly_total: None,
                    weekly_opus: None,
                    weekly_sonnet: None,
                    error: Some(format!("API error: {}", response.status())),
                })
            }
        }
        Err(e) => Ok(QuotaData {
            connected: false,
            session: None,
            weekly_total: None,
            weekly_opus: None,
            weekly_sonnet: None,
            error: Some(format!("Network error: {}", e)),
        }),
    }
}

// Resize window to fit content
#[tauri::command]
async fn resize_window(app: AppHandle, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let size = LogicalSize::new(320.0, height);
        window.set_size(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// Hide or show dock icon (macOS only)
#[tauri::command]
async fn set_dock_visibility(visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use cocoa::appkit::{NSApplication, NSApplicationActivationPolicy};
        use cocoa::base::nil;

        unsafe {
            let app = NSApplication::sharedApplication(nil);
            if visible {
                app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular);
            } else {
                app.setActivationPolicy_(NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory);
            }
        }
    }
    Ok(())
}

// Update tray icon with percentage
#[tauri::command]
async fn update_tray_icon(
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
            // Generate ring icon with number (44x44 for Retina displays)
            let icon_bytes = tray_icon::generate_tray_icon_with_ring(percentage, 44);
            println!("[Tray] Generated ring icon: {} bytes", icon_bytes.len());

            // Create Tauri image from PNG bytes
            let icon = Image::from_bytes(&icon_bytes).map_err(|e| {
                println!("[Tray] Error creating image: {}", e);
                e.to_string()
            })?;

            // Update tray icon
            tray.set_icon(Some(icon)).map_err(|e| {
                println!("[Tray] Error setting icon: {}", e);
                e.to_string()
            })?;

            // Disable template mode to show colored icon
            tray.set_icon_as_template(false).map_err(|e| e.to_string())?;

            println!("[Tray] Icon updated successfully");
        } else {
            println!("[Tray] Tray not found by ID");
        }
    } else {
        println!("[Tray] No tray ID stored");
    }

    Ok(())
}

// Parse a QuotaWindow from Anthropic API (has utilization and resets_at)
fn parse_quota_window(value: &serde_json::Value) -> Option<UsageInfo> {
    if value.is_null() || !value.is_object() {
        return None;
    }

    let utilization = value["utilization"].as_f64().unwrap_or(0.0);
    let resets_at = value["resets_at"].as_str().map(|s| s.to_string());

    Some(UsageInfo {
        used: utilization,
        limit: 100.0,
        percentage: utilization,
        reset_time: resets_at,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(TrayState {
            tray_id: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![get_quota, update_tray_icon, resize_window, set_dock_visibility])
        .setup(|app| {
            // Generate initial ring icon showing 0%
            let initial_icon_bytes = tray_icon::generate_tray_icon_with_ring(0, 44);
            let initial_icon = Image::from_bytes(&initial_icon_bytes)
                .expect("Failed to create initial icon");
            println!("[Tray] Created initial ring icon: {} bytes", initial_icon_bytes.len());

            // Create right-click menu
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&quit_item]).build()?;

            // Setup system tray
            let tray = TrayIconBuilder::new()
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
                                // Position window near tray icon
                                if let Ok(Some(rect)) = tray.rect() {
                                    let window_size = window.outer_size().unwrap_or_default();
                                    // Convert Position and Size to physical values
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
                .build(app)?;

            // Store tray ID for later updates
            if let Some(state) = app.try_state::<TrayState>() {
                if let Ok(mut guard) = state.tray_id.lock() {
                    *guard = Some(tray.id().clone());
                }
            }

            // Apply vibrancy and rounded corners on macOS
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                {
                    apply_vibrancy(&window, NSVisualEffectMaterial::Popover, None, Some(8.0))
                        .expect("Failed to apply vibrancy");
                }

                // Hide window when it loses focus
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = window_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
