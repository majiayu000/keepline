use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

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
        .invoke_handler(tauri::generate_handler![get_quota])
        .setup(|app| {
            // Setup system tray
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .tooltip("Claude Quota Monitor")
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

            // Hide window when it loses focus
            if let Some(window) = app.get_webview_window("main") {
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
