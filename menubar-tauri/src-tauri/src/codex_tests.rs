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
        let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"));
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

use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

fn test_jwt(payload: serde_json::Value) -> String {
    let header = STANDARD_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let body = STANDARD_NO_PAD.encode(payload.to_string());
    format!("{header}.{body}.sig")
}

fn expired_access_token() -> String {
    test_jwt(serde_json::json!({ "exp": Utc::now().timestamp() - 120 }))
}

fn fresh_access_token() -> String {
    test_jwt(serde_json::json!({ "exp": Utc::now().timestamp() + 3600 }))
}

fn account_id_token(account_id: &str) -> String {
    test_jwt(serde_json::json!({
        "exp": Utc::now().timestamp() + 3600,
        "email": "refreshed@example.com",
        "https://api.openai.com/auth": { "chatgpt_account_id": account_id },
    }))
}

fn write_auth_file(
    path: &std::path::Path,
    access_token: &str,
    refresh_token: &str,
    id_token: &str,
) {
    let value = serde_json::json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": "sk-test",
        "tokens": {
            "id_token": id_token,
            "access_token": access_token,
            "refresh_token": refresh_token,
            "account_id": "acc_kept",
        },
        "last_refresh": "2020-01-01T00:00:00Z",
        "extra": { "keep": true },
    });
    fs::write(path, serde_json::to_vec_pretty(&value).expect("auth json")).expect("write auth");
}

fn read_auth_file(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read auth")).expect("parse auth")
}

struct DirModeGuard<'a> {
    path: &'a std::path::Path,
}

impl Drop for DirModeGuard<'_> {
    fn drop(&mut self) {
        let Ok(metadata) = fs::metadata(self.path) else {
            return;
        };
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        let _ = fs::set_permissions(self.path, permissions);
    }
}

#[tokio::test]
async fn persisted_refresh_updates_bundle_and_keeps_mode_0600() {
    let dir = tempfile::tempdir().expect("tempdir");
    let auth_path = dir.path().join("auth.json");
    let new_id = account_id_token("acc_new");
    let issued_id = new_id.clone();
    write_auth_file(&auth_path, &expired_access_token(), "old-refresh", "old-id");

    let refreshed = refresh_persisted_codex_auth(&auth_path, move |_| async move {
        Ok(serde_json::json!({
            "access_token": "new-access",
            "refresh_token": "new-refresh",
            "id_token": issued_id,
        }))
    })
    .await
    .expect("refresh persists");

    assert_eq!(refreshed.access_token, "new-access");
    assert_eq!(refreshed.id_token.as_deref(), Some(new_id.as_str()));
    assert_eq!(
        chatgpt_account_id_from_id_token(refreshed.id_token.as_deref().expect("id token"))
            .as_deref(),
        Some("acc_new")
    );

    let saved = read_auth_file(&auth_path);
    assert_eq!(saved["auth_mode"], "chatgpt");
    assert_eq!(saved["OPENAI_API_KEY"], "sk-test");
    assert_eq!(saved["extra"]["keep"], true);
    assert_eq!(saved["tokens"]["account_id"], "acc_kept");
    assert_eq!(saved["tokens"]["access_token"], "new-access");
    assert_eq!(saved["tokens"]["refresh_token"], "new-refresh");
    assert_eq!(saved["tokens"]["id_token"], new_id);
    let last_refresh = saved["last_refresh"].as_str().expect("last_refresh");
    assert!(last_refresh.ends_with('Z'));
    DateTime::parse_from_rfc3339(last_refresh).expect("chrono accepts last_refresh");
    let mode = fs::metadata(&auth_path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[tokio::test]
async fn omitted_or_empty_rotated_tokens_stay_stored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let omitted_path = dir.path().join("omitted.json");
    write_auth_file(
        &omitted_path,
        &expired_access_token(),
        "old-refresh",
        "old-id",
    );
    refresh_persisted_codex_auth(&omitted_path, |_| async {
        Ok(serde_json::json!({ "access_token": "new-access" }))
    })
    .await
    .expect("omitted fields still persist access token");
    let omitted = read_auth_file(&omitted_path);
    assert_eq!(omitted["tokens"]["access_token"], "new-access");
    assert_eq!(omitted["tokens"]["refresh_token"], "old-refresh");
    assert_eq!(omitted["tokens"]["id_token"], "old-id");

    let empty_path = dir.path().join("empty.json");
    write_auth_file(
        &empty_path,
        &expired_access_token(),
        "old-refresh",
        "old-id",
    );
    refresh_persisted_codex_auth(&empty_path, |_| async {
        Ok(serde_json::json!({
            "access_token": "newer-access",
            "refresh_token": "",
            "id_token": "",
        }))
    })
    .await
    .expect("empty fields still persist access token");
    let empty = read_auth_file(&empty_path);
    assert_eq!(empty["tokens"]["access_token"], "newer-access");
    assert_eq!(empty["tokens"]["refresh_token"], "old-refresh");
    assert_eq!(empty["tokens"]["id_token"], "old-id");
    assert_eq!(empty["tokens"]["account_id"], "acc_kept");
    assert_eq!(empty["OPENAI_API_KEY"], "sk-test");
}

#[test]
fn locked_reread_of_fresh_token_skips_exchange() {
    let dir = tempfile::tempdir().expect("tempdir");
    let auth_path = dir.path().join("auth.json");
    write_auth_file(
        &auth_path,
        &expired_access_token(),
        "refresh-keep",
        "id-keep",
    );
    let fresh = fresh_access_token();
    let lock = ExclusiveFileLock::acquire(&refresh_lock_path(&auth_path)).expect("lock");

    let auth_for_thread = auth_path.clone();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let result = runtime.block_on(refresh_persisted_codex_auth(&auth_for_thread, |_| async {
            panic!("token endpoint must not be called");
        }));
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_millis(250)) {
        Err(mpsc::RecvTimeoutError::Timeout) => {}
        other => panic!("helper did not wait for the refresh lock: {other:?}"),
    }

    write_auth_file(&auth_path, &fresh, "refresh-keep", "id-keep");
    drop(lock);

    let refreshed = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("helper finished")
        .expect("fresh token");
    worker.join().expect("worker");
    assert_eq!(refreshed.access_token, fresh);
    let saved = read_auth_file(&auth_path);
    assert_eq!(saved["tokens"]["refresh_token"], "refresh-keep");
    assert_eq!(saved["last_refresh"], "2020-01-01T00:00:00Z");
}

#[tokio::test]
async fn failed_atomic_write_does_not_return_new_token_or_retry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let _restore = DirModeGuard { path: dir.path() };
    let auth_path = dir.path().join("auth.json");
    let old_access = expired_access_token();
    write_auth_file(&auth_path, &old_access, "old-refresh", "old-id");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in_exchange = Arc::clone(&calls);
    let dir_path = dir.path().to_path_buf();

    let result = refresh_persisted_codex_auth(&auth_path, move |refresh| {
        let calls_in_exchange = Arc::clone(&calls_in_exchange);
        let dir_path = dir_path.clone();
        async move {
            let seen = calls_in_exchange.fetch_add(1, Ordering::SeqCst);
            assert_eq!(seen, 0, "refresh token was posted more than once");
            assert_eq!(refresh, "old-refresh");
            let mut permissions = fs::metadata(&dir_path).expect("dir metadata").permissions();
            permissions.set_mode(0o555);
            fs::set_permissions(&dir_path, permissions).expect("revoke dir write");
            Ok(serde_json::json!({
                "access_token": "new-access",
                "refresh_token": "new-refresh",
                "id_token": "new-id",
            }))
        }
    })
    .await;

    match result {
        Err(CodexAuthRefreshError::Persist(message)) => {
            assert!(!message.contains("new-access"));
            assert!(!message.contains("new-refresh"));
        }
        other => panic!("expected persist error, got {other:?}"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let saved = read_auth_file(&auth_path);
    assert_eq!(saved["tokens"]["access_token"], old_access);
    assert_eq!(saved["tokens"]["refresh_token"], "old-refresh");
    assert_eq!(saved["tokens"]["id_token"], "old-id");
    assert_eq!(saved["last_refresh"], "2020-01-01T00:00:00Z");
}

#[tokio::test]
async fn token_endpoint_failure_leaves_auth_unchanged_and_returns_old_token() {
    let dir = tempfile::tempdir().expect("tempdir");
    let auth_path = dir.path().join("auth.json");
    let old_access = expired_access_token();
    write_auth_file(&auth_path, &old_access, "old-refresh", "old-id");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in_exchange = Arc::clone(&calls);

    let refreshed = refresh_persisted_codex_auth(&auth_path, move |_| {
        let calls_in_exchange = Arc::clone(&calls_in_exchange);
        async move {
            calls_in_exchange.fetch_add(1, Ordering::SeqCst);
            Err("token endpoint unavailable".to_string())
        }
    })
    .await
    .expect("endpoint failure falls back");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(refreshed.access_token, old_access);
    assert_eq!(refreshed.id_token.as_deref(), Some("old-id"));
    let saved = read_auth_file(&auth_path);
    assert_eq!(saved["tokens"]["refresh_token"], "old-refresh");
    assert_eq!(saved["last_refresh"], "2020-01-01T00:00:00Z");
}
