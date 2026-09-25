//! Codex / ChatGPT integration: types, JWT decoding, and Tauri commands for
//! account info, session stats, and rate limits.

use agent_sessions::{
    read_history_from, Agent, HistoryOptions, LineErrorKind, RawReadOptions, Roots, StreamError,
};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chrono::{DateTime, NaiveDate, Utc};
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn ts(value: &str) -> i64 {
        match DateTime::parse_from_rfc3339(value) {
            Ok(parsed) => parsed.timestamp(),
            Err(error) => panic!("invalid fixed test timestamp: {}", error),
        }
    }

    fn test_date() -> NaiveDate {
        match NaiveDate::from_ymd_opt(2026, 4, 13) {
            Some(date) => date,
            None => panic!("invalid fixed test date"),
        }
    }

    #[test]
    fn collect_codex_stats_counts_valid_history() {
        let today = test_date();
        let history = format!(
            "{{\"ts\":{}}}\n{{\"ts\":{}}}\n",
            ts("2026-04-13T10:00:00Z"),
            ts("2026-04-12T10:00:00Z")
        );

        let stats = collect_codex_stats(Cursor::new(history), today);

        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.today_sessions, 1);
        assert!(stats.last_activity.is_some());
        assert_eq!(stats.error, None);
    }

    #[test]
    fn collect_codex_stats_surfaces_malformed_history_line() {
        let today = test_date();
        let history = format!("{{\"ts\":{}}}\nnot-json\n", ts("2026-04-13T10:00:00Z"));

        let stats = collect_codex_stats(Cursor::new(history), today);

        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.today_sessions, 0);
        assert!(stats.last_activity.is_none());
        assert!(stats
            .error
            .as_deref()
            .is_some_and(|message| message.contains("Failed to parse history.jsonl line 2")));
    }

    #[test]
    fn collect_codex_stats_keeps_lenient_record_count_and_utc_dates() {
        let history = concat!(
            "\n\u{2003}\r\n",
            "{\"session_id\":\"same\",\"ts\":1776038400}\r\n",
            "{\"session_id\":\"same\",\"ts\":1776038460}\n",
            "{\"ts\":1776124800}\n",
            "{\"ts\":\"1776038400\"}\n",
            "{\"ts\":null}\n",
            "{\"ts\":1.5}\n",
            "{\"ts\":-1,\"text\":42}\n",
            "{}\nnull\n[]\ntrue\n17\n\"text\""
        );
        let stats = collect_codex_stats(Cursor::new(history), test_date());
        assert_eq!(stats.total_sessions, 13);
        assert_eq!(stats.today_sessions, 2);
        assert_eq!(stats.last_activity.as_deref(), Some("2026-04-14 00:00"));
        assert_eq!(stats.error, None);
    }

    #[test]
    fn collect_codex_stats_preserves_out_of_range_latest_timestamp() {
        let history = "{\"ts\":1776038400}\n{\"ts\":9223372036854775807}\n";
        let stats = collect_codex_stats(Cursor::new(history), test_date());
        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.today_sessions, 1);
        assert_eq!(stats.last_activity, None);
        assert_eq!(stats.error, None);
    }

    #[test]
    fn collect_codex_stats_rejects_incomplete_tail() {
        let stats = collect_codex_stats(Cursor::new("{}\n\n{\"ts\":"), test_date());
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.today_sessions, 0);
        assert_eq!(stats.last_activity, None);
        assert!(stats
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("Failed to parse history.jsonl line 3:")));
    }

    #[test]
    fn collect_codex_stats_reports_invalid_utf8_as_read_failure() {
        let stats = collect_codex_stats(Cursor::new(b"{}\n\xff\n"), test_date());
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.today_sessions, 0);
        assert_eq!(stats.last_activity, None);
        assert!(stats
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("Failed to read history.jsonl line 2:")));
    }

    #[test]
    fn collect_codex_stats_reports_io_failure_after_blank_lines() {
        struct FailingReader;
        impl std::io::Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("synthetic read failure"))
            }
        }
        impl BufRead for FailingReader {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                Err(std::io::Error::other("synthetic read failure"))
            }
            fn consume(&mut self, _: usize) {}
        }
        let reader = std::io::Read::chain(Cursor::new("{}\n\n"), FailingReader);
        let stats = collect_codex_stats(reader, test_date());
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.today_sessions, 0);
        assert_eq!(stats.last_activity, None);
        assert_eq!(
            stats.error.as_deref(),
            Some("Failed to read history.jsonl line 3: synthetic read failure")
        );
    }

    #[test]
    fn codex_stats_environment_child() {
        let Ok(scenario) = std::env::var("KEEPLINE_HISTORY_TEST_SCENARIO") else {
            return;
        };
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let stats = runtime
            .block_on(get_codex_stats())
            .expect("stats command result");
        match scenario.as_str() {
            "override" => {
                assert_eq!(stats.total_sessions, 2);
                assert_eq!(stats.error, None);
            }
            "fallback" => {
                assert_eq!(stats.total_sessions, 1);
                assert_eq!(stats.error, None);
            }
            "missing" => {
                assert_eq!(stats.total_sessions, 0);
                assert_eq!(stats.error, None);
            }
            "empty" => {
                assert_eq!(stats.total_sessions, 0);
                assert!(stats
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("CODEX_HOME is empty")));
            }
            _ => panic!("unknown test scenario"),
        }
    }

    #[test]
    fn codex_stats_respects_codex_home_in_isolated_processes() {
        let home = tempfile::tempdir().expect("isolated home");
        let default_root = home.path().join(".codex");
        let custom_root = home.path().join("custom-codex");
        fs::create_dir_all(&default_root).expect("default root");
        fs::create_dir_all(&custom_root).expect("custom root");
        fs::write(default_root.join("history.jsonl"), "{}\n").expect("default history");
        fs::write(custom_root.join("history.jsonl"), "{}\n{}\n").expect("custom history");
        for scenario in ["override", "fallback", "missing", "empty"] {
            let mut child =
                std::process::Command::new(std::env::current_exe().expect("test binary"));
            child
                .args([
                    "--exact",
                    "codex::tests::codex_stats_environment_child",
                    "--nocapture",
                ])
                .env("HOME", home.path())
                .env("CLAUDE_CONFIG_DIR", "")
                .env("KEEPLINE_HISTORY_TEST_SCENARIO", scenario);
            match scenario {
                "override" => {
                    child.env("CODEX_HOME", &custom_root);
                }
                "fallback" => {
                    child.env_remove("CODEX_HOME");
                }
                "missing" => {
                    child.env("CODEX_HOME", home.path().join("absent"));
                }
                "empty" => {
                    child.env("CODEX_HOME", "");
                }
                _ => unreachable!(),
            }
            let output = child.output().expect("isolated test process");
            assert!(
                output.status.success(),
                "{scenario}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

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

async fn refresh_codex_access_token(refresh_token: &str) -> Option<String> {
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
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().await.ok()?;
    json["access_token"].as_str().map(|s| s.to_string())
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

    let mut access_token = match auth_json["tokens"]["access_token"].as_str() {
        Some(token) => token.to_string(),
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

    // Mirror src/web/api/routes/usage.ts: refresh expired access tokens before
    // calling WHAM so users don't get a spurious "disconnected" between logins.
    if is_token_expired(&access_token) {
        if let Some(refresh_token) = auth_json["tokens"]["refresh_token"].as_str() {
            if let Some(new_token) = refresh_codex_access_token(refresh_token).await {
                access_token = new_token;
            }
        }
    }

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
