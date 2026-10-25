//! Record-local normalization, sourced receipts and durable pending tool correlation.
use keepline_core::{
    content::{short, Content, Evidence, Quote, Source, Turn},
    model::Agent,
    protocol::HookFacts,
};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
        r"(?i)(?:^\s*完成(?:[。.!！，,:：]|\s*$)|已(?:完成|修复|实现|提交)|修复完成|全部.{0,12}通过|完成了|implemented|fixed|completed|all tests pass|\bdone\b)",
    )
});
fn claims_done(s: &str) -> bool {
    let mut fence: Option<(char, usize)> = None;
    for line in s.lines() {
        let trimmed = line.trim_start();
        let marker = trimmed
            .chars()
            .next()
            .filter(|c| matches!(c, '`' | '~'))
            .map(|c| (c, trimmed.chars().take_while(|v| *v == c).count()));
        if let Some((ch, n)) = marker.filter(|(_, n)| *n >= 3) {
            match fence {
                None => fence = Some((ch, n)),
                Some((open, len)) if open == ch && n >= len => fence = None,
                _ => (),
            };
            continue;
        }
        if fence.is_some() || trimmed.starts_with('>') {
            continue;
        }
        // A negation only cancels the claim in its own sentence: "完成。…（可选项尚未实现）"
        // still claims the requested work is done. Commas do not end a sentence, so
        // "完成，但尚未测试" stays unclaimed.
        if CLAIM.find_iter(line).any(|m| {
            !NEGATIONS
                .iter()
                .any(|no| sentence(line, m.start()).contains(no))
        }) {
            return true;
        }
    }
    false
}
const NEGATIONS: [&str; 6] = [
    "未完成",
    "尚未",
    "没有完成",
    "还没",
    "not completed",
    "not done",
];
/// The sentence of `line` containing byte `at`.
fn sentence(line: &str, at: usize) -> &str {
    let ends = |i: usize, c: char| {
        matches!(c, '。' | '！' | '？' | '；' | '!' | '?' | ';')
            || (c == '.' && line[i + 1..].chars().next().is_none_or(char::is_whitespace))
    };
    let start = line[..at]
        .char_indices()
        .rev()
        .find(|&(i, c)| ends(i, c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let end = line[at..]
        .char_indices()
        .find(|&(i, c)| ends(at + i, c))
        .map_or(line.len(), |(i, c)| at + i + c.len_utf8());
    &line[start..end]
}
// Strip bounded, explicit wrappers; do not infer the author of arbitrary unmarked prose.
fn user_text(s: &str) -> String {
    let mut clean = s.to_owned();
    if clean
        .trim_start()
        .starts_with("# Files mentioned by the user:")
    {
        clean = clean
            .split_once("## My request:")
            .map(|(_, request)| request.to_owned())
            .unwrap_or_default();
    }

    let reply_open = "<send_user_message_question_reply>";
    let reply_close = "</send_user_message_question_reply>";
    if let Some(start) = clean.find(reply_open) {
        if let Some(end) = clean[start + reply_open.len()..].find(reply_close) {
            let body = &clean[start + reply_open.len()..start + reply_open.len() + end];
            let answers = serde_json::from_str::<Vec<Value>>(body)
                .ok()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v["answer"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            clean.replace_range(
                start..start + reply_open.len() + end + reply_close.len(),
                &answers,
            );
        }
    }

    // A copied web page can carry hundreds of lines before the user's own suffix.
    if let Some(start) = clean.find("要查看键盘快捷键，按下问号") {
        if let Some(end) = clean[start..].rfind("X Corp.") {
            clean.replace_range(start..start + end + "X Corp.".len(), "\n");
        } else {
            clean.truncate(start);
        }
    }

    for tag in [
        "system-reminder",
        "environment_context",
        "instructions",
        "INSTRUCTIONS",
        "skills_instructions",
        "permissions instructions",
        "collaboration_mode",
        "command-name",
        "command-message",
        "command-args",
        "local-command-caveat",
        "local-command-stdout",
        "hook-output",
        "hook-context",
        "task-notification",
        "cross-session-message",
        "side-sessions-event",
        "teammate-message",
        "context",
        "quoted_text",
        "paste",
        "pasted_text",
        "pasted_content",
        "in-app-browser-context",
        "reference",
        "file_contents",
        "skill",
        "skill_content",
        "hook-additional-context",
    ] {
        let open = format!("<{tag}");
        let close = format!("</{tag}");
        while let Some(start) = clean.find(&open) {
            let boundary = clean.as_bytes().get(start + open.len()).copied();
            if !boundary.is_some_and(|c| c == b'>' || c.is_ascii_whitespace()) {
                break;
            }
            if let Some(end) = clean[start..]
                .find(&close)
                .and_then(|end| clean[start + end..].find('>').map(|last| end + last + 1))
            {
                clean.replace_range(start..start + end, "\n");
            } else {
                clean.truncate(start);
                break;
            }
        }
    }
    // Unquoted multi-bullet assistant explanations sometimes have the user's instruction
    // appended after the final sentence. Only retain that explicit imperative suffix.
    if clean
        .lines()
        .filter(|l| l.trim_start().starts_with("* "))
        .count()
        >= 2
        && (clean.contains("所以我按") || clean.contains("所以我先") || clean.contains("我先选了"))
    {
        if let Some((_, suffix)) = clean.rsplit_once('。') {
            let suffix = suffix.trim();
            if [
                "删掉",
                "删除",
                "请",
                "帮我",
                "你帮我",
                "我希望",
                "我想",
                "你看看",
            ]
            .iter()
            .any(|s| suffix.starts_with(s))
            {
                clean = suffix.to_owned();
            } else {
                return String::new();
            }
        } else {
            return String::new();
        }
    }
    // Standalone instruction/skill payloads are generated context, not a user request.
    if CONTEXT.is_match(clean.trim()) {
        return String::new();
    }
    let mut lines = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut instruction = false;
    for line in clean.lines() {
        let t = line.trim();
        if matches!(t, "## My request:" | "# User request" | "# 用户要求") {
            instruction = false;
            continue;
        }
        if t.starts_with("# AGENTS.md") || t.starts_with("# CLAUDE.md") {
            instruction = !s.contains("<INSTRUCTIONS>") && !s.contains("<instructions>");
            continue;
        }
        if instruction {
            if t.starts_with("# User request") || t.starts_with("# 用户要求") {
                instruction = false;
            }
            continue;
        }
        let marker = t
            .chars()
            .next()
            .filter(|c| matches!(c, '`' | '~'))
            .map(|c| (c, t.chars().take_while(|v| *v == c).count()));
        if let Some((c, n)) = marker.filter(|(_, n)| *n >= 3) {
            match fence {
                None => fence = Some((c, n)),
                Some((ch, len)) if c == ch && n >= len => fence = None,
                _ => (),
            }
            continue;
        }
        if fence.is_some() || t.starts_with('>') {
            continue;
        }
        if !t.is_empty()
            || lines
                .last()
                .is_some_and(|last: &&str| !last.trim().is_empty())
        {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_owned()
}
static CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?s)^\s*(?:# Global Codex Working Agreements|\[Request interrupted by user(?: for tool use)?\]|This session is being continued from a previous conversation|Background \(from the session that started this one\):|Base directory for this skill:|<turn_aborted>|<external_codex_apps_|<skill(?:\s|>)|<available_skills>|<skills>)",
    )
});

// A session_meta line may exceed the 16 KiB head budget because of base_instructions.
// Decode only complete top-level metadata values in its prefix, never scan its body.
pub fn header_value(runtime: Agent, bytes: &[u8]) -> Option<Value> {
    if let Ok(v) = serde_json::from_slice(bytes) {
        return Some(v);
    }
    if runtime != Agent::Codex {
        return None;
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
fn question_summary(v: &Value) -> String {
    let questions = v.get("questions").and_then(Value::as_array);
    if let Some(questions) = questions {
        return short(
            &questions
                .iter()
                .filter_map(|q| {
                    let prompt = q["question"].as_str().or_else(|| q["title"].as_str())?;
                    let options = q["options"]
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|o| {
                                    o.as_str()
                                        .or_else(|| o["label"].as_str())
                                        .map(str::to_owned)
                                })
                                .collect::<Vec<_>>()
                                .join(" / ")
                        })
                        .unwrap_or_default();
                    Some(if options.is_empty() {
                        prompt.to_owned()
                    } else {
                        format!("{prompt}（{options}）")
                    })
                })
                .collect::<Vec<_>>()
                .join("\n"),
            600,
        );
    }
    short(
        v["question"]
            .as_str()
            .or_else(|| v["message"].as_str())
            .or_else(|| v["reason"].as_str())
            .unwrap_or("需要在原终端回答问题或审批"),
        600,
    )
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
    /// A turn end arrived while a dialog was still shown; settles the turn once the dialog
    /// is known to be gone.
    #[serde(default)]
    completed_while_waiting: bool,
    /// The turn a subagent's dialog interrupted, restored once that dialog is gone.
    #[serde(default)]
    child_wait_from: Option<Turn>,
    #[serde(skip)]
    pub records: Vec<Record>,
    /// "Running→Waiting Hook" lines for the status trace, one per change of `card.turn`.
    #[serde(skip)]
    pub transitions: Vec<String>,
}
/// What `claude agents --json` says about a live session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Official {
    Waiting(String),
    Busy,
    Idle,
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
    pub fn observe_header(&mut self, runtime: Agent, line: &[u8]) {
        if let Some(v) = header_value(runtime, line) {
            self.metadata(runtime, &v);
        }
    }
    fn metadata(&mut self, runtime: Agent, v: &Value) {
        let m = if runtime == Agent::Codex {
            &v["payload"]
        } else {
            v
        };
        // Whatever hides it, a Codex subagent still belongs to its parent.
        if runtime == Agent::Codex && v["type"] == "session_meta" {
            let spawn = &m["source"]["subagent"]["thread_spawn"];
            if let Some(parent) = spawn["parent_thread_id"]
                .as_str()
                .or(m["parent_thread_id"].as_str())
            {
                self.card.parent = parent.to_owned();
                self.card.agent_name = m["agent_nickname"]
                    .as_str()
                    .or(spawn["agent_path"].as_str())
                    .unwrap_or("")
                    .to_owned();
            }
        }
        let cwd = m["cwd"].as_str().unwrap_or("");
        let temporary = [
            "/tmp",
            "/private/tmp",
            "/var/tmp",
            "/private/var/tmp",
            "/var/folders",
            "/private/var/folders",
        ]
        .iter()
        .any(|root| cwd == *root || cwd.starts_with(&format!("{root}/")));
        let reason = if temporary {
            Some(keepline_core::content::TEMPORARY_DIRECTORY)
        } else if runtime == Agent::Claude {
            let entry = m["entrypoint"].as_str().unwrap_or("");
            let user_type = m["userType"].as_str().unwrap_or("");
            if m["isSidechain"] == true
                || m.get("agentId")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty())
            {
                Some("Claude 子代理 / sidechain")
            } else if matches!(entry, "hook" | "sdk-cli")
                || m["isInteractive"] == false
                || m["argv"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x == "-p" || x == "--print"))
                || entry.contains("sdk")
                || entry.contains("print")
                || m["isNonInteractive"] == true
                || m["print"] == true
            {
                Some("Claude 非交互 / SDK")
            } else if !user_type.is_empty() && user_type != "external" {
                Some("Claude 内部 / Hook 用户来源")
            } else {
                if matches!(entry, "cli" | "claude-desktop" | "vscode") && user_type == "external" {
                    self.card.interactive = true;
                }
                None
            }
        } else if v["type"] == "session_meta" {
            let source = m["source"].as_str().unwrap_or("");
            let origin = m["originator"].as_str().unwrap_or("");
            if m["source"].get("subagent").is_some()
                || matches!(
                    m["thread_source"].as_str(),
                    Some("subagent" | "guardian_review")
                )
            {
                Some("Codex 子代理")
            } else if source == "exec" || origin.contains("exec") || m["is_exec"] == true {
                Some("Codex exec 非交互")
            } else if origin.contains("sdk") || matches!(source, "sdk" | "api") {
                Some("Codex SDK 非交互")
            } else {
                if matches!(source, "cli" | "vscode")
                    && matches!(m["thread_source"].as_str(), None | Some("user"))
                {
                    self.card.interactive = true;
                }
                None
            }
        } else {
            None
        };
        if let Some(reason) = reason {
            if self.card.hidden_reason.is_empty() {
                self.card.hidden_reason = reason.into();
            }
        }
        // Some launchers inherit the desktop entrypoint. Require an explicit machine task marker.
        let prompt = if v["type"] == "queue-operation" {
            text(&v["content"])
        } else if runtime == Agent::Claude && v["type"] == "user" {
            text(&v["message"]["content"])
        } else {
            String::new()
        };
        if (prompt.starts_with("Use the you-are-here skill")
            && prompt.contains("This is an unattended background refresh:"))
            || prompt.starts_with("This is an unattended background refresh:")
            || prompt.starts_with("You are a Claude-Mem")
            || prompt.starts_with("You are a memory extraction agent")
            || prompt.starts_with("<remem-hook")
        {
            self.card.hidden_reason = "Hook / 无人值守后台任务".into();
        }
    }
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
        let s = if user { user_text(&s) } else { s };
        if s.is_empty() {
            return;
        }
        if user {
            if self
                .card
                .asks
                .iter()
                .any(|q| q.source == *source && q.text == s)
            {
                return;
            }
            if self.last_user == s && self.last_user_turn == self.turn_id {
                return;
            }
            self.last_user = s.clone();
            self.last_user_turn = self.turn_id.clone();
            if self.card.asks.is_empty() && !self.card.partial_history {
                self.card.first_ask_known = true;
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
            self.card.background = None;
            self.last_failed.clear();
            self.completed_while_waiting = false;
            self.child_wait_from = None;
        }
        let still_waiting = self.card.turn == Turn::Waiting
            && !self.card.waiting.is_empty()
            && turn == Turn::Completed;
        self.completed_while_waiting |= still_waiting;
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
            self.card.waiting = question_summary(&args);
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
        let before = self.card.turn;
        self.add_line(runtime, line, source);
        self.settle(before, "日志", self.card.updated);
    }
    fn add_line(&mut self, runtime: Agent, line: &[u8], source: Source) {
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            self.card.unknown_records += 1;
            return;
        };
        self.metadata(runtime, &v);
        let at = seconds(&v["timestamp"]);
        let ty = v["type"].as_str().unwrap_or("");
        let p = &v["payload"];
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
                        self.card.waiting = question_summary(p);
                        self.phase(Turn::Waiting, "", at, &source);
                    }
                    "error" | "turn_failed" => {
                        self.card.error = short(
                            &text(p.get("message").or_else(|| p.get("error")).unwrap_or(p)),
                            600,
                        );
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
                ) => {}
                _ => self.card.unknown_records += 1,
            }
        } else {
            if v["isSidechain"] == true {
                return;
            }
            let msg = &v["message"];
            let blocks = &msg["content"];
            if ty == "user" && v["isMeta"] != true {
                let s = user_text(&text(blocks));
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
    pub fn hook(&mut self, event: &str, facts: &HookFacts, at: u64, source: Source) {
        let before = self.card.turn;
        self.hook_event(event, facts, at, &source);
        self.settle(before, "Hook", at);
    }
    fn hook_event(&mut self, event: &str, facts: &HookFacts, at: u64, source: &Source) {
        // SubagentStop lists the parent's tasks; other subagent events say nothing about
        // the parent turn.
        if event == "SubagentStop" {
            if facts.background.is_some() {
                self.card.background = facts.background;
            }
            return;
        }
        if facts.subagent {
            return;
        }
        // Work that will resume the session: a tool after the stop is that resumed turn.
        let keeps_working = self
            .card
            .background
            .is_some_and(|b| b.keeps_working() || b.wakes());
        match event {
            "PermissionRequest" => self.wait("CLI 请求工具审批（Hook）", at, source),
            "Notification" => match facts.notification.as_deref() {
                Some("permission_prompt") => self.wait("CLI 请求工具审批（Hook）", at, source),
                Some("elicitation_dialog") => self.wait("MCP 表单等待填写（Hook）", at, source),
                Some("elicitation_url_dialog") => {
                    self.wait("MCP 请求在浏览器中完成（Hook）", at, source)
                }
                Some("agent_needs_input") => self.wait("Agent 等待你的输入（Hook）", at, source),
                _ => (),
            },
            "Stop" => {
                // Absent arrays are unknown; an older turn's tasks never carry over.
                self.card.background = facts.background;
                self.card.completed_claim = claims_done(&self.card.last_reply);
                self.phase(Turn::Completed, "", at, source);
            }
            "StopFailure" => {
                if self.card.error.is_empty() {
                    self.card.error = "API 错误中止了回合（Hook）".into();
                }
                self.phase(Turn::Error, "", at, source);
            }
            // A tool finishing after a hook-reported dialog means the dialog was answered.
            "PostToolUse" | "PostToolUseFailure"
                if self.card.turn == Turn::Waiting && self.card.signal == "Hook" =>
            {
                self.card.turn = Turn::Running;
                self.card.waiting.clear();
            }
            // Turns woken by background work send no prompt; a tool after a stop is the only
            // sign of one. Without running work it is a straggler from the finished turn.
            "PreToolUse" | "PostToolUse" | "PostToolUseFailure"
                if self.card.turn == Turn::Completed && keeps_working =>
            {
                self.phase(Turn::Running, "", at, source);
            }
            _ => (),
        }
    }
    fn wait(&mut self, reason: &str, at: u64, source: &Source) {
        // A dialog already known from the transcript or screen keeps its richer text.
        if self.card.turn != Turn::Waiting {
            self.card.waiting = reason.into();
            self.phase(Turn::Waiting, "", at, source);
        }
    }
    /// Applies `claude agents --json`. It only settles a dialog: it never starts or ends a
    /// turn the transcript and hooks have not reported, because it can lag either of them.
    pub fn official(&mut self, status: &Official, at: u64, source: Source) {
        let before = self.card.turn;
        match status {
            Official::Waiting(reason) if self.card.turn != Turn::Waiting => {
                self.card.waiting = format!("{reason}（Claude 官方状态）");
                self.phase(Turn::Waiting, "", at, &source);
            }
            Official::Busy if self.card.turn == Turn::Waiting => {
                self.card.turn = Turn::Running;
                self.card.waiting.clear();
            }
            Official::Idle if self.card.turn == Turn::Waiting => {
                self.card.waiting.clear();
                let settled = if self.completed_while_waiting {
                    Turn::Completed
                } else {
                    Turn::Interrupted
                };
                self.phase(settled, "", at, &source);
            }
            _ => (),
        }
        self.settle(before, "官方状态", at);
    }
    /// Applies the state of a Codex session's subagents, read from their own logs: running
    /// ones keep a stopped turn working, and a subagent's dialog is the parent's dialog.
    pub fn children(&mut self, kids: &Children, at: u64, source: Source) {
        let before = self.card.turn;
        if kids.running > 0 || self.card.background.is_some() {
            let mut b = self.card.background.unwrap_or_default();
            b.agents = kids.running;
            self.card.background = Some(b);
        }
        match &kids.waiting {
            Some(w) if self.card.turn != Turn::Waiting => {
                self.child_wait_from = Some(self.card.turn);
                self.card.waiting = w.clone();
                self.phase(Turn::Waiting, "", at, &source);
            }
            None if self.card.turn == Turn::Waiting => {
                if let Some(turn) = self.child_wait_from.take() {
                    self.card.waiting.clear();
                    self.card.turn = turn;
                }
            }
            _ => (),
        }
        self.settle(before, "子 Agent", at);
    }
    fn settle(&mut self, before: Turn, signal: &str, at: u64) {
        if self.card.turn == before {
            return;
        }
        if self.card.turn == Turn::Waiting {
            self.card.waiting_episode += 1;
        } else {
            self.completed_while_waiting = false;
        }
        self.card.signal = signal.into();
        self.card.signal_at = at;
        self.transitions
            .push(format!("{before:?}→{:?} {signal}", self.card.turn));
    }
}
/// What a parent's subagents are doing, from `subagent_links`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Children {
    pub running: u32,
    pub waiting: Option<String>,
}
/// A subagent whose log has not moved for this long no longer counts as running: its
/// process may have died mid-turn.
const CHILD_FRESH: u64 = 900;
pub fn link_child(db: &Connection, path: &str, card: &Content) -> io::Result<()> {
    db.prepare_cached("INSERT INTO subagent_links VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET parent=excluded.parent,name=excluded.name,turn=excluded.turn,waiting=excluded.waiting,updated=excluded.updated")
        .map_err(db_err)?
        .execute(params![
            path,
            card.parent,
            card.agent_name,
            format!("{:?}", card.turn),
            card.waiting,
            i64::try_from(card.updated).map_err(db_err)?
        ])
        .map_err(db_err)?;
    Ok(())
}
pub fn children(db: &Connection, parent: &str, now: u64) -> io::Result<Children> {
    let mut q = db
        .prepare_cached("SELECT name,turn,waiting FROM subagent_links WHERE parent=?1 AND updated>=?2 ORDER BY updated DESC")
        .map_err(db_err)?;
    let since = i64::try_from(now.saturating_sub(CHILD_FRESH)).map_err(db_err)?;
    let mut rows = q.query(params![parent, since]).map_err(db_err)?;
    let mut kids = Children::default();
    while let Some(r) = rows.next().map_err(db_err)? {
        let name: String = r.get(0).map_err(db_err)?;
        let turn: String = r.get(1).map_err(db_err)?;
        match turn.as_str() {
            "Running" => kids.running += 1,
            "Waiting" if kids.waiting.is_none() => {
                let waiting: String = r.get(2).map_err(db_err)?;
                let name = if name.is_empty() {
                    "子 Agent".into()
                } else {
                    format!("子 Agent {name}")
                };
                kids.waiting = Some(format!("{name}：{}", short(&waiting, 500)));
                kids.running += 1;
            }
            _ => (),
        }
    }
    Ok(kids)
}
/// Reads the status facts from a hook payload. Stop and SubagentStop list the parent
/// session's in-flight tasks; an entry counts unless its status says it has ended.
pub fn hook_facts(v: &Value) -> HookFacts {
    const ENDED: [&str; 9] = [
        "completed",
        "complete",
        "done",
        "failed",
        "error",
        "killed",
        "stopped",
        "cancelled",
        "canceled",
    ];
    let tasks = v["background_tasks"].as_array();
    let crons = v["session_crons"].as_array();
    let background = (tasks.is_some() || crons.is_some()).then(|| {
        let mut b = keepline_core::content::Background {
            crons: crons.map_or(0, |c| c.len() as u32),
            ..Default::default()
        };
        for task in tasks.into_iter().flatten() {
            let status = task["status"].as_str().unwrap_or("").to_lowercase();
            if ENDED.contains(&status.as_str()) {
                continue;
            }
            match task["type"].as_str().unwrap_or("") {
                "shell" => b.shells += 1,
                "monitor" => b.monitors += 1,
                _ => b.agents += 1,
            }
        }
        b
    });
    HookFacts {
        background,
        notification: v["notification_type"].as_str().map(str::to_owned),
        subagent: v["agent_id"].as_str().is_some_and(|id| !id.is_empty()),
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
    matches!(
        w.as_slice(),
        ["cargo", "test" | "check", ..]
            | ["go", "test", ..]
            | ["pytest", ..]
            | ["python" | "python3", "-m", "pytest", ..]
            | [
                "bun" | "npm" | "pnpm" | "yarn",
                "test" | "typecheck" | "check",
                ..,
            ]
            | [
                "bun" | "npm" | "pnpm" | "yarn",
                "run",
                "test" | "typecheck" | "check",
                ..,
            ]
    )
}
fn git_commit(command: &str) -> bool {
    let w = words(command);
    w.first()
        .is_some_and(|s| s.rsplit('/').next() == Some("git"))
        && w.get(1).is_some_and(|s| s == "commit")
}
pub fn schema(db: &Connection) -> io::Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS session_content(path TEXT PRIMARY KEY,title TEXT NOT NULL,turn TEXT NOT NULL,updated INTEGER NOT NULL,projection TEXT NOT NULL,parser TEXT NOT NULL); CREATE TABLE IF NOT EXISTS subagent_links(path TEXT PRIMARY KEY,parent TEXT NOT NULL,name TEXT NOT NULL,turn TEXT NOT NULL,waiting TEXT NOT NULL,updated INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS subagent_parent ON subagent_links(parent); CREATE TABLE IF NOT EXISTS session_records(path TEXT NOT NULL,inode INTEGER NOT NULL,offset INTEGER NOT NULL,ordinal INTEGER NOT NULL,at INTEGER NOT NULL,kind TEXT NOT NULL,text TEXT NOT NULL,call_id TEXT NOT NULL,exit_code INTEGER,PRIMARY KEY(path,inode,offset,ordinal));").map_err(db_err)
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
/// With `KEEPLINE_STATUS_TRACE=<file>`, appends one line per turn change for regression
/// checks against a transcript. Paths and states only; no prompt or reply text.
fn trace(path: &str, parser: &Parser) {
    let Some(file) = std::env::var_os("KEEPLINE_STATUS_TRACE") else {
        return;
    };
    if parser.transitions.is_empty() {
        return;
    }
    let lines: String = parser
        .transitions
        .iter()
        .map(|t| format!("{} {path} {t}\n", keepline_core::content::now()))
        .collect();
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .and_then(|mut f| std::io::Write::write_all(&mut f, lines.as_bytes()));
}
pub fn save(db: &Connection, path: &str, parser: &Parser) -> io::Result<()> {
    trace(path, parser);
    let mut encoded = serde_json::to_value(parser).map_err(db_err)?;
    encoded["_rules"] = json!(8);
    db.prepare_cached("INSERT INTO session_content VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET title=excluded.title,turn=excluded.turn,updated=excluded.updated,projection=excluded.projection,parser=excluded.parser").map_err(db_err)?.execute(params![path,parser.card.title,format!("{:?}",parser.card.turn),i64::try_from(parser.card.updated).map_err(db_err)?,serde_json::to_string(&parser.card).map_err(db_err)?,serde_json::to_string(&encoded).map_err(db_err)?]).map_err(db_err)?;
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
    fn error_cards_use_message_text_not_event_json() {
        let mut p = Parser::default();
        add(
            &mut p,
            json!({"type":"event_msg","payload":{"type":"error","message":"测试失败，退出码 1"}}),
        );
        assert_eq!(p.card.error, "测试失败，退出码 1");
    }
    #[test]
    fn codex_interactive_entrypoint_is_not_a_brand_whitelist() {
        for origin in [
            "Codex Desktop",
            "codex-tui",
            "codex_work_desktop",
            "herdr-client",
        ] {
            let mut p = Parser::default();
            p.metadata(Agent::Codex, &json!({"type":"session_meta","payload":{"source":"vscode","thread_source":"user","originator":origin,"cwd":"/repo"}}));
            assert!(p.card.hidden_reason().is_none(), "{origin}");
            p.metadata(
                Agent::Codex,
                &json!({"type":"session_meta","payload":{"source":"exec","originator":origin}}),
            );
            assert!(p.card.hidden_reason().is_some());
        }
    }
    #[test]
    fn questions_show_text_and_choices_instead_of_json() {
        let mut p = Parser::default();
        call(
            &mut p,
            "question",
            "AskUserQuestion",
            json!({"questions":[{"question":"要部署吗？","options":[{"label":"仅本地","description":"保留"},{"label":"部署"}]}]}),
        );
        assert_eq!(p.card.waiting, "要部署吗？（仅本地 / 部署）");
        assert_eq!(
            question_summary(&json!({"questions":[{"title":"选一个","options":["A","B"]}]})),
            "选一个（A / B）"
        );
    }
    #[test]
    fn tail_only_user_is_an_addition_not_a_fabricated_first_request() {
        let mut p = Parser::default();
        p.card.partial_history = true;
        add(
            &mut p,
            json!({"type":"event_msg","payload":{"type":"user_message","message":"继续"}}),
        );
        assert!(p.card.title.is_empty());
        assert!(!p.card.first_ask_known);
        assert!(p.card.here().ask.starts_with("最初要求未覆盖"));
    }
    #[test]
    fn interactive_metadata_and_automation_override_are_not_title_blacklists() {
        let src = Source::default();
        for (meta, hidden) in [
            (
                json!({"entrypoint":"cli","userType":"external","isSidechain":false,"cwd":"/repo"}),
                false,
            ),
            (
                json!({"entrypoint":"claude-desktop","userType":"external","cwd":"/repo"}),
                false,
            ),
            (
                json!({"entrypoint":"sdk-cli","userType":"external","cwd":"/repo"}),
                true,
            ),
            (
                json!({"entrypoint":"cli","userType":"external","argv":["claude","-p","hello"]}),
                true,
            ),
            (json!({"entrypoint":"cli","userType":"internal"}), true),
            (
                json!({"entrypoint":"cli","userType":"external","isSidechain":true}),
                true,
            ),
            (
                json!({"entrypoint":"cli","userType":"external","cwd":"/private/tmp/check"}),
                true,
            ),
            (json!({"cwd":"/repo"}), true),
        ] {
            let mut p = Parser::default();
            p.metadata(Agent::Claude, &meta);
            assert_eq!(p.card.hidden_reason().is_some(), hidden, "{meta}");
        }
        let mut p = Parser::default();
        p.metadata(
            Agent::Claude,
            &json!({"entrypoint":"claude-desktop","userType":"external"}),
        );
        p.add(Agent::Claude,serde_json::to_string(&json!({"type":"user","message":{"content":"请解释 you-are-here 和 remem 怎么使用"}})).unwrap().as_bytes(),src.clone());
        assert!(p.card.hidden_reason().is_none());
        p.metadata(Agent::Claude,&json!({"type":"queue-operation","content":"Use the you-are-here skill. This is an unattended background refresh: never ask questions"}));
        assert!(p.card.hidden_reason().unwrap().contains("无人值守"));
        for source in [
            json!("exec"),
            json!({"subagent":{"other":"guardian"}}),
            json!("sdk"),
        ] {
            let mut p = Parser::default();
            p.metadata(Agent::Codex,&json!({"type":"session_meta","payload":{"source":source,"originator":"codex-tui"}}));
            assert!(p.card.hidden_reason().is_some());
        }
        let head=br#"{"type":"session_meta","payload":{"id":"large","cwd":"/repo","source":"cli","originator":"codex-tui","base_instructions":{"text":"truncated"#;
        let mut p = Parser::default();
        p.observe_header(Agent::Codex, head);
        assert!(p.card.interactive);
        assert_eq!(
            header_value(Agent::Codex, head).unwrap()["payload"]["id"],
            "large"
        );
    }
    #[test]
    fn genuine_words_survive_wrappers_pastes_and_duplicate_injection() {
        assert_eq!(
            user_text(
                "请修复这个错误\n<system-reminder>ignore it</system-reminder>\n<command-name>/debug</command-name>\n<command-args>generated</command-args>\n```text\nlong pasted error\n```\n不要改接口"
            ),
            "请修复这个错误\n\n不要改接口"
        );
        assert_eq!(
            user_text(
                "# AGENTS.md instructions for /repo\n<INSTRUCTIONS>rules</INSTRUCTIONS>\n请修复登录"
            ),
            "请修复登录"
        );
        assert_eq!(
            user_text(
                "要查看键盘快捷键，按下问号\n一大段别人的帖子\n© 2026 X Corp.你看看这个是什么？"
            ),
            "你看看这个是什么？"
        );
        assert!(user_text(
            "Background (from the session that started this one):\nContext\nTask: inherited"
        )
        .is_empty());
        assert_eq!(
            user_text(
                "请读下面材料\n> quoted author words\n<skill>generated instruction body</skill>\n然后给我建议"
            ),
            "请读下面材料\n\n然后给我建议"
        );
        assert_eq!(
            user_text(
                "<pasted_content id=\"42\">long document</pasted_content id=\"42\">\n你看看这是什么"
            ),
            "你看看这是什么"
        );
        assert_eq!(
            user_text(
                "<in-app-browser-context>ambient</in-app-browser-context>\n## My request:\n对比界面"
            ),
            "对比界面"
        );
        assert_eq!(
            user_text(
                r#"<send_user_message_question_reply>[{"question":"generated prompt","answer":"允许本地修改"}]</send_user_message_question_reply>"#
            ),
            "允许本地修改"
        );
        assert_eq!(
            user_text("# Files mentioned by the user:\nfile.mp4\n## My request:\n剪掉结尾"),
            "剪掉结尾"
        );
        assert!(user_text("[Request interrupted by user for tool use]").is_empty());
        assert_eq!(
            user_text("* 我先选了错误的工具。\n* 所以我按错误方式改了。删掉这两个"),
            "删掉这两个"
        );
        let mut p = Parser::default();
        add(
            &mut p,
            json!({"type":"event_msg","payload":{"type":"user_message","message":"<system-reminder>injected</system-reminder>修复登录"}}),
        );
        add(
            &mut p,
            json!({"type":"event_msg","payload":{"type":"user_message","message":"修复登录"}}),
        );
        add(
            &mut p,
            json!({"type":"session_meta","payload":{"title":"injected title"}}),
        );
        assert_eq!(p.card.title, "修复登录");
        assert_eq!(p.card.asks.len(), 1);
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
    fn claude_short_completion_is_a_review_claim_not_evidence() {
        for reply in [
            "完成",
            "完成。",
            "完成!",
            "完成！",
            // R9 real Claude reply: the sentence goes on after the first full stop.
            "完成。approval-proof.txt 已写入相同字节：`ok`（0x6F 0x6B，无换行符）。",
            "说明如下：\n完成，两个文件都已写入。",
            // Real Claude reply (2026-10 local logs): another item is "尚未实现" in a later sentence.
            "完成。分支确认是 design/rewrite（Phase 5 可选项、尚未实现）。",
        ] {
            let mut p = Parser::default();
            p.add(
                Agent::Claude,
                &serde_json::to_vec(&json!({"type":"assistant","message":{"model":"claude","stop_reason":"end_turn","content":[{"type":"text","text":reply}]}})).unwrap(),
                Source::default(),
            );
            assert_eq!(p.card.turn, Turn::Completed);
            assert!(p.card.completed_claim, "{reply}");
            assert!(p.card.evidence.is_empty());
            assert!(p.card.here().done.contains("尚无"));
        }
        for text in [
            "完成后请运行测试",
            "完成吗？",
            "完成度 80%，还差测试",
            "我会先完成。然后再说",
            "尚未完成。",
            "> 完成。",
            "```\n完成。\n```",
            "完成，但尚未测试",
            "Tests are not completed yet.",
            "已完成一半，还没提交",
        ] {
            assert!(!claims_done(text), "{text}");
        }
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
        p.hook("PermissionRequest", &HookFacts::default(), 10, s.clone());
        assert_eq!(p.card.turn, Turn::Waiting);
        p.hook("Stop", &HookFacts::default(), 11, s.clone());
        assert_eq!(p.card.turn, Turn::Waiting, "Stop is not an approval answer");
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"new","message":{"content":"批准，请继续"}}"#.as_bytes(),
            s.clone(),
        );
        p.hook("Stop", &HookFacts::default(), 12, s);
        assert_eq!(p.card.turn, Turn::Completed);
    }
    #[test]
    fn hook_payload_facts_count_running_tasks_by_kind() {
        let facts = hook_facts(&json!({
            "hook_event_name": "Stop",
            "background_tasks": [
                {"id": "1", "type": "subagent", "status": "running"},
                {"id": "2", "type": "shell", "status": "running", "command": "cargo test"},
                {"id": "3", "type": "monitor"},
                {"id": "4", "type": "subagent", "status": "completed"},
                {"id": "5", "type": "workflow", "status": "pending"}
            ],
            "session_crons": [{"id": "c", "schedule": "*/5 * * * *"}],
            "last_assistant_message": "never forwarded"
        }));
        assert_eq!(
            facts.background,
            Some(keepline_core::content::Background {
                agents: 2,
                shells: 1,
                monitors: 1,
                crons: 1
            })
        );
        assert!(!facts.subagent);
        let empty = hook_facts(&json!({"hook_event_name": "Stop"}));
        assert_eq!(
            empty.background, None,
            "missing arrays are unknown, not idle"
        );
        let sub = hook_facts(&json!({"hook_event_name": "PreToolUse", "agent_id": "a1"}));
        assert!(sub.subagent);
        let n = hook_facts(&json!({"notification_type": "permission_prompt"}));
        assert_eq!(n.notification.as_deref(), Some("permission_prompt"));
    }
    /// Recorded payload shapes (structure only, see eval/protocol-contract/hook_shapes.py):
    /// every one parses, and the counts follow the task list it carried.
    #[test]
    fn recorded_hook_payload_shapes_parse_as_their_task_lists_say() {
        let corpus = include_str!("../tests/fixtures/hook_shapes.jsonl");
        let (mut claude_stop_with_agents, mut codex_stops) = (0, 0);
        for line in corpus.lines() {
            let shape: Value = serde_json::from_str(line).unwrap();
            let mut payload = serde_json::Map::new();
            for key in shape["keys"].as_array().unwrap() {
                payload.insert(key.as_str().unwrap().into(), Value::Null);
            }
            payload.insert("hook_event_name".into(), shape["event"].clone());
            let tasks: Vec<(String, String)> = shape["tasks"]
                .as_array()
                .map(|t| {
                    t.iter()
                        .map(|p| (p[0].as_str().unwrap().into(), p[1].as_str().unwrap().into()))
                        .collect()
                })
                .unwrap_or_default();
            if shape["tasks"].is_array() {
                payload.insert(
                    "background_tasks".into(),
                    tasks
                        .iter()
                        .map(|(t, s)| json!({"type": t, "status": s}))
                        .collect(),
                );
            }
            if let Some(n) = shape["crons"].as_u64() {
                payload.insert("session_crons".into(), (0..n).map(|_| json!({})).collect());
            }
            if let Some(n) = shape["notification"].as_str() {
                payload.insert("notification_type".into(), json!(n));
            }
            if shape["agent_id"] == true {
                payload.insert("agent_id".into(), json!("a"));
            }
            let facts = hook_facts(&Value::Object(payload));
            assert_eq!(facts.subagent, shape["agent_id"] == true, "{line}");
            assert_eq!(
                facts.notification.as_deref(),
                shape["notification"].as_str(),
                "{line}"
            );
            let listed = shape["tasks"].is_array() || shape["crons"].is_u64();
            assert_eq!(facts.background.is_some(), listed, "{line}");
            if let Some(b) = facts.background {
                let count = |kind: &str| tasks.iter().filter(|(t, _)| t == kind).count() as u32;
                assert_eq!(b.shells, count("shell"), "{line}");
                assert_eq!(b.monitors, count("monitor"), "{line}");
                assert_eq!(
                    b.agents,
                    tasks.len() as u32 - count("shell") - count("monitor"),
                    "{line}"
                );
                if shape["runtime"] == "claude" && shape["event"] == "Stop" && b.agents > 0 {
                    claude_stop_with_agents += 1;
                }
            }
            if shape["runtime"] == "codex" && shape["event"] == "Stop" {
                codex_stops += 1;
                assert_eq!(
                    facts.background, None,
                    "Codex 0.163 Stop lists no tasks: {line}"
                );
            }
        }
        assert!(
            claude_stop_with_agents > 0 && codex_stops > 0,
            "corpus lost its key shapes"
        );
    }
    fn facts(agents: u32) -> HookFacts {
        HookFacts {
            background: Some(keepline_core::content::Background {
                agents,
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    /// Replays one hook sequence and checks every turn change it caused (status trace).
    #[test]
    fn hook_sequence_replays_to_the_expected_status_trace() {
        let mut p = Parser::default();
        let s = Source::default();
        let none = HookFacts::default();
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"u1","message":{"content":"跑完测试"}}"#.as_bytes(),
            s.clone(),
        );
        // A stop with a running subagent: the turn is over but its work is not.
        p.hook("Stop", &facts(1), 1, s.clone());
        assert!(p.card.background.unwrap().keeps_working());
        // A subagent's own events never move the parent turn.
        let sub = HookFacts {
            subagent: true,
            ..Default::default()
        };
        p.hook("PermissionRequest", &sub, 2, s.clone());
        assert_eq!(p.card.turn, Turn::Completed);
        // The subagent woke the parent: a tool runs without a prompt.
        p.hook("PreToolUse", &none, 3, s.clone());
        assert_eq!(p.card.turn, Turn::Running);
        assert_eq!(p.card.background, None, "a new turn forgets the old tasks");
        // One dialog, reported by two hooks, is one waiting episode.
        p.hook("PermissionRequest", &none, 4, s.clone());
        p.hook(
            "Notification",
            &HookFacts {
                notification: Some("permission_prompt".into()),
                ..Default::default()
            },
            10,
            s.clone(),
        );
        assert_eq!((p.card.turn, p.card.waiting_episode), (Turn::Waiting, 1));
        p.hook("PostToolUse", &none, 11, s.clone());
        assert_eq!(p.card.turn, Turn::Running, "the approved tool finished");
        p.hook("Stop", &facts(0), 12, s.clone());
        // Stragglers after a stop with nothing running change nothing.
        p.hook("PostToolUse", &none, 13, s.clone());
        p.hook(
            "Notification",
            &HookFacts {
                notification: Some("idle_prompt".into()),
                ..Default::default()
            },
            14,
            s.clone(),
        );
        assert_eq!(p.card.turn, Turn::Completed);
        p.hook("SubagentStop", &facts(2), 15, s.clone());
        assert_eq!(
            p.card.turn,
            Turn::Completed,
            "SubagentStop only updates the count"
        );
        assert_eq!(p.card.background.unwrap().agents, 2);
        p.hook("StopFailure", &none, 16, s);
        assert_eq!(p.card.error, "API 错误中止了回合（Hook）");
        assert_eq!(
            p.transitions,
            [
                "Unknown→Running 日志",
                "Running→Completed Hook",
                "Completed→Running Hook",
                "Running→Waiting Hook",
                "Waiting→Running Hook",
                "Running→Completed Hook",
                "Completed→Error Hook",
            ]
        );
    }
    #[test]
    fn codex_subagent_header_names_its_parent_and_children_drive_the_parent() {
        let s = Source::default();
        let mut child = Parser::default();
        child.add(
            Agent::Codex,
            br#"{"type":"session_meta","payload":{"id":"kid","cwd":"/p","source":{"subagent":{"thread_spawn":{"parent_thread_id":"dad","depth":1,"agent_path":"/root/fix","agent_nickname":"Hypatia"}}},"agent_nickname":"Hypatia","parent_thread_id":"dad"}}"#,
            s.clone(),
        );
        assert_eq!(
            (child.card.parent.as_str(), child.card.agent_name.as_str()),
            ("dad", "Hypatia")
        );
        assert_eq!(child.card.hidden_reason, "Codex 子代理");
        let mut p = Parser::default();
        p.add(
            Agent::Codex,
            br#"{"timestamp":1,"type":"event_msg","payload":{"type":"task_started","turn_id":"t1"}}"#,
            s.clone(),
        );
        p.add(
            Agent::Codex,
            r#"{"timestamp":2,"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"已派出子任务"}}"#.as_bytes(),
            s.clone(),
        );
        p.children(
            &Children {
                running: 1,
                waiting: None,
            },
            3,
            s.clone(),
        );
        assert_eq!(p.card.turn, Turn::Completed);
        assert!(p.card.background.unwrap().keeps_working());
        // The subagent's approval is the parent's; once answered the parent is as before.
        let ask = Children {
            running: 1,
            waiting: Some("子 Agent Hypatia：是否运行迁移？".into()),
        };
        p.children(&ask, 4, s.clone());
        assert_eq!(
            (p.card.turn, p.card.signal.as_str()),
            (Turn::Waiting, "子 Agent")
        );
        p.children(&ask, 5, s.clone());
        assert_eq!(p.card.waiting_episode, 1, "the same dialog is one episode");
        p.children(
            &Children {
                running: 0,
                waiting: None,
            },
            6,
            s.clone(),
        );
        assert_eq!(p.card.turn, Turn::Completed);
        assert!(!p.card.background.unwrap().keeps_working());
        // A parent without subagents is left alone: unknown stays unknown.
        let mut lone = Parser::default();
        lone.children(&Children::default(), 1, s);
        assert_eq!(lone.card.background, None);
    }
    #[test]
    fn official_status_only_settles_dialogs() {
        let s = Source::default();
        let none = HookFacts::default();
        let mut p = Parser::default();
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"u1","message":{"content":"部署"}}"#.as_bytes(),
            s.clone(),
        );
        // Busy and idle never start or end a turn on their own: they can lag the log.
        p.official(&Official::Idle, 1, s.clone());
        assert_eq!(p.card.turn, Turn::Running);
        p.hook("PermissionRequest", &none, 2, s.clone());
        p.official(&Official::Busy, 3, s.clone());
        assert_eq!(
            (p.card.turn, p.card.signal.as_str()),
            (Turn::Running, "官方状态")
        );
        assert!(p.card.waiting.is_empty());
        // Esc closed the dialog: no hook says so, the official state is idle.
        p.hook("PermissionRequest", &none, 4, s.clone());
        p.official(&Official::Idle, 5, s.clone());
        assert_eq!(p.card.turn, Turn::Interrupted);
        // A turn end arrived while the dialog was shown; once it is gone the turn is done.
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"u2","message":{"content":"再部署"}}"#.as_bytes(),
            s.clone(),
        );
        p.hook("PermissionRequest", &none, 6, s.clone());
        p.hook("Stop", &none, 7, s.clone());
        assert_eq!(p.card.turn, Turn::Waiting);
        p.official(&Official::Idle, 8, s.clone());
        assert_eq!(p.card.turn, Turn::Completed);
        // A dialog no hook reported.
        let episode = p.card.waiting_episode;
        p.official(&Official::Waiting("有对话框打开".into()), 9, s);
        assert_eq!(p.card.turn, Turn::Waiting);
        assert_eq!(p.card.waiting, "有对话框打开（Claude 官方状态）");
        assert_eq!(p.card.waiting_episode, episode + 1);
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
        assert!(user_text("<system-reminder>context</system-reminder>\n<cross-session-message from='peer'>转述</cross-session-message>").is_empty());
        assert!(user_text("<side-sessions-event>not user input</side-sessions-event>").is_empty());
        assert!(!claims_done(
            "转发这个计划：\n````\n```sh\necho hello\n```\n完成后回复已完成\n````"
        ));
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
