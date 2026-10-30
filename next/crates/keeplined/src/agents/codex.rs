//! Codex: `~/.codex/sessions` rollouts, one `session_meta` header per thread.
use super::{home_dir, temporary, Adapter, Hint, OfficialAnswer, Pending};
use crate::content::{input, question_summary, re, text, Parser};
use keepline_core::content::{short, Source, Turn};
use keepline_core::model::AgentSession;
use keepline_core::protocol::TerminalNote;
use regex::Regex;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

pub struct Codex;
impl Adapter for Codex {
    fn screen_rules(&self) -> &'static str {
        include_str!("screen/codex.json")
    }
    fn root(&self, home: &Path) -> PathBuf {
        home_dir(home, "CODEX_HOME", ".codex").join("sessions")
    }
    fn runs(&self, executable: &str, script: Option<&str>) -> bool {
        executable == "codex" || script.is_some_and(|s| s.ends_with("/@openai/codex/bin/codex.js"))
    }
    fn process_session(&self, _pid: u32, args: &[String]) -> (Option<String>, bool) {
        codex_args(args)
    }
    // A session_meta line may exceed the 16 KiB head budget because of base_instructions.
    // Decode only complete top-level metadata values in its prefix, never scan its body.
    fn header_value(&self, bytes: &[u8]) -> Option<Value> {
        if let Ok(v) = serde_json::from_slice(bytes) {
            return Some(v);
        }
        let s = std::str::from_utf8(bytes).ok()?;
        if !s.contains("\"session_meta\"") {
            return None;
        }
        static KEYS: LazyLock<Regex> = LazyLock::new(|| {
            re(
                r#"(?:^|[,{])\s*"(id|cwd|originator|source|thread_source|parent_thread_id|is_exec)"\s*:\s*"#,
            )
        });
        let mut p = serde_json::Map::new();
        for m in KEYS.captures_iter(s) {
            let mut d = serde_json::Deserializer::from_str(&s[m.get(0)?.end()..]);
            if let Ok(v) = Value::deserialize(&mut d) {
                p.entry(m[1].to_owned()).or_insert(v);
            }
        }
        Some(json!({"type":"session_meta","payload":p}))
    }
    /// A rollout is named `rollout-<time>-<thread id>.jsonl`.
    fn name(&self, root: &Path, transcript: &Path) -> Option<String> {
        let stem = transcript.file_stem()?.to_str()?;
        let id = stem.get(stem.len().checked_sub(36)?..)?;
        state_name(root, id).or_else(|| thread_name(root, id))
    }
    fn identity(&self, _path: &Path, value: &Value) -> Option<(String, String)> {
        if value["type"] != "session_meta" {
            return None;
        }
        Some((
            value["payload"]["id"].as_str()?.into(),
            value["payload"]["cwd"].as_str()?.into(),
        ))
    }
    fn metadata(&self, parser: &mut Parser, v: &Value) {
        let m = &v["payload"];
        // Whatever hides it, a Codex subagent still belongs to its parent.
        if v["type"] == "session_meta" {
            let spawn = &m["source"]["subagent"]["thread_spawn"];
            if let Some(parent) = spawn["parent_thread_id"]
                .as_str()
                .or(m["parent_thread_id"].as_str())
            {
                parser.card.parent = parent.to_owned();
                parser.card.agent_name = m["agent_nickname"]
                    .as_str()
                    .or(spawn["agent_path"].as_str())
                    .unwrap_or("")
                    .to_owned();
            }
        }
        if temporary(m["cwd"].as_str().unwrap_or("")) {
            parser.hide(keepline_core::content::TEMPORARY_DIRECTORY);
        } else if v["type"] == "session_meta" {
            let source = m["source"].as_str().unwrap_or("");
            let origin = m["originator"].as_str().unwrap_or("");
            if m["source"].get("subagent").is_some()
                || matches!(
                    m["thread_source"].as_str(),
                    Some("subagent" | "guardian_review")
                )
            {
                parser.hide("Codex 子代理");
            } else if source == "exec" || origin.contains("exec") || m["is_exec"] == true {
                parser.hide("Codex exec 非交互");
            } else if origin.contains("sdk") || matches!(source, "sdk" | "api") {
                parser.hide("Codex SDK 非交互");
            } else if matches!(source, "cli" | "vscode")
                && matches!(m["thread_source"].as_str(), None | Some("user"))
            {
                parser.card.interactive = true;
            }
        }
        if v["type"] == "queue-operation" {
            parser.machine_prompt(&text(&v["content"]));
        }
    }
    fn is_user(&self, v: &Value) -> bool {
        v["type"] == "event_msg" && v["payload"]["type"] == "user_message"
            || v["type"] == "response_item" && v["payload"]["role"] == "user"
    }
    fn add(&self, parser: &mut Parser, v: &Value, at: u64, source: &Source) {
        let ty = v["type"].as_str().unwrap_or("");
        let p = &v["payload"];
        if ty == "event_msg" && p["type"] != "item_completed" {
            match p["type"].as_str().unwrap_or("") {
                "task_started" | "turn_started" => parser.phase(
                    Turn::Running,
                    p["turn_id"].as_str().unwrap_or(""),
                    at,
                    source,
                ),
                "task_complete" | "task_completed" | "turn_completed" => {
                    if let Some(s) = p["last_agent_message"].as_str() {
                        parser.message(false, s.into(), at, source, true);
                    }
                    parser.phase(Turn::Completed, "", at, source);
                }
                "turn_aborted" => parser.phase(Turn::Interrupted, "", at, source),
                "user_message" => parser.message(true, text(&p["message"]), at, source, false),
                "agent_message" => parser.message(false, text(&p["message"]), at, source, false),
                "approval_request" | "permission_request" | "request_user_input" => {
                    parser.card.waiting = question_summary(p);
                    parser.phase(Turn::Waiting, "", at, source);
                }
                "error" | "turn_failed" => {
                    parser.card.error = short(
                        &text(p.get("message").or_else(|| p.get("error")).unwrap_or(p)),
                        600,
                    );
                    parser.phase(Turn::Error, "", at, source);
                }
                "token_count" | "agent_reasoning" | "context_compacted" => (),
                _ => parser.card.unknown_records += 1,
            }
            return;
        }
        let item = if ty == "response_item" {
            p
        } else {
            v.get("item").or_else(|| p.get("item")).unwrap_or(p)
        };
        match item["type"].as_str().unwrap_or("") {
            "message" => match item["role"].as_str() {
                Some("user") => parser.message(true, text(&item["content"]), at, source, false),
                Some("assistant") => parser.message(
                    false,
                    text(&item["content"]),
                    at,
                    source,
                    item["phase"] == "final_answer" || item["channel"] == "final",
                ),
                _ => (),
            },
            "function_call" | "custom_tool_call" => parser.tool(
                item["call_id"]
                    .as_str()
                    .or_else(|| item["id"].as_str())
                    .unwrap_or(""),
                item["name"].as_str().unwrap_or(""),
                input(item.get("arguments").unwrap_or(&item["input"])),
                at,
                source,
            ),
            "function_call_output" | "custom_tool_call_output" => parser.result(
                item["call_id"].as_str().unwrap_or(""),
                &item["output"],
                item["exit_code"].as_i64().map(|n| n as i32),
                false,
                at,
                source,
            ),
            "command_execution" | "CommandExecution" => {
                let id = item["id"]
                    .as_str()
                    .or_else(|| item["call_id"].as_str())
                    .unwrap_or("");
                if !parser.calls.contains_key(id) {
                    parser.tool(
                        id,
                        "exec_command",
                        json!({"cmd":item["command"]}),
                        at,
                        source,
                    );
                }
                parser.result(
                    id,
                    item.get("output").unwrap_or(&item["aggregated_output"]),
                    item["exit_code"].as_i64().map(|n| n as i32),
                    false,
                    at,
                    source,
                );
            }
            "reasoning" => (),
            _ if matches!(
                ty,
                "session_meta"
                    | "turn_context"
                    | "compacted"
                    | "token_usage_record"
                    | "world_state"
                    | "inter_agent_communication_metadata"
            ) => {}
            _ => parser.card.unknown_records += 1,
        }
    }
    fn links_children(&self) -> bool {
        true
    }
    /// The shared daemon runs the thread; the TUI process is a client of it. A node
    /// wrapper whose own child is Codex is not the one to watch.
    fn pending(
        &self,
        process: &crate::recognition::Process,
        processes: &[crate::recognition::Process],
    ) -> Pending {
        if processes
            .iter()
            .any(|p| p.parent == process.pid && p.runtime == process.runtime)
        {
            return Pending::Wait;
        }
        // `--no-daemon` opens its own log; before the first prompt there is nothing to say.
        if process.no_daemon {
            Pending::Quiet
        } else {
            Pending::Unbound { title_status: true }
        }
    }
    fn title_status(&self, title: &str) -> Option<Hint> {
        let state = codex_state(title);
        let why = "这个 Codex 连着共用的后台，Keepline 只看得到它忙不忙，读不到对话。想在这里看到对话内容，下次用 codex --no-daemon 启动。";
        Some(Hint {
            text: match state {
                Some(state) => format!("仅显示状态：{state}；看对话内容请用 codex --no-daemon"),
                None => "未关联：看对话内容请用 codex --no-daemon".into(),
            },
            note: TerminalNote {
                note: match state {
                    Some(state) => format!("codex · {state} · 看不到对话"),
                    None => "codex · 看不到对话".into(),
                },
                why: why.into(),
            },
        })
    }
    fn outside_pty(&self) -> bool {
        true
    }
    /// A Codex thread id is its session id; `--no-daemon` threads are not loaded there.
    fn official(&self, due: &[&(AgentSession, Option<u32>)]) -> Option<OfficialAnswer> {
        let threads: Vec<String> = due.iter().map(|(s, _)| s.cli_session_id.clone()).collect();
        let result = crate::codex_status::socket()
            .ok_or_else(|| std::io::Error::other("no HOME for the Codex control socket"))
            .and_then(|socket| crate::codex_status::query(&socket, &threads))
            .map(|statuses| {
                statuses
                    .into_iter()
                    .filter_map(|(thread, status)| {
                        Some((
                            due.iter().position(|(s, _)| s.cli_session_id == thread)?,
                            status,
                        ))
                    })
                    .collect()
            });
        Some(("Codex app-server thread/read", result))
    }
}

/// Codex's own title rules (the ones Herdr reads): "Action Required" while blocked,
/// a braille spinner while working, anything else once idle. Status only, never identity.
fn codex_state(title: &str) -> Option<&'static str> {
    if title.trim().is_empty() {
        None
    } else if title.contains("Action Required") {
        Some("需要你处理")
    } else if title.chars().any(|c| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".contains(c)) {
        Some("工作中")
    } else {
        Some("空闲")
    }
}
/// The Codex app's own sidebar reads `threads.name` from `state_<n>.sqlite` beside
/// `sessions/`, which also names threads `session_index.jsonl` never got. One read-only
/// connection to the newest schema is kept; one that stops answering is dropped and found
/// again, and a schema without the column leaves naming to the index.
fn state_name(root: &Path, id: &str) -> Option<String> {
    static STATE: Mutex<Option<(PathBuf, Connection)>> = Mutex::new(None);
    let dir = root.parent()?;
    let mut state = STATE.lock().ok()?;
    if state.as_ref().is_none_or(|(d, _)| d != dir) {
        let newest = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name();
                let n = name
                    .to_str()?
                    .strip_prefix("state_")?
                    .strip_suffix(".sqlite")?;
                Some((n.parse::<u32>().ok()?, e.path()))
            })
            .max()?
            .1;
        let db = Connection::open_with_flags(
            newest,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()?;
        db.busy_timeout(std::time::Duration::from_millis(50)).ok()?;
        *state = Some((dir.to_owned(), db));
    }
    let (_, db) = state.as_ref()?;
    let found = db
        .prepare_cached("SELECT name FROM threads WHERE id=?1")
        .and_then(|mut q| {
            q.query_row([id], |r| r.get::<_, Option<String>>(0))
                .optional()
        });
    match found {
        Ok(name) => name
            .flatten()
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty()),
        Err(_) => {
            *state = None;
            None
        }
    }
}
/// Codex names each thread in `session_index.jsonl` beside `sessions/`, appending a line
/// whenever it is named or renamed, so the last line for a thread wins. The file is read
/// once, then only what was appended since.
fn thread_name(root: &Path, id: &str) -> Option<String> {
    struct Names {
        path: PathBuf,
        read: u64,
        names: HashMap<String, String>,
    }
    static NAMES: Mutex<Option<Names>> = Mutex::new(None);
    let path = root.parent()?.join("session_index.jsonl");
    let len = std::fs::metadata(&path).ok()?.len();
    let mut names = NAMES.lock().ok()?;
    if names
        .as_ref()
        .is_none_or(|n| n.path != path || len < n.read)
    {
        *names = Some(Names {
            path: path.clone(),
            read: 0,
            names: HashMap::new(),
        });
    }
    let n = names.as_mut()?;
    if len > n.read {
        let mut file = File::open(&path).ok()?;
        file.seek(SeekFrom::Start(n.read)).ok()?;
        let mut appended = Vec::new();
        file.take(len - n.read).read_to_end(&mut appended).ok()?;
        // A line still being written waits for its newline.
        let complete = appended
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |i| i + 1);
        for line in appended[..complete].split(|b| *b == b'\n') {
            let Ok(v) = serde_json::from_slice::<Value>(line) else {
                continue;
            };
            if let (Some(id), Some(name)) = (v["id"].as_str(), v["thread_name"].as_str()) {
                n.names.insert(id.to_owned(), name.trim().to_owned());
            }
        }
        n.read += complete as u64;
    }
    n.names.get(id).filter(|s| !s.is_empty()).cloned()
}

// Codex options that consume the next argv element; anything else positional ends option parsing.
const CODEX_VALUE_FLAGS: &[&str] = &[
    "-c",
    "--config",
    "-m",
    "--model",
    "-p",
    "--profile",
    "-s",
    "--sandbox",
    "-a",
    "--ask-for-approval",
    "-C",
    "--cd",
    "-i",
    "--image",
    "--add-dir",
    "--enable",
    "--disable",
    "--remote",
    "--remote-auth-token-env",
    "--local-provider",
];
/// `codex [opts] resume <uuid>` names its thread exactly, with or without the shared daemon.
/// `--last`, thread names, `fork` and prompts are not identity evidence.
pub(crate) fn codex_args(args: &[String]) -> (Option<String>, bool) {
    let no_daemon = args
        .iter()
        .skip(1)
        .take_while(|a| *a != "--")
        .any(|a| a == "--no-daemon");
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if CODEX_VALUE_FLAGS.contains(&arg.as_str()) {
            args.next();
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        if arg != "resume" {
            break;
        }
        let mut found = None;
        while let Some(arg) = args.next() {
            if arg == "--last" {
                return (None, no_daemon);
            }
            if CODEX_VALUE_FLAGS.contains(&arg.as_str()) {
                args.next();
            } else if arg == "--" || !arg.starts_with('-') {
                let id = if arg == "--" { args.next() } else { Some(arg) };
                found = id
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    .map(|id| id.to_string());
                break;
            }
        }
        return (found, no_daemon);
    }
    (None, no_daemon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unbound_codex_says_its_state_plainly() {
        let note = |title: &str| Codex.title_status(title).unwrap().note.note;
        assert_eq!(note("⠋ fixing"), "codex · 工作中 · 看不到对话");
        assert_eq!(note("Action Required"), "codex · 需要你处理 · 看不到对话");
        assert_eq!(note(""), "codex · 看不到对话");
        assert_eq!(
            Codex.title_status("⠋").unwrap().text,
            "仅显示状态：工作中；看对话内容请用 codex --no-daemon"
        );
    }

    #[test]
    fn only_the_innermost_codex_process_waits_for_a_screen() {
        let process = |pid, parent, no_daemon| crate::recognition::Process {
            pid,
            parent,
            runtime: Some(keepline_core::model::Agent::Codex),
            cli_session_id: None,
            no_daemon,
        };
        let wrapper = process(1, 0, false);
        let native = process(2, 1, false);
        let all = [wrapper.clone(), native.clone()];
        assert_eq!(Codex.pending(&wrapper, &all), Pending::Wait);
        assert_eq!(
            Codex.pending(&native, &all),
            Pending::Unbound { title_status: true }
        );
        // `--no-daemon`: unbound, but nothing to say until its first prompt.
        assert_eq!(Codex.pending(&process(3, 0, true), &[]), Pending::Quiet);
    }
    use std::io::Write;

    #[test]
    fn thread_names_follow_appended_renames_and_wait_for_whole_lines() {
        let home =
            std::env::temp_dir().join(format!("keepline-codex-names-{}", std::process::id()));
        std::fs::create_dir_all(home.join("sessions")).unwrap();
        let index = home.join("session_index.jsonl");
        let id = "01a18aad-c882-79a2-9d72-3ee5359f0add";
        let rollout = home.join(format!("sessions/rollout-2026-10-30T10-40-10-{id}.jsonl"));
        std::fs::write(
            &index,
            format!("{{\"id\":\"{id}\",\"thread_name\":\"查找网站\"}}\n{{\"id\":\"other\",\"thread_name\":\"别的\"}}\n"),
        )
        .unwrap();
        let root = home.join("sessions");
        assert_eq!(Codex.name(&root, &rollout).as_deref(), Some("查找网站"));
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&index)
            .unwrap();
        // Half a line is not a rename yet.
        write!(f, "{{\"id\":\"{id}\",\"thread_name\":\"排查").unwrap();
        assert_eq!(Codex.name(&root, &rollout).as_deref(), Some("查找网站"));
        writeln!(f, " mirror 停止运行\"}}").unwrap();
        assert_eq!(
            Codex.name(&root, &rollout).as_deref(),
            Some("排查 mirror 停止运行")
        );
        assert_eq!(Codex.name(&root, &root.join("rollout-x.jsonl")), None);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn the_app_state_names_threads_the_index_missed_and_wins_over_it() {
        let home =
            std::env::temp_dir().join(format!("keepline-codex-state-{}", uuid::Uuid::new_v4()));
        let root = home.join("sessions");
        std::fs::create_dir_all(&root).unwrap();
        let id = "01a18aad-c882-79a2-9d72-3ee5359f0add";
        let other = "019de0c2-0000-7000-8000-000000000000";
        let rollout = |id: &str| root.join(format!("rollout-2026-10-30T10-40-10-{id}.jsonl"));
        std::fs::write(
            home.join("session_index.jsonl"),
            format!("{{\"id\":\"{id}\",\"thread_name\":\"旧名\"}}\n{{\"id\":\"{other}\",\"thread_name\":\"只在索引\"}}\n"),
        )
        .unwrap();
        // An older schema without names, and the newest one with them.
        Connection::open(home.join("state_4.sqlite"))
            .unwrap()
            .execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY)")
            .unwrap();
        let db = Connection::open(home.join("state_5.sqlite")).unwrap();
        db.execute_batch(&format!(
            "CREATE TABLE threads(id TEXT PRIMARY KEY, name TEXT);
             INSERT INTO threads VALUES('{id}',' 查找 Codex 聊天中的网站 '),('{other}',NULL);"
        ))
        .unwrap();
        assert_eq!(
            Codex.name(&root, &rollout(id)).as_deref(),
            Some("查找 Codex 聊天中的网站")
        );
        assert_eq!(
            Codex.name(&root, &rollout(other)).as_deref(),
            Some("只在索引")
        );
        // A schema that lost the column hands naming back to the index.
        db.execute_batch("ALTER TABLE threads DROP COLUMN name")
            .unwrap();
        drop(db);
        assert_eq!(Codex.name(&root, &rollout(id)).as_deref(), Some("旧名"));
        std::fs::remove_dir_all(home).unwrap();
    }
}
