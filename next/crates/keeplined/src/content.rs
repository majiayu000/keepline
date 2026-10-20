//! Record-local normalization, sourced receipts and durable pending tool correlation.
use keepline_core::{
    content::{Content, Evidence, Quote, Source, Turn, short},
    model::Agent,
};
use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, sync::LazyLock};
fn re(s: &str) -> Regex {
    Regex::new(s).unwrap()
}
static EXIT: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?i)(?:Process exited with code|exit[_ ]code[^0-9-]{0,8}|Exit code:)\s*(-?\d+)")
});
static SESSION: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)session ID\s+(\d+)"));
static TEST: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(?im)^.*(?:test result:|Tests?:|\b\d+\s+(?:passed|failed|pass|fail)\b).*$")
});
static FAILED: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\b([1-9]\d*)\s+(?:failed|fail)\b"));
static COMMIT: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?m)^\[[^\]\n]+\s+([a-f0-9]{7,40})\][^\n]*"));
static PATCH: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?m)^\*\*\* (?:Update|Add|Delete) File: (.+)$"));
static CLAIM: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)(?:已(?:完成|修复|实现|提交)|修复完成|全部.{0,12}通过|完成了|implemented|fixed|completed|all tests pass|\bdone\b)",
    )
});
fn claims_done(s: &str) -> bool {
    let mut fenced = false;
    s.lines()
        .filter(|line| {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                false
            } else {
                !fenced && !line.trim_start().starts_with('>')
            }
        })
        .any(|line| {
            ![
                "未完成",
                "尚未",
                "没有完成",
                "还没",
                "not completed",
                "not done",
            ]
            .iter()
            .any(|no| line.contains(no))
                && CLAIM.is_match(line)
        })
}
static INJECTED: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?s)<(?:environment_context|system-reminder|instructions|INSTRUCTIONS|skills_instructions|permissions instructions|collaboration_mode)[^>]*>.*?</(?:environment_context|system-reminder|instructions|INSTRUCTIONS|skills_instructions|permissions instructions|collaboration_mode)>",
    )
});
static CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?s)^\s*(?:# AGENTS\.md instructions for|# Global Codex Working Agreements|This session is being continued from a previous conversation|<turn_aborted>|<task-notification>|<local-command-caveat>|<command-name>|<external_codex_apps_|<cross-session-message|<teammate-message|<environment_context>|<permissions instructions>|<skills_instructions>)",
    )
});
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(|b| b.as_str().or_else(|| b.get("text").and_then(Value::as_str)))
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Null => String::new(),
        _ => v.to_string(),
    }
}
fn input(v: &Value) -> Value {
    v.as_str()
        .map(|s| serde_json::from_str(s).unwrap_or_else(|_| json!({"input":s})))
        .unwrap_or_else(|| v.clone())
}
fn seconds(v: &Value) -> u64 {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp().max(0) as u64)
        .or_else(|| v.as_u64())
        .unwrap_or(0)
}
fn db_err(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Call {
    name: String,
    input: Value,
    source: Source,
    at: u64,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Parser {
    pub card: Content,
    turn_id: String,
    last_user: String,
    last_user_turn: String,
    calls: BTreeMap<String, Call>,
    executions: BTreeMap<String, String>,
    last_failed: String,
    #[serde(skip)]
    pub records: Vec<Record>,
}
#[derive(Clone, Debug)]
pub struct Record {
    pub source: Source,
    pub at: u64,
    pub kind: String,
    pub text: String,
    pub call_id: String,
    pub code: Option<i32>,
}
impl Parser {
    fn record(
        &mut self,
        kind: &str,
        text: String,
        call_id: &str,
        code: Option<i32>,
        at: u64,
        source: &Source,
    ) {
        self.records.push(Record {
            source: source.clone(),
            at,
            kind: kind.into(),
            text,
            call_id: call_id.into(),
            code,
        });
    }
    fn message(&mut self, user: bool, s: String, at: u64, source: &Source, final_answer: bool) {
        let s = if user {
            if CONTEXT.is_match(&s) {
                return;
            }
            INJECTED.replace_all(&s, "").trim().to_owned()
        } else {
            s
        };
        if s.is_empty() {
            return;
        }
        if user {
            if self.last_user == s && self.last_user_turn == self.turn_id {
                return;
            }
            self.last_user = s.clone();
            self.last_user_turn = self.turn_id.clone();
            if self.card.title.is_empty() {
                self.card.title = short(&s.replace('\n', " "), 70);
            }
            self.record("user", s.clone(), "", None, at, source);
            self.card.asks.push(Quote {
                text: short(&s, 1200),
                at,
                source: source.clone(),
            });
            if self.card.asks.len() > 8 {
                self.card.asks.remove(1);
            }
            self.card.turn = Turn::Running;
            self.card.waiting.clear();
            self.card.error.clear();
            self.card.completed_claim = false;
        } else {
            self.record("assistant", s.clone(), "", None, at, source);
            self.card.last_reply = short(&s, 1400);
            if final_answer {
                self.card.completed_claim = claims_done(&s);
            }
        }
        self.card.updated = self.card.updated.max(at);
    }
    fn phase(&mut self, turn: Turn, id: &str, at: u64, source: &Source) {
        if turn == Turn::Running {
            self.turn_id = if id.is_empty() {
                format!("{at}:{}", source.offset)
            } else {
                id.into()
            };
            self.card.turn_started = at;
            self.card.turn_key = self.turn_id.clone();
            self.card.completed_claim = false;
            self.card.waiting.clear();
            self.card.error.clear();
            self.last_failed.clear();
        }
        let still_waiting = self.card.turn == Turn::Waiting
            && !self.card.waiting.is_empty()
            && turn == Turn::Completed;
        self.card.turn = if still_waiting { Turn::Waiting } else { turn };
        if turn == Turn::Completed && !self.last_failed.is_empty() {
            self.card.turn = Turn::Error;
            self.card.error = self.last_failed.clone();
        }
        self.card.updated = self.card.updated.max(at);
        self.record("turn", format!("{turn:?}"), id, None, at, source);
    }
    fn tool(&mut self, id: &str, name: &str, args: Value, at: u64, source: &Source) {
        if id.is_empty() || name.is_empty() {
            return;
        }
        self.record(
            "tool_call",
            format!("{name} {}", short(&args.to_string(), 8192)),
            id,
            None,
            at,
            source,
        );
        if name == "AskUserQuestion"
            || name.ends_with("request_user_input")
            || name.ends_with("request_user_input_async")
        {
            self.card.turn = Turn::Waiting;
            self.card.waiting = short(&args.to_string(), 600);
        }
        self.calls.insert(
            id.into(),
            Call {
                name: name.into(),
                input: args,
                source: source.clone(),
                at,
            },
        );
        if self.calls.len() > 32 {
            let oldest = self
                .calls
                .iter()
                .min_by_key(|(_, c)| c.source.offset)
                .map(|(k, _)| k.clone())
                .unwrap();
            self.calls.remove(&oldest);
            self.card.unknown_records += 1;
        }
        self.card.updated = self.card.updated.max(at);
    }
    fn fact(&mut self, kind: &str, summary: String, code: Option<i32>, at: u64, source: &Source) {
        self.card.evidence.push(Evidence {
            kind: kind.into(),
            summary: short(&summary, 400),
            exit_code: code,
            at,
            source: source.clone(),
        });
        if self.card.evidence.len() > 16 {
            self.card.evidence.remove(0);
        }
    }
    fn result(
        &mut self,
        id: &str,
        value: &Value,
        explicit: Option<i32>,
        implicit: bool,
        at: u64,
        source: &Source,
    ) {
        let object = input(value);
        let output = object
            .get("output")
            .map(text)
            .unwrap_or_else(|| text(value));
        let codes: Vec<i32> = EXIT
            .captures_iter(&output)
            .filter_map(|c| c[1].parse().ok())
            .collect();
        let structured = object["exit_code"]
            .as_i64()
            .or_else(|| object["exitCode"].as_i64())
            .or_else(|| object["metadata"]["exit_code"].as_i64())
            .map(|n| n as i32);
        let mut code = explicit.or(structured).or_else(|| {
            codes
                .iter()
                .find(|n| **n != 0)
                .or_else(|| codes.last())
                .copied()
        });
        if object["is_error"] == true {
            code = Some(1);
        }
        self.record("tool_result", short(&output, 8192), id, code, at, source);
        let Some(call) = self.calls.get(id).cloned() else {
            self.card.unknown_records += 1;
            return;
        };
        if call.name.ends_with("write_stdin") {
            let key = call.input["session_id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| call.input["session_id"].to_string());
            if let Some(original) = self.executions.get(&key).cloned() {
                self.result(&original, value, code, implicit, at, source);
            }
            self.calls.remove(id);
            return;
        }
        let pending = object
            .get("session_id")
            .filter(|v| !v.is_null())
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
            .or_else(|| SESSION.captures(&output).map(|c| c[1].into()));
        if code.is_none() {
            if let Some(sid) = pending {
                self.executions.insert(sid, id.into());
                return;
            }
        }
        let background = output.contains("running in the background")
            || object.get("backgroundTaskId").is_some();
        if implicit
            && code.is_none()
            && !background
            && matches!(call.name.as_str(), "Bash" | "Edit" | "Write")
        {
            code = Some(0);
        }
        let name = call.name.rsplit(['.', '_']).next().unwrap_or(&call.name);
        let patch = call.name.ends_with("apply_patch");
        if patch && code.is_none() && output.contains("Success. Updated the following files:") {
            code = Some(0);
        }
        if code.is_some_and(|n| n != 0)
            && (output.contains("requires approval")
                || output.contains("haven't granted")
                || output.contains("waiting for approval"))
        {
            self.card.waiting = short(&output, 600);
            self.card.turn = Turn::Waiting;
            self.last_failed.clear();
            self.fact("approval", self.card.waiting.clone(), None, at, source);
            self.calls.remove(id);
            self.card.updated = self.card.updated.max(at);
            return;
        }
        if code.is_some_and(|n| n != 0) {
            self.last_failed = format!("{}: {}", call.name, short(&output, 240));
            self.fact("error", self.last_failed.clone(), code, at, source);
        } else if code == Some(0) {
            self.last_failed.clear();
        }
        if code == Some(0) && (patch || matches!(name, "Edit" | "Write")) {
            let mut paths = Vec::new();
            for key in ["file_path", "path"] {
                if let Some(s) = call.input[key].as_str() {
                    paths.push(s.to_owned());
                }
            }
            let patch_text = call.input["input"]
                .as_str()
                .or_else(|| call.input["patch"].as_str())
                .unwrap_or("");
            paths.extend(PATCH.captures_iter(patch_text).map(|c| c[1].to_owned()));
            for path in paths {
                self.fact("file", format!("文件修改：{path}"), code, at, source);
            }
        }
        if call.name == "Bash" || call.name.ends_with("exec_command") {
            if let Some(command) = call.input["cmd"]
                .as_str()
                .or_else(|| call.input["command"].as_str())
            {
                self.fact("command", short(command, 300), code, at, source);
                // Only receipts from literal test/commit commands qualify; cat/echo/git log never do.
                if verification(command) {
                    let lines: Vec<_> = TEST
                        .find_iter(&output)
                        .map(|m| m.as_str())
                        .take(6)
                        .collect();
                    if !lines.is_empty() && code.is_some() {
                        self.fact(
                            "test",
                            format!("{}：{}", short(command, 100), lines.join("；")),
                            if FAILED.is_match(&output) {
                                Some(1)
                            } else {
                                code
                            },
                            at,
                            source,
                        );
                    } else if code.is_some() {
                        self.fact(
                            "test",
                            format!(
                                "{}（exit {}，未捕获计数）",
                                short(command, 200),
                                code.unwrap()
                            ),
                            code,
                            at,
                            source,
                        );
                    }
                }
                if git_commit(command) && code == Some(0) {
                    for m in COMMIT.find_iter(&output) {
                        self.fact("commit", m.as_str().into(), code, at, source);
                    }
                }
            }
        }
        if self.card.turn == Turn::Waiting
            && (code.is_some()
                || call.name == "AskUserQuestion"
                || call.name.contains("request_user_input"))
        {
            self.card.turn = Turn::Running;
            self.card.waiting.clear();
        }
        self.calls.remove(id);
        self.executions.retain(|_, v| v != id);
        self.card.updated = self.card.updated.max(at);
    }
    pub fn add(&mut self, runtime: Agent, line: &[u8], source: Source) {
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            self.card.unknown_records += 1;
            return;
        };
        let at = seconds(&v["timestamp"]);
        let ty = v["type"].as_str().unwrap_or("");
        let p = &v["payload"];
        let title = if runtime == Agent::Codex {
            p["thread_name"].as_str().or_else(|| p["title"].as_str())
        } else {
            v["customTitle"].as_str().or_else(|| {
                if ty == "summary" {
                    v["summary"].as_str()
                } else {
                    None
                }
            })
        };
        if let Some(title) = title {
            self.card.title = short(title, 100);
            self.card.cli_title = true;
        }
        if runtime == Agent::Codex {
            if ty == "event_msg" && p["type"] != "item_completed" {
                match p["type"].as_str().unwrap_or("") {
                    "task_started" | "turn_started" => self.phase(
                        Turn::Running,
                        p["turn_id"].as_str().unwrap_or(""),
                        at,
                        &source,
                    ),
                    "task_complete" | "task_completed" | "turn_completed" => {
                        if let Some(s) = p["last_agent_message"].as_str() {
                            self.message(false, s.into(), at, &source, true);
                        }
                        self.phase(Turn::Completed, "", at, &source);
                    }
                    "turn_aborted" => self.phase(Turn::Interrupted, "", at, &source),
                    "user_message" => self.message(true, text(&p["message"]), at, &source, false),
                    "agent_message" => self.message(false, text(&p["message"]), at, &source, false),
                    "approval_request" | "permission_request" | "request_user_input" => {
                        self.card.waiting = short(&p.to_string(), 600);
                        self.phase(Turn::Waiting, "", at, &source);
                    }
                    "error" | "turn_failed" => {
                        self.card.error = short(&p.to_string(), 600);
                        self.phase(Turn::Error, "", at, &source);
                    }
                    "token_count" | "agent_reasoning" | "context_compacted" => (),
                    _ => self.card.unknown_records += 1,
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
                    Some("user") => self.message(true, text(&item["content"]), at, &source, false),
                    Some("assistant") => self.message(
                        false,
                        text(&item["content"]),
                        at,
                        &source,
                        item["phase"] == "final_answer" || item["channel"] == "final",
                    ),
                    _ => (),
                },
                "function_call" | "custom_tool_call" => self.tool(
                    item["call_id"]
                        .as_str()
                        .or_else(|| item["id"].as_str())
                        .unwrap_or(""),
                    item["name"].as_str().unwrap_or(""),
                    input(item.get("arguments").unwrap_or(&item["input"])),
                    at,
                    &source,
                ),
                "function_call_output" | "custom_tool_call_output" => self.result(
                    item["call_id"].as_str().unwrap_or(""),
                    &item["output"],
                    item["exit_code"].as_i64().map(|n| n as i32),
                    false,
                    at,
                    &source,
                ),
                "command_execution" | "CommandExecution" => {
                    let id = item["id"]
                        .as_str()
                        .or_else(|| item["call_id"].as_str())
                        .unwrap_or("");
                    if !self.calls.contains_key(id) {
                        self.tool(
                            id,
                            "exec_command",
                            json!({"cmd":item["command"]}),
                            at,
                            &source,
                        );
                    }
                    self.result(
                        id,
                        item.get("output").unwrap_or(&item["aggregated_output"]),
                        item["exit_code"].as_i64().map(|n| n as i32),
                        false,
                        at,
                        &source,
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
                ) =>
                {
                    ()
                }
                _ => self.card.unknown_records += 1,
            }
        } else {
            if v["isSidechain"] == true {
                return;
            }
            let msg = &v["message"];
            let blocks = &msg["content"];
            if ty == "user" && v["isMeta"] != true {
                let s = text(blocks);
                if !s.is_empty() {
                    self.phase(Turn::Running, v["uuid"].as_str().unwrap_or(""), at, &source);
                    self.message(true, s, at, &source, false);
                }
            }
            if let Some(blocks) = blocks.as_array() {
                for b in blocks {
                    match b["type"].as_str() {
                        Some("tool_use") => self.tool(
                            b["id"].as_str().unwrap_or(""),
                            b["name"].as_str().unwrap_or(""),
                            b["input"].clone(),
                            at,
                            &source,
                        ),
                        Some("tool_result") => self.result(
                            b["tool_use_id"].as_str().unwrap_or(""),
                            &b["content"],
                            if b["is_error"] == true { Some(1) } else { None },
                            v["toolUseResult"].get("backgroundTaskId").is_none(),
                            at,
                            &source,
                        ),
                        _ => (),
                    }
                }
            }
            if v["isApiErrorMessage"] == true {
                self.card.error = short(&text(blocks), 600);
                self.phase(Turn::Error, "", at, &source);
                return;
            }
            if ty == "assistant" && msg["model"] != "<synthetic>" {
                let final_answer = matches!(
                    msg["stop_reason"].as_str(),
                    Some("end_turn" | "stop_sequence")
                );
                self.message(false, text(blocks), at, &source, final_answer);
                if final_answer {
                    self.phase(Turn::Completed, "", at, &source);
                }
            }
            if ty == "system" && matches!(v["subtype"].as_str(), Some("turn_duration" | "stop")) {
                self.card.completed_claim = claims_done(&self.card.last_reply);
                self.phase(Turn::Completed, "", at, &source);
            }
        }
    }
    pub fn hook(&mut self, event: &str, at: u64, source: Source) {
        match event {
            "PermissionRequest" => {
                self.card.waiting = "CLI 请求工具审批（Hook）".into();
                self.phase(Turn::Waiting, "", at, &source);
            }
            "Stop" => {
                self.card.completed_claim = claims_done(&self.card.last_reply);
                self.phase(Turn::Completed, "", at, &source);
            }
            _ => (),
        }
    }
}
// Parse argv without executing it. Compound commands remain unknown: their combined exit
// status cannot establish which step succeeded. Quoted arguments are safe to inspect.
fn words(command: &str) -> Vec<String> {
    let Some(words) = shlex::split(command) else {
        return vec![];
    };
    if words
        .iter()
        .any(|w| w.contains([';', '&', '|']) || w.contains('`') || w.contains("$("))
        || command.contains('\n')
    {
        return vec![];
    }
    words
        .into_iter()
        .skip_while(|s| s.contains('=') && !s.starts_with('-'))
        .collect()
}
fn verification(command: &str) -> bool {
    let w = words(command);
    if w.iter()
        .any(|w| matches!(w.as_str(), "--help" | "-h" | "--version"))
    {
        return false;
    }
    let w: Vec<_> = w
        .iter()
        .map(|s| s.rsplit('/').next().unwrap_or(s))
        .collect();
    match w.as_slice() {
        ["cargo", "test" | "check", ..]
        | ["go", "test", ..]
        | ["pytest", ..]
        | ["python" | "python3", "-m", "pytest", ..] => true,
        [
            "bun" | "npm" | "pnpm" | "yarn",
            "test" | "typecheck" | "check",
            ..,
        ] => true,
        [
            "bun" | "npm" | "pnpm" | "yarn",
            "run",
            "test" | "typecheck" | "check",
            ..,
        ] => true,
        _ => false,
    }
}
fn git_commit(command: &str) -> bool {
    let w = words(command);
    w.first()
        .is_some_and(|s| s.rsplit('/').next() == Some("git"))
        && w.get(1).is_some_and(|s| s == "commit")
}
pub fn schema(db: &Connection) -> io::Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS session_content(path TEXT PRIMARY KEY,title TEXT NOT NULL,turn TEXT NOT NULL,updated INTEGER NOT NULL,projection TEXT NOT NULL,parser TEXT NOT NULL); CREATE TABLE IF NOT EXISTS session_records(path TEXT NOT NULL,inode INTEGER NOT NULL,offset INTEGER NOT NULL,ordinal INTEGER NOT NULL,at INTEGER NOT NULL,kind TEXT NOT NULL,text TEXT NOT NULL,call_id TEXT NOT NULL,exit_code INTEGER,PRIMARY KEY(path,inode,offset,ordinal));").map_err(db_err)
}
pub fn load(db: &Connection, path: &str) -> io::Result<Parser> {
    let value: Option<String> = db
        .prepare_cached("SELECT parser FROM session_content WHERE path=?1")
        .map_err(db_err)?
        .query_row([path], |r| r.get(0))
        .optional()
        .map_err(db_err)?;
    value
        .map(|s| serde_json::from_str(&s).map_err(db_err))
        .unwrap_or_else(|| Ok(Parser::default()))
}
pub fn card(db: &Connection, path: &str) -> io::Result<Option<Content>> {
    let value: Option<String> = db
        .prepare_cached("SELECT projection FROM session_content WHERE path=?1")
        .map_err(db_err)?
        .query_row([path], |r| r.get(0))
        .optional()
        .map_err(db_err)?;
    value
        .map(|s| serde_json::from_str(&s).map_err(db_err))
        .transpose()
}
pub fn save(db: &Connection, path: &str, parser: &Parser) -> io::Result<()> {
    db.prepare_cached("INSERT INTO session_content VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET title=excluded.title,turn=excluded.turn,updated=excluded.updated,projection=excluded.projection,parser=excluded.parser").map_err(db_err)?.execute(params![path,parser.card.title,format!("{:?}",parser.card.turn),i64::try_from(parser.card.updated).map_err(db_err)?,serde_json::to_string(&parser.card).map_err(db_err)?,serde_json::to_string(parser).map_err(db_err)?]).map_err(db_err)?;
    let mut ordinals = BTreeMap::new();
    for r in &parser.records {
        let i = ordinals
            .entry((r.source.inode, r.source.offset))
            .or_insert(0u64);
        let ordinal = *i;
        *i += 1;
        db.prepare_cached(
            "INSERT OR IGNORE INTO session_records VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        )
        .map_err(db_err)?
        .execute(params![
            path,
            i64::try_from(r.source.inode).map_err(db_err)?,
            i64::try_from(r.source.offset).map_err(db_err)?,
            ordinal as i64,
            i64::try_from(r.at).map_err(db_err)?,
            r.kind,
            r.text,
            r.call_id,
            r.code
        ])
        .map_err(db_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn add(p: &mut Parser, v: Value) {
        let n = p.records.len() as u64;
        p.add(
            Agent::Codex,
            &serde_json::to_vec(&v).unwrap(),
            Source {
                path: "fixture".into(),
                inode: 1,
                offset: n * 1024,
            },
        );
    }
    fn item(p: &mut Parser, v: Value) {
        add(
            p,
            json!({"type":"response_item","timestamp":"2026-10-21T00:00:00Z","payload":v}),
        );
    }
    fn call(p: &mut Parser, id: &str, name: &str, input: Value) {
        item(
            p,
            json!({"type":"function_call","call_id":id,"name":name,"arguments":input.to_string()}),
        );
    }
    fn result(p: &mut Parser, id: &str, s: &str) {
        item(
            p,
            json!({"type":"function_call_output","call_id":id,"output":s}),
        );
    }
    #[test]
    fn user_dedup_additions_and_claims_are_not_receipts() {
        let mut p = Parser::default();
        add(
            &mut p,
            json!({"type":"event_msg","payload":{"type":"user_message","message":"修复输入法"}}),
        );
        item(
            &mut p,
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":"修复输入法"}]}),
        );
        item(
            &mut p,
            json!({"type":"message","role":"user","content":[{"type":"input_text","text":"追加：不要提交"}]}),
        );
        item(
            &mut p,
            json!({"type":"message","role":"assistant","phase":"final_answer","content":[{"text":"已完成，全部测试通过"}]}),
        );
        assert_eq!(p.card.asks.len(), 2);
        assert!(p.card.completed_claim);
        assert!(p.card.here().done.contains("尚无"));
    }
    #[test]
    fn real_exit_receipts_and_async_correlation_survive_restart() {
        let mut p = Parser::default();
        call(
            &mut p,
            "a",
            "functions.exec_command",
            json!({"cmd":"cargo test --lib"}),
        );
        result(&mut p, "a", "Process running with session ID 123");
        assert!(!p.card.evidence.iter().any(|e| e.kind == "test"));
        let mut p: Parser = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        call(
            &mut p,
            "poll",
            "functions.write_stdin",
            json!({"session_id":123}),
        );
        result(
            &mut p,
            "poll",
            "Process exited with code 0\ntest result: ok. 12 passed; 0 failed",
        );
        assert!(p.card.here().done.contains("12 passed"));
        call(
            &mut p,
            "fake",
            "functions.exec_command",
            json!({"cmd":"cat old-test.log"}),
        );
        result(
            &mut p,
            "fake",
            "Process exited with code 0\ntest result: ok. 999 passed",
        );
        assert!(!p.card.here().done.contains("999"));
    }
    #[test]
    fn patch_commit_and_failed_test_are_sourced() {
        let mut p = Parser::default();
        call(
            &mut p,
            "p",
            "functions.apply_patch",
            json!({"input":"*** Begin Patch\n*** Update File: a.rs\n*** End Patch"}),
        );
        result(&mut p, "p", "Success. Updated the following files:\nM a.rs");
        call(
            &mut p,
            "g",
            "functions.exec_command",
            json!({"cmd":"git commit -m 'fix input'"}),
        );
        result(
            &mut p,
            "g",
            "Process exited with code 0\n[main abcdef1] fix input",
        );
        call(
            &mut p,
            "t",
            "functions.exec_command",
            json!({"cmd":"cargo test"}),
        );
        result(
            &mut p,
            "t",
            "Process exited with code 101\ntest result: FAILED. 2 passed; 1 failed",
        );
        add(
            &mut p,
            json!({"type":"event_msg","payload":{"type":"task_complete"}}),
        );
        assert_eq!(p.card.turn, Turn::Error);
        assert!(p.card.here().done.contains("a.rs"));
        assert!(p.card.here().done.contains("abcdef1"));
        assert!(!p.card.here().done.contains("2 passed"));
        assert!(p.card.evidence.iter().all(|e| e.source.inode == 1));
    }
    #[test]
    fn claude_edit_permission_and_stop() {
        let mut p = Parser::default();
        let s = Source::default();
        for v in [
            json!({"type":"user","uuid":"u","message":{"content":"修复错误"}}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"e","name":"Edit","input":{"file_path":"a.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"e","content":"Successfully updated"}]}}),
        ] {
            p.add(Agent::Claude, &serde_json::to_vec(&v).unwrap(), s.clone());
        }
        assert_eq!(p.card.asks.len(), 1);
        assert!(p.card.here().done.contains("a.rs"));
        p.hook("PermissionRequest", 10, s.clone());
        assert_eq!(p.card.turn, Turn::Waiting);
        p.hook("Stop", 11, s.clone());
        assert_eq!(p.card.turn, Turn::Waiting, "Stop is not an approval answer");
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"new","message":{"content":"批准，请继续"}}"#.as_bytes(),
            s.clone(),
        );
        p.hook("Stop", 12, s);
        assert_eq!(p.card.turn, Turn::Completed);
    }
    #[test]
    fn injected_messages_and_quoted_completion_are_not_user_intent_or_claims() {
        let mut p = Parser::default();
        for text in [
            "<cross-session-message from='peer'>用户说已完成</cross-session-message>",
            "<external_codex_apps_open_page>{}</external_codex_apps_open_page>",
        ] {
            add(
                &mut p,
                json!({"type":"event_msg","payload":{"type":"user_message","message":text}}),
            );
        }
        assert!(p.card.asks.is_empty());
        assert!(!claims_done(
            "请执行这个计划：\n```\n完成后回复：已完成\n```"
        ));
        assert!(!claims_done("尚未完成"));
        assert!(!verification("cargo test --lib; echo ok"));
        assert!(!verification("cargo test --help"));
    }
    #[test]
    fn source_rows_are_idempotent() {
        let db = Connection::open_in_memory().unwrap();
        schema(&db).unwrap();
        let mut p = Parser::default();
        call(
            &mut p,
            "t",
            "functions.exec_command",
            json!({"cmd":"cargo test"}),
        );
        result(
            &mut p,
            "t",
            "Process exited with code 0\ntest result: ok. 1 passed",
        );
        save(&db, "fixture", &p).unwrap();
        save(&db, "fixture", &p).unwrap();
        let n: i64 = db
            .query_row("SELECT COUNT(*) FROM session_records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, p.records.len() as i64);
        assert_eq!(load(&db, "fixture").unwrap().card, p.card);
    }
}
