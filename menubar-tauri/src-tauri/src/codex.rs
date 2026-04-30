//! Codex / ChatGPT integration: types, JWT decoding, and Tauri commands for
//! account info, session stats, and rate limits.

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

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

fn get_codex_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

// Decode a JWT payload without verification (used to read ChatGPT account
// metadata from a locally stored token).
fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{}==", payload),
        3 => format!("{}=", payload),
        _ => payload.to_string(),
    };

    let standard = padded.replace('-', "+").replace('_', "/");

    STANDARD_NO_PAD
        .decode(&standard)
        .ok()
        .or_else(|| base64::engine::general_purpose::STANDARD.decode(&standard).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
}

#[tauri::command]
pub async fn get_codex_info() -> Result<CodexData, String> {
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

#[tauri::command]
pub async fn get_codex_stats() -> Result<CodexStats, String> {
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
                    if let Some(dt) = DateTime::from_timestamp(ts, 0) {
                        if dt.date_naive() == today {
                            today_sessions += 1;
                        }
                    }

                    if last_ts.map_or(true, |old| ts > old) {
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

#[tauri::command]
pub async fn open_chatgpt_quota() -> Result<(), String> {
    tauri_plugin_opener::open_url("https://chatgpt.com", None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_codex_rate_limits() -> Result<CodexRateLimits, String> {
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

    let account_id = auth_json["tokens"]["id_token"]
        .as_str()
        .and_then(|token| decode_jwt_payload(token))
        .and_then(|payload| {
            payload["https://api.openai.com/auth"]["chatgpt_account_id"]
                .as_str()
                .map(|s| s.to_string())
        });

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
                        let plan_type = data["plan_type"].as_str().map(|s| s.to_string());

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
