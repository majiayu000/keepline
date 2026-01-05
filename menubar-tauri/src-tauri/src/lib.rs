mod tray_icon;

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
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

// Codex data structures
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodexData {
    pub connected: bool,
    #[serde(rename = "planType")]
    pub plan_type: Option<String>,
    #[serde(rename = "accountId")]
    pub account_id: Option<String>,
    #[serde(rename = "subscriptionUntil")]
    pub subscription_until: Option<String>,
    pub email: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodexStats {
    #[serde(rename = "totalSessions")]
    pub total_sessions: u32,
    #[serde(rename = "todaySessions")]
    pub today_sessions: u32,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
}

// Codex rate limit data (from ChatGPT backend API)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodexRateLimitWindow {
    #[serde(rename = "usedPercent")]
    pub used_percent: f64,
    #[serde(rename = "windowMinutes")]
    pub window_minutes: Option<i64>,
    #[serde(rename = "resetsAt")]
    pub resets_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodexCredits {
    #[serde(rename = "hasCredits")]
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodexRateLimits {
    pub connected: bool,
    #[serde(rename = "planType")]
    pub plan_type: Option<String>,
    pub primary: Option<CodexRateLimitWindow>,
    pub secondary: Option<CodexRateLimitWindow>,
    pub credits: Option<CodexCredits>,
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
async fn set_dock_visibility(app: AppHandle, visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Use Tauri's activation policy API
        use tauri::ActivationPolicy;
        if visible {
            let _ = app.set_activation_policy(ActivationPolicy::Regular);
        } else {
            let _ = app.set_activation_policy(ActivationPolicy::Accessory);
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

// Get Codex config directory path
fn get_codex_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

// Decode JWT payload (without verification)
fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // Decode base64 payload (second part)
    let payload = parts[1];
    // Add padding if needed
    let padded = match payload.len() % 4 {
        2 => format!("{}==", payload),
        3 => format!("{}=", payload),
        _ => payload.to_string(),
    };

    // Replace URL-safe chars
    let standard = padded.replace('-', "+").replace('_', "/");

    STANDARD_NO_PAD
        .decode(&standard)
        .ok()
        .or_else(|| base64::engine::general_purpose::STANDARD.decode(&standard).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
}

// Get Codex subscription info from auth.json
#[tauri::command]
async fn get_codex_info() -> Result<CodexData, String> {
    let codex_home = match get_codex_home() {
        Some(path) => path,
        None => {
            return Ok(CodexData {
                connected: false,
                plan_type: None,
                account_id: None,
                subscription_until: None,
                email: None,
                error: Some("Could not find home directory".to_string()),
            });
        }
    };

    let auth_file = codex_home.join("auth.json");
    if !auth_file.exists() {
        return Ok(CodexData {
            connected: false,
            plan_type: None,
            account_id: None,
            subscription_until: None,
            email: None,
            error: Some("Codex not configured. Please run 'codex' to login.".to_string()),
        });
    }

    // Read and parse auth.json
    let auth_content = match fs::read_to_string(&auth_file) {
        Ok(content) => content,
        Err(e) => {
            return Ok(CodexData {
                connected: false,
                plan_type: None,
                account_id: None,
                subscription_until: None,
                email: None,
                error: Some(format!("Failed to read auth.json: {}", e)),
            });
        }
    };

    let auth_json: serde_json::Value = match serde_json::from_str(&auth_content) {
        Ok(json) => json,
        Err(e) => {
            return Ok(CodexData {
                connected: false,
                plan_type: None,
                account_id: None,
                subscription_until: None,
                email: None,
                error: Some(format!("Failed to parse auth.json: {}", e)),
            });
        }
    };

    // Get id_token from tokens object
    let id_token = match auth_json["tokens"]["id_token"].as_str() {
        Some(token) => token,
        None => {
            return Ok(CodexData {
                connected: false,
                plan_type: None,
                account_id: None,
                subscription_until: None,
                email: None,
                error: Some("No id_token found in auth.json".to_string()),
            });
        }
    };

    // Decode JWT to extract subscription info
    let payload = match decode_jwt_payload(id_token) {
        Some(p) => p,
        None => {
            return Ok(CodexData {
                connected: false,
                plan_type: None,
                account_id: None,
                subscription_until: None,
                email: None,
                error: Some("Failed to decode JWT token".to_string()),
            });
        }
    };

    // Extract info from JWT payload
    let auth_info = &payload["https://api.openai.com/auth"];
    let plan_type = auth_info["chatgpt_plan_type"]
        .as_str()
        .map(|s| s.to_string());
    let account_id = auth_info["chatgpt_account_id"]
        .as_str()
        .map(|s| s.to_string());
    let subscription_until = auth_info["chatgpt_subscription_active_until"]
        .as_str()
        .map(|s| s.to_string());
    let email = payload["email"].as_str().map(|s| s.to_string());

    Ok(CodexData {
        connected: true,
        plan_type,
        account_id,
        subscription_until,
        email,
        error: None,
    })
}

// Get Codex usage statistics from history.jsonl
#[tauri::command]
async fn get_codex_stats() -> Result<CodexStats, String> {
    let codex_home = match get_codex_home() {
        Some(path) => path,
        None => {
            return Ok(CodexStats {
                total_sessions: 0,
                today_sessions: 0,
                last_activity: None,
            });
        }
    };

    let history_file = codex_home.join("history.jsonl");
    if !history_file.exists() {
        return Ok(CodexStats {
            total_sessions: 0,
            today_sessions: 0,
            last_activity: None,
        });
    }

    let file = match fs::File::open(&history_file) {
        Ok(f) => f,
        Err(_) => {
            return Ok(CodexStats {
                total_sessions: 0,
                today_sessions: 0,
                last_activity: None,
            });
        }
    };

    let reader = BufReader::new(file);
    let today = Utc::now().date_naive();
    let mut total_sessions = 0u32;
    let mut today_sessions = 0u32;
    let mut last_ts: Option<i64> = None;

    for line in reader.lines() {
        if let Ok(line_content) = line {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line_content) {
                total_sessions += 1;

                if let Some(ts) = entry["ts"].as_i64() {
                    // Check if this is today
                    if let Some(dt) = DateTime::from_timestamp(ts, 0) {
                        if dt.date_naive() == today {
                            today_sessions += 1;
                        }
                    }

                    // Track latest timestamp
                    if last_ts.is_none() || ts > last_ts.unwrap() {
                        last_ts = Some(ts);
                    }
                }
            }
        }
    }

    let last_activity = last_ts
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string());

    Ok(CodexStats {
        total_sessions,
        today_sessions,
        last_activity,
    })
}

// Open ChatGPT in browser to view quota
#[tauri::command]
async fn open_chatgpt_quota() -> Result<(), String> {
    tauri_plugin_opener::open_url("https://chatgpt.com", None::<&str>)
        .map_err(|e| e.to_string())
}

// Get Codex rate limits from ChatGPT backend API
#[tauri::command]
async fn get_codex_rate_limits() -> Result<CodexRateLimits, String> {
    let codex_home = match get_codex_home() {
        Some(path) => path,
        None => {
            return Ok(CodexRateLimits {
                connected: false,
                plan_type: None,
                primary: None,
                secondary: None,
                credits: None,
                error: Some("Could not find home directory".to_string()),
            });
        }
    };

    let auth_file = codex_home.join("auth.json");
    if !auth_file.exists() {
        return Ok(CodexRateLimits {
            connected: false,
            plan_type: None,
            primary: None,
            secondary: None,
            credits: None,
            error: Some("Codex not configured. Please run 'codex' to login.".to_string()),
        });
    }

    // Read auth.json
    let auth_content = match fs::read_to_string(&auth_file) {
        Ok(content) => content,
        Err(e) => {
            return Ok(CodexRateLimits {
                connected: false,
                plan_type: None,
                primary: None,
                secondary: None,
                credits: None,
                error: Some(format!("Failed to read auth.json: {}", e)),
            });
        }
    };

    let auth_json: serde_json::Value = match serde_json::from_str(&auth_content) {
        Ok(json) => json,
        Err(e) => {
            return Ok(CodexRateLimits {
                connected: false,
                plan_type: None,
                primary: None,
                secondary: None,
                credits: None,
                error: Some(format!("Failed to parse auth.json: {}", e)),
            });
        }
    };

    // Get access_token from tokens object
    let access_token = match auth_json["tokens"]["access_token"].as_str() {
        Some(token) => token,
        None => {
            return Ok(CodexRateLimits {
                connected: false,
                plan_type: None,
                primary: None,
                secondary: None,
                credits: None,
                error: Some("No access_token found in auth.json".to_string()),
            });
        }
    };

    // Try to get account_id from id_token JWT
    let account_id = auth_json["tokens"]["id_token"]
        .as_str()
        .and_then(|token| decode_jwt_payload(token))
        .and_then(|payload| {
            payload["https://api.openai.com/auth"]["chatgpt_account_id"]
                .as_str()
                .map(|s| s.to_string())
        });

    // Call ChatGPT backend API
    let client = reqwest::Client::new();
    let mut request = client
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "codex-cli")
        .timeout(std::time::Duration::from_secs(10));

    if let Some(ref acc_id) = account_id {
        request = request.header("ChatGPT-Account-Id", acc_id);
    }

    match request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(data) => {
                        // Parse the response
                        let plan_type = data["plan_type"].as_str().map(|s| s.to_string());

                        // Parse primary window
                        let primary = data["rate_limit"]
                            .get("primary_window")
                            .and_then(|w| {
                                if w.is_null() {
                                    None
                                } else {
                                    Some(CodexRateLimitWindow {
                                        used_percent: w["used_percent"].as_i64().unwrap_or(0) as f64,
                                        window_minutes: w["limit_window_seconds"]
                                            .as_i64()
                                            .map(|s| (s + 59) / 60),
                                        resets_at: w["reset_at"].as_i64(),
                                    })
                                }
                            });

                        // Parse secondary window
                        let secondary = data["rate_limit"]
                            .get("secondary_window")
                            .and_then(|w| {
                                if w.is_null() {
                                    None
                                } else {
                                    Some(CodexRateLimitWindow {
                                        used_percent: w["used_percent"].as_i64().unwrap_or(0) as f64,
                                        window_minutes: w["limit_window_seconds"]
                                            .as_i64()
                                            .map(|s| (s + 59) / 60),
                                        resets_at: w["reset_at"].as_i64(),
                                    })
                                }
                            });

                        // Parse credits
                        let credits = data["credits"].as_object().map(|c| CodexCredits {
                            has_credits: c.get("has_credits")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false),
                            unlimited: c.get("unlimited")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false),
                            balance: c.get("balance")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        });

                        Ok(CodexRateLimits {
                            connected: true,
                            plan_type,
                            primary,
                            secondary,
                            credits,
                            error: None,
                        })
                    }
                    Err(e) => Ok(CodexRateLimits {
                        connected: false,
                        plan_type: None,
                        primary: None,
                        secondary: None,
                        credits: None,
                        error: Some(format!("Failed to parse response: {}", e)),
                    }),
                }
            } else if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
                Ok(CodexRateLimits {
                    connected: false,
                    plan_type: None,
                    primary: None,
                    secondary: None,
                    credits: None,
                    error: Some("Token expired. Please run 'codex' to re-login.".to_string()),
                })
            } else {
                Ok(CodexRateLimits {
                    connected: false,
                    plan_type: None,
                    primary: None,
                    secondary: None,
                    credits: None,
                    error: Some(format!("API error: {}", response.status())),
                })
            }
        }
        Err(e) => Ok(CodexRateLimits {
            connected: false,
            plan_type: None,
            primary: None,
            secondary: None,
            credits: None,
            error: Some(format!("Network error: {}", e)),
        }),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(TrayState {
            tray_id: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![get_quota, update_tray_icon, resize_window, set_dock_visibility, get_codex_info, get_codex_stats, open_chatgpt_quota, get_codex_rate_limits])
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
