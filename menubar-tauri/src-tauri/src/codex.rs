//! Codex / ChatGPT integration: types, JWT decoding, and Tauri commands for
//! account info, session stats, and rate limits.

use agent_sessions::{
    read_history_from, Agent, HistoryOptions, LineErrorKind, RawReadOptions, Roots, StreamError,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

mod codex_auth_file;
use codex_auth_file::{atomic_replace_private_file, refresh_lock_path, ExclusiveFileLock};

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

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodexStats {
    #[serde(rename = "totalSessions")]
    pub total_sessions: u32,
    #[serde(rename = "todaySessions")]
    pub today_sessions: u32,
    #[serde(rename = "lastActivity")]
    pub last_activity: Option<String>,
    /// Set when the stats could not be computed (e.g. file unreadable).
    /// `None` and zero counts means "no history yet" — not an error.
    pub error: Option<String>,
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

const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_REFRESH_URL: &str = "https://auth.openai.com/oauth/token";

fn get_codex_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

fn codex_stats_error(message: String) -> CodexStats {
    CodexStats {
        total_sessions: 0,
        today_sessions: 0,
        last_activity: None,
        error: Some(message),
    }
}

fn collect_codex_stats<R: BufRead>(reader: R, today: NaiveDate) -> CodexStats {
    let mut total_sessions = 0u32;
    let mut today_sessions = 0u32;
    let mut last_ts: Option<i64> = None;

    let options = HistoryOptions {
        strict_fields: false,
        read: RawReadOptions {
            max_read_bytes: None,
            max_line_bytes: None,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut history = match read_history_from(Agent::Codex, reader, &options) {
        Ok(history) => history,
        Err(error) => {
            return codex_stats_error(format!("Failed to read history.jsonl: {error}"));
        }
    };
    while let Some(entry) = history.next() {
        let entry = match entry {
            Ok(entry) => entry.value,
            Err(StreamError::Line { line_no, kind, .. }) => {
                let operation = if matches!(kind, LineErrorKind::InvalidUtf8) {
                    "read"
                } else {
                    "parse"
                };
                return codex_stats_error(format!(
                    "Failed to {operation} history.jsonl line {line_no}: {kind:?}"
                ));
            }
            Err(StreamError::Io(error)) => {
                return codex_stats_error(format!(
                    "Failed to read history.jsonl line {}: {error}",
                    history.next_line_no()
                ));
            }
            Err(error) => {
                return codex_stats_error(format!("Failed to read history.jsonl: {error}"));
            }
        };

        // Preserve the public field's historic meaning: history entries, not
        // distinct session IDs. Missing or invalid optional fields still count.
        total_sessions += 1;
        if let Some(at) = entry.at {
            if at.date_naive() == today {
                today_sessions += 1;
            }
        }
        if let Some(ts) = entry.timestamp {
            if last_ts.map_or(true, |old| ts > old) {
                last_ts = Some(ts);
            }
        }
    }

    let last_activity = last_ts
        .and_then(|ts| DateTime::from_timestamp(ts, 0))
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string());

    CodexStats {
        total_sessions,
        today_sessions,
        last_activity,
        error: None,
    }
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
        .or_else(|| {
            base64::engine::general_purpose::STANDARD
                .decode(&standard)
                .ok()
        })
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
}

// Treat tokens as expired if they will expire within the next 60 seconds, so
// the request after this check is unlikely to race the boundary.
fn is_token_expired(token: &str) -> bool {
    let payload = match decode_jwt_payload(token) {
        Some(p) => p,
        None => return true,
    };
    let exp = match payload["exp"].as_i64() {
        Some(e) => e,
        None => return true,
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    exp * 1000 < now_ms + 60_000
}

#[derive(Debug)]
struct CodexQuotaAuth {
    access_token: String,
    id_token: Option<String>,
}

#[derive(Debug)]
enum CodexAuthRefreshError {
    Persist(String),
    Unavailable(String),
}

fn non_empty_json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn token_string(auth: &serde_json::Value, key: &str) -> Option<String> {
    auth.get("tokens")
        .and_then(|tokens| non_empty_json_string(tokens, key))
}

fn read_codex_auth(auth_path: &Path) -> Result<serde_json::Value, CodexAuthRefreshError> {
    let content = fs::read_to_string(auth_path).map_err(|error| {
        CodexAuthRefreshError::Unavailable(format!("Failed to read auth.json: {error}"))
    })?;
    let auth_json: serde_json::Value = serde_json::from_str(&content).map_err(|error| {
        CodexAuthRefreshError::Unavailable(format!("Failed to parse auth.json: {error}"))
    })?;
    if auth_json.is_object() {
        Ok(auth_json)
    } else {
        Err(CodexAuthRefreshError::Unavailable(
            "Failed to parse auth.json: expected object".to_string(),
        ))
    }
}

fn current_quota_auth(auth: &serde_json::Value) -> Result<CodexQuotaAuth, CodexAuthRefreshError> {
    let Some(access_token) = token_string(auth, "access_token") else {
        return Err(CodexAuthRefreshError::Unavailable(
            "No access_token found in auth.json".to_string(),
        ));
    };
    Ok(CodexQuotaAuth {
        access_token,
        id_token: token_string(auth, "id_token"),
    })
}

fn store_refresh_response(
    auth: &mut serde_json::Value,
    response: &serde_json::Value,
) -> Result<String, CodexAuthRefreshError> {
    let Some(new_access) = non_empty_json_string(response, "access_token") else {
        return Err(CodexAuthRefreshError::Unavailable(String::new()));
    };
    let tokens = auth
        .get_mut("tokens")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or_else(|| {
            CodexAuthRefreshError::Persist(
                "Failed to persist Codex auth: tokens object missing".to_string(),
            )
        })?;
    tokens.insert(
        "access_token".to_string(),
        serde_json::Value::String(new_access.clone()),
    );
    if let Some(refresh) = non_empty_json_string(response, "refresh_token") {
        tokens.insert(
            "refresh_token".to_string(),
            serde_json::Value::String(refresh),
        );
    }
    if let Some(id_token) = non_empty_json_string(response, "id_token") {
        tokens.insert("id_token".to_string(), serde_json::Value::String(id_token));
    }
    auth.as_object_mut()
        .ok_or_else(|| {
            CodexAuthRefreshError::Persist(
                "Failed to persist Codex auth: expected object".to_string(),
            )
        })?
        .insert(
            "last_refresh".to_string(),
            serde_json::Value::String(Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
        );
    Ok(new_access)
}

// Re-reads auth.json under an exclusive lock. One token POST is attempted only
// when the locked access token is inside the 60-second expiry window.
async fn refresh_persisted_codex_auth<F, Fut>(
    auth_path: &Path,
    exchange: F,
) -> Result<CodexQuotaAuth, CodexAuthRefreshError>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<serde_json::Value, String>>,
{
    let _lock = ExclusiveFileLock::acquire(&refresh_lock_path(auth_path)).map_err(|error| {
        CodexAuthRefreshError::Persist(format!("Failed to lock Codex auth: {error}"))
    })?;
    let mut auth_json = read_codex_auth(auth_path)?;
    let current = current_quota_auth(&auth_json)?;
    if !is_token_expired(&current.access_token) {
        return Ok(current);
    }
    let Some(refresh_token) = token_string(&auth_json, "refresh_token") else {
        return Ok(current);
    };

    let response = match exchange(refresh_token).await {
        Ok(response) => response,
        Err(_) => return Ok(current),
    };
    if non_empty_json_string(&response, "access_token").is_none() {
        return Ok(current);
    }
    let new_access = match store_refresh_response(&mut auth_json, &response) {
        Ok(access) => access,
        Err(CodexAuthRefreshError::Unavailable(_)) => return Ok(current),
        Err(error) => return Err(error),
    };
    let mut bytes = serde_json::to_vec_pretty(&auth_json).map_err(|error| {
        CodexAuthRefreshError::Persist(format!("Failed to persist Codex auth: {error}"))
    })?;
    bytes.push(b'\n');
    atomic_replace_private_file(auth_path, &bytes).map_err(|error| {
        CodexAuthRefreshError::Persist(format!("Failed to persist Codex auth: {error}"))
    })?;
    Ok(CodexQuotaAuth {
        access_token: new_access,
        id_token: token_string(&auth_json, "id_token"),
    })
}

async fn exchange_codex_refresh_token(refresh_token: String) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
        "client_id": CODEX_CLIENT_ID,
    });
    let response = reqwest::Client::new()
        .post(CODEX_REFRESH_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("token endpoint status {}", response.status()));
    }
    response.json().await.map_err(|error| error.to_string())
}

fn chatgpt_account_id_from_id_token(id_token: &str) -> Option<String> {
    decode_jwt_payload(id_token).and_then(|payload| {
        payload
            .get("https://api.openai.com/auth")
            .and_then(|auth| auth.get("chatgpt_account_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    })
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
    let codex_home = match Roots::from_env_for(Agent::Codex) {
        Ok(roots) => match roots.codex {
            Some(path) => path,
            None => {
                return Ok(codex_stats_error(
                    "Could not find home directory".to_string(),
                ))
            }
        },
        Err(error) => {
            return Ok(codex_stats_error(format!(
                "Could not resolve Codex history directory: {error}"
            )));
        }
    };

    let history_file = codex_home.join("history.jsonl");
    if !history_file.exists() {
        // Genuinely no history yet, not an error.
        return Ok(CodexStats {
            total_sessions: 0,
            today_sessions: 0,
            last_activity: None,
            error: None,
        });
    }

    let file = match fs::File::open(&history_file) {
        Ok(f) => f,
        Err(e) => {
            return Ok(CodexStats {
                total_sessions: 0,
                today_sessions: 0,
                last_activity: None,
                error: Some(format!("Failed to read history.jsonl: {}", e)),
            });
        }
    };

    Ok(collect_codex_stats(
        BufReader::new(file),
        Utc::now().date_naive(),
    ))
}

#[tauri::command]
pub async fn open_chatgpt_quota() -> Result<(), String> {
    tauri_plugin_opener::open_url("https://chatgpt.com", None::<&str>).map_err(|e| e.to_string())
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

    // Refresh under the auth lock and use the persisted id_token for the
    // WHAM account header, including one saved by this refresh.
    let quota_auth =
        match refresh_persisted_codex_auth(&auth_file, exchange_codex_refresh_token).await {
            Ok(auth) => auth,
            Err(CodexAuthRefreshError::Persist(message)) => return Err(message),
            Err(CodexAuthRefreshError::Unavailable(message)) => {
                return Ok(CodexRateLimits {
                    connected: false,
                    plan_type: None,
                    primary: None,
                    secondary: None,
                    credits: None,
                    error: Some(message),
                });
            }
        };
    let access_token = quota_auth.access_token;
    let account_id = quota_auth
        .id_token
        .as_deref()
        .and_then(chatgpt_account_id_from_id_token);

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

                        let primary = data["rate_limit"].get("primary_window").and_then(|w| {
                            if w.is_null() {
                                None
                            } else {
                                Some(CodexRateLimitWindow {
                                    used_percent: w["used_percent"].as_f64().unwrap_or(0.0),
                                    window_minutes: w["limit_window_seconds"]
                                        .as_i64()
                                        .map(|s| (s + 59) / 60),
                                    resets_at: w["reset_at"].as_i64(),
                                })
                            }
                        });

                        let secondary = data["rate_limit"].get("secondary_window").and_then(|w| {
                            if w.is_null() {
                                None
                            } else {
                                Some(CodexRateLimitWindow {
                                    used_percent: w["used_percent"].as_f64().unwrap_or(0.0),
                                    window_minutes: w["limit_window_seconds"]
                                        .as_i64()
                                        .map(|s| (s + 59) / 60),
                                    resets_at: w["reset_at"].as_i64(),
                                })
                            }
                        });

                        let credits = data["credits"].as_object().map(|c| CodexCredits {
                            has_credits: c
                                .get("has_credits")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false),
                            unlimited: c
                                .get("unlimited")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false),
                            balance: c
                                .get("balance")
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
