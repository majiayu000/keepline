//! Grok Build: `~/.grok/sessions/<url-encoded cwd>/<id>/updates.jsonl`, an ACP
//! `session/update` stream. The id and directory are in the path and the envelope, and
//! `summary.json` beside it says what started the session. The TUI cannot be given an id;
//! `active_sessions.json` is Grok's own registry of which process runs which session.
//! Tool approvals are not in the stream, so they come only from the screen.
use super::{home_dir, temporary, Adapter};
use crate::content::{text, Parser};
use keepline_core::content::{short, Source, Turn};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub struct Grok;

fn grok_home(home: &Path) -> PathBuf {
    home_dir(home, "GROK_HOME", ".grok")
}

/// Directory names under the root are absolute working directories, URL-encoded.
fn encoded_cwd(name: &str) -> bool {
    name.starts_with("%2F")
}

fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn session_cwd(transcript: &Path) -> Option<String> {
    decode(transcript.parent()?.parent()?.file_name()?.to_str()?)
}

/// When the kernel started `pid`, in microseconds, as Grok records it.
fn started(pid: u32) -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    // SAFETY: the kernel writes at most `size` bytes into the zeroed buffer.
    let n = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if n != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
}

/// The session the registry names for one live process. Entries outlive crashed
/// processes, so a recorded start time must match; two claims for one process name none.
fn registered(registry: &Path, pid: u32, start: Option<u64>) -> Option<String> {
    let bytes = std::fs::read(registry).ok()?;
    let entries: Vec<Value> = serde_json::from_slice(&bytes).ok()?;
    let mut found = entries.iter().filter(|e| {
        e["pid"].as_u64() == Some(u64::from(pid))
            && e["pid_start"]
                .as_u64()
                .is_none_or(|recorded| Some(recorded) == start)
    });
    let id = found.next()?["session_id"].as_str()?;
    (found.next().is_none() && !id.is_empty()).then(|| id.to_owned())
}

/// The card's names for the tools whose results it reads; the rest keep Grok's names.
fn canonical(name: &str) -> &str {
    match name {
        "run_terminal_command" => "Bash",
        "search_replace" => "Edit",
        "write" => "Write",
        "ask_user_question" => "AskUserQuestion",
        _ => name,
    }
}

/// Prompts Grok sends itself, such as a finished background workflow's reminder.
fn hidden(update: &Value) -> bool {
    update["_meta"]["hideFromScrollback"] == true
}

/// A finished reply segment: before a tool call, or at the end of the turn.
fn flush(parser: &mut Parser, final_answer: bool, at: u64, source: &Source) {
    let reply = std::mem::take(&mut parser.reply);
    if !reply.trim().is_empty() {
        parser.message(false, reply, at, source, final_answer);
    }
}

/// A finished tool call as the card's result reader expects it.
fn outcome(update: &Value) -> (Value, Option<i32>) {
    let raw = &update["rawOutput"];
    let shown: Vec<&str> = update["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b["content"]["text"].as_str())
                .collect()
        })
        .unwrap_or_default();
    let output = raw["output_for_prompt"]
        .as_str()
        .or_else(|| raw["message"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if shown.is_empty() {
                short(&raw.to_string(), 8192)
            } else {
                shown.join("\n")
            }
        });
    let code = if update["status"] == "failed" {
        Some(1)
    } else if raw["type"] == "Bash" {
        raw["exit_code"].as_i64().map(|n| n as i32)
    } else {
        None
    };
    let mut value = Map::new();
    value.insert("output".into(), output.into());
    if raw["is_error"] == true {
        value.insert("is_error".into(), true.into());
    }
    // Started, not finished: no receipt until its output is read back.
    if raw["type"] == "BackgroundTaskStarted" {
        value.insert("backgroundTaskId".into(), raw["task_id"].clone());
    }
    (Value::Object(value), code)
}

impl Adapter for Grok {
    fn root(&self, home: &Path) -> PathBuf {
        grok_home(home).join("sessions")
    }
    fn runs(&self, executable: &str, _script: Option<&str>) -> bool {
        executable == "grok"
    }
    fn process_session(&self, pid: u32, _args: &[String]) -> (Option<String>, bool) {
        let Some(home) = std::env::var_os("HOME") else {
            return (None, false);
        };
        let registry = grok_home(Path::new(&home)).join("active_sessions.json");
        (registered(&registry, pid, started(pid)), false)
    }
    fn registry(&self, root: &Path) -> Option<PathBuf> {
        Some(root.parent()?.join("active_sessions.json"))
    }
    fn is_transcript(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|n| n == "updates.jsonl")
            && path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .is_some_and(encoded_cwd)
    }
    /// Only working directories and the sessions in them; never images, workflows or
    /// terminal captures inside a session.
    fn descend(&self, dir: &Path) -> bool {
        let named = |p: Option<&Path>| {
            p.and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .is_some_and(encoded_cwd)
        };
        named(Some(dir)) || named(dir.parent())
    }
    fn identity(&self, path: &Path, record: &Value) -> Option<(String, String)> {
        Some((
            record["params"]["sessionId"].as_str()?.into(),
            session_cwd(path)?,
        ))
    }
    fn observe_file(&self, parser: &mut Parser, path: &Path) {
        if session_cwd(path).is_some_and(|cwd| temporary(&cwd)) {
            parser.hide(keepline_core::content::TEMPORARY_DIRECTORY);
            return;
        }
        let summary = path.with_file_name("summary.json");
        if std::fs::metadata(&summary).map_or(true, |m| m.len() > 1024 * 1024) {
            return;
        }
        let Some(summary) = std::fs::read(&summary)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        else {
            return;
        };
        match summary["session_kind"].as_str() {
            Some("subagent" | "subagent_resume") => parser.hide("Grok 子代理"),
            Some("headless") => parser.hide("Grok 非交互 / headless"),
            // Kinds not seen yet stay unproven rather than shown.
            Some(_) => (),
            None => parser.card.interactive = true,
        }
    }
    fn metadata(&self, parser: &mut Parser, record: &Value) {
        let update = &record["params"]["update"];
        if update["sessionUpdate"] == "user_message_chunk" && !hidden(update) {
            parser.machine_prompt(&text(&update["content"]["text"]));
        }
    }
    fn is_user(&self, record: &Value) -> bool {
        let update = &record["params"]["update"];
        update["sessionUpdate"] == "user_message_chunk" && !hidden(update)
    }
    fn add(&self, parser: &mut Parser, record: &Value, at: u64, source: &Source) {
        let u = &record["params"]["update"];
        match u["sessionUpdate"].as_str().unwrap_or("") {
            // No turn-start record: a prompt, shown or Grok's own, starts one.
            "user_message_chunk" => {
                if parser.card.turn != Turn::Running {
                    flush(parser, false, at, source);
                    parser.phase(Turn::Running, "", at, source);
                }
                if !hidden(u) && u["content"]["type"] == "text" {
                    parser.message(true, text(&u["content"]["text"]), at, source, false);
                }
            }
            "agent_message_chunk" if u["content"]["type"] == "text" => {
                parser
                    .reply
                    .push_str(u["content"]["text"].as_str().unwrap_or(""));
                parser.card.updated = parser.card.updated.max(at);
            }
            "tool_call" => {
                flush(parser, false, at, source);
                let name = u["_meta"]["x.ai/tool"]["name"]
                    .as_str()
                    .or_else(|| u["title"].as_str())
                    .unwrap_or("");
                parser.tool(
                    u["toolCallId"].as_str().unwrap_or(""),
                    canonical(name),
                    u["rawInput"].clone(),
                    at,
                    source,
                );
            }
            "tool_call_update" if matches!(u["status"].as_str(), Some("completed" | "failed")) => {
                let (value, code) = outcome(u);
                parser.result(
                    u["toolCallId"].as_str().unwrap_or(""),
                    &value,
                    code,
                    true,
                    at,
                    source,
                );
            }
            "turn_completed" => {
                flush(parser, true, at, source);
                match u["stop_reason"].as_str() {
                    Some("cancelled") => parser.phase(Turn::Interrupted, "", at, source),
                    Some("error") => {
                        parser.card.error = short(&text(&u["agent_result"]), 600);
                        parser.phase(Turn::Error, "", at, source);
                    }
                    _ => parser.phase(Turn::Completed, "", at, source),
                }
            }
            // The rest is for Grok's own UI: thoughts, hooks, plans, recaps, compaction.
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::model::Agent;
    use serde_json::json;

    fn line(update: Value) -> Vec<u8> {
        serde_json::to_vec(
            &json!({"timestamp": 1_800_000_000u64, "method": "session/update",
            "params": {"sessionId": "01a1-grok", "update": update}}),
        )
        .unwrap()
    }
    fn feed(p: &mut Parser, updates: Vec<Value>) {
        for (i, u) in updates.into_iter().enumerate() {
            p.add(
                Agent::Grok,
                &line(u),
                Source {
                    path: "/g/%2Frepo/01a1-grok/updates.jsonl".into(),
                    inode: 1,
                    offset: i as u64 * 100,
                },
            );
        }
    }
    fn chunk(kind: &str, text: &str) -> Value {
        json!({"sessionUpdate": kind, "content": {"type": "text", "text": text}})
    }
    fn call(id: &str, tool: &str, input: Value) -> Value {
        json!({"sessionUpdate": "tool_call", "toolCallId": id, "title": tool, "rawInput": input,
            "_meta": {"x.ai/tool": {"name": tool}}})
    }
    fn done(id: &str, raw: Value) -> Value {
        json!({"sessionUpdate": "tool_call_update", "toolCallId": id, "status": "completed",
            "rawOutput": raw})
    }
    fn turn(stop: &str) -> Value {
        json!({"sessionUpdate": "turn_completed", "stop_reason": stop,
            "agent_result": "API error (status 402 Payment Required): balance exhausted"})
    }

    #[test]
    fn a_turn_reads_its_prompt_commands_edits_and_streamed_reply() {
        let mut p = Parser::default();
        feed(
            &mut p,
            vec![
                chunk("user_message_chunk", "修好登录页的测试"),
                chunk("agent_thought_chunk", "thinking"),
                chunk("agent_message_chunk", "先跑一下"),
                chunk("agent_message_chunk", "测试。"),
                call(
                    "c1",
                    "run_terminal_command",
                    json!({"command": "cargo test"}),
                ),
                json!({"sessionUpdate": "tool_call_update", "toolCallId": "c1",
                    "title": "Execute `cargo test`"}),
                done(
                    "c1",
                    json!({"type": "Bash", "exit_code": 101,
                        "output_for_prompt": "exit: 101\ntest result: FAILED. 3 passed; 1 failed"}),
                ),
                call(
                    "c2",
                    "search_replace",
                    json!({"file_path": "/repo/src/login.rs", "old_string": "a", "new_string": "b"}),
                ),
                done("c2", json!({"type": "SearchReplace", "EditsApplied": {}})),
                call(
                    "c3",
                    "run_terminal_command",
                    json!({"command": "cargo test"}),
                ),
                done(
                    "c3",
                    json!({"type": "Bash", "exit_code": 0,
                        "output_for_prompt": "exit: 0\ntest result: ok. 4 passed; 0 failed"}),
                ),
                chunk("agent_message_chunk", "已修复，测试全部通过。"),
                turn("end_turn"),
            ],
        );
        let c = &p.card;
        assert_eq!(c.title, "修好登录页的测试");
        assert!(c.first_ask_known);
        assert_eq!(c.turn, Turn::Completed);
        assert_eq!(c.last_reply, "已修复，测试全部通过。");
        assert!(c.completed_claim);
        let facts: Vec<_> = c
            .evidence
            .iter()
            .map(|e| (e.kind.as_str(), e.exit_code))
            .collect();
        assert!(facts.contains(&("error", Some(101))), "{facts:?}");
        assert!(facts.contains(&("test", Some(1))), "{facts:?}");
        assert!(facts.contains(&("file", Some(0))), "{facts:?}");
        assert!(facts.contains(&("test", Some(0))), "{facts:?}");
        // The reply before the first command was shown, not lost.
        assert!(p.records.iter().any(|r| r.text == "先跑一下测试。"));
    }

    #[test]
    fn questions_wait_and_answers_resume_and_ends_are_told_apart() {
        let mut p = Parser::default();
        feed(
            &mut p,
            vec![
                chunk("user_message_chunk", "写一条发布说明"),
                call(
                    "q",
                    "ask_user_question",
                    json!({"questions": [{"question": "发到哪里？",
                        "options": [{"label": "Threads"}, {"label": "X"}]}]}),
                ),
            ],
        );
        assert_eq!(p.card.turn, Turn::Waiting);
        assert!(p.card.waiting.contains("发到哪里"), "{}", p.card.waiting);
        feed(
            &mut p,
            vec![done(
                "q",
                json!({"type": "AskUserQuestion", "UserAnswered": {}}),
            )],
        );
        assert_eq!(p.card.turn, Turn::Running);
        feed(&mut p, vec![turn("cancelled")]);
        assert_eq!(p.card.turn, Turn::Interrupted);
        // Grok's own wake-up prompt runs a turn but is not something the user asked.
        let asks = p.card.asks.len();
        feed(
            &mut p,
            vec![json!({"sessionUpdate": "user_message_chunk",
                    "content": {"type": "text", "text": "A background workflow stopped."},
                    "_meta": {"hideFromScrollback": true}})],
        );
        assert_eq!(p.card.turn, Turn::Running);
        assert_eq!(p.card.asks.len(), asks);
        feed(&mut p, vec![turn("error")]);
        assert_eq!(p.card.turn, Turn::Error);
        assert!(p.card.error.contains("402"), "{}", p.card.error);
    }

    #[test]
    fn failures_and_background_starts_are_not_receipts_of_success() {
        let mut p = Parser::default();
        feed(
            &mut p,
            vec![
                chunk("user_message_chunk", "部署预览"),
                call(
                    "w",
                    "write",
                    json!({"file_path": "/repo/a.md", "content": "x"}),
                ),
                json!({"sessionUpdate": "tool_call_update", "toolCallId": "w", "status": "failed",
                    "rawOutput": {"error": "tool_execution_failed", "message": "denied"}}),
                call(
                    "b",
                    "run_terminal_command",
                    json!({"command": "npm run dev"}),
                ),
                done(
                    "b",
                    json!({"type": "BackgroundTaskStarted", "task_id": "t1", "status": "running"}),
                ),
            ],
        );
        let facts: Vec<_> = p
            .card
            .evidence
            .iter()
            .map(|e| (e.kind.as_str(), e.exit_code))
            .collect();
        assert!(facts.contains(&("error", Some(1))), "{facts:?}");
        assert!(!facts.contains(&("file", Some(0))), "{facts:?}");
        assert!(facts.contains(&("command", None)), "{facts:?}");
    }

    #[test]
    fn identity_comes_from_the_envelope_and_the_encoded_directory() {
        let path = Path::new(
            "/h/.grok/sessions/%2FUsers%2Fme%2F%E4%BB%A3%E7%A0%81/01a1-grok/updates.jsonl",
        );
        assert!(Grok.is_transcript(path));
        assert_eq!(
            crate::agents::identity(Agent::Grok, path, &line(json!({"sessionUpdate": "plan"}))),
            Some(("01a1-grok".into(), "/Users/me/代码".into()))
        );
        for other in [
            "/h/.grok/sessions/%2Frepo/01a1/chat_history.jsonl",
            "/h/.grok/sessions/%2Frepo/01a1/workflows/wf/updates.jsonl",
        ] {
            assert!(!Grok.is_transcript(Path::new(other)), "{other}");
        }
        assert!(Grok.descend(Path::new("/h/.grok/sessions/%2Frepo")));
        assert!(Grok.descend(Path::new("/h/.grok/sessions/%2Frepo/01a1")));
        assert!(!Grok.descend(Path::new("/h/.grok/sessions/%2Frepo/01a1/images")));
        assert_eq!(decode("%2Fa%zz"), None);
    }

    #[test]
    fn the_registry_names_a_session_only_for_the_process_that_registered_it() {
        let dir = std::env::temp_dir().join(format!("kl-grok-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let registry = dir.join("active_sessions.json");
        std::fs::write(
            &registry,
            json!([
                {"session_id": "old", "pid": 10, "pid_start": 1, "cwd": "/a"},
                {"session_id": "new", "pid": 10, "pid_start": 2, "cwd": "/a"},
                {"session_id": "legacy", "pid": 20, "cwd": "/b"},
                {"session_id": "x", "pid": 30, "cwd": "/c"},
                {"session_id": "y", "pid": 30, "cwd": "/c"},
            ])
            .to_string(),
        )
        .unwrap();
        assert_eq!(registered(&registry, 10, Some(2)).as_deref(), Some("new"));
        assert_eq!(registered(&registry, 10, Some(3)), None);
        assert_eq!(registered(&registry, 10, None), None);
        assert_eq!(
            registered(&registry, 20, Some(9)).as_deref(),
            Some("legacy")
        );
        assert_eq!(registered(&registry, 30, Some(9)), None);
        assert_eq!(registered(&dir.join("missing.json"), 10, Some(2)), None);
        // This test process is live: the kernel's start time is what Grok records.
        assert!(started(std::process::id()).is_some_and(|t| t > 1_600_000_000_000_000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_summary_beside_the_stream_says_who_started_the_session() {
        let dir = std::env::temp_dir().join(format!("kl-grok-{}", uuid::Uuid::new_v4()));
        let session = dir.join("%2FUsers%2Fme%2Frepo").join("01a1");
        std::fs::create_dir_all(&session).unwrap();
        let path = session.join("updates.jsonl");
        let mut p = Parser::default();
        Grok.observe_file(&mut p, &path);
        assert!(!p.card.interactive && p.card.hidden_reason.is_empty());
        for (summary, interactive, hidden) in [
            (json!({"info": {"id": "01a1"}}), true, ""),
            (
                json!({"session_kind": "headless"}),
                false,
                "Grok 非交互 / headless",
            ),
            (json!({"session_kind": "subagent"}), false, "Grok 子代理"),
        ] {
            std::fs::write(session.join("summary.json"), summary.to_string()).unwrap();
            let mut p = Parser::default();
            Grok.observe_file(&mut p, &path);
            assert_eq!(
                (p.card.interactive, p.card.hidden_reason.as_str()),
                (interactive, hidden)
            );
        }
        let mut p = Parser::default();
        Grok.observe_file(
            &mut p,
            Path::new("/g/%2Fprivate%2Ftmp%2Fx/01a1/updates.jsonl"),
        );
        assert_eq!(
            p.card.hidden_reason,
            keepline_core::content::TEMPORARY_DIRECTORY
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
