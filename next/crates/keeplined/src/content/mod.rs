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
mod db;
mod rules;
pub use db::{card, children, link_child, load, save, schema, Children};
use rules::*;
pub(crate) use rules::{claims_done, re, user_text};
// A session_meta line may exceed the 16 KiB head budget because of base_instructions.
// Decode only complete top-level metadata values in its prefix, never scan its body.
pub fn header_value(runtime: Agent, bytes: &[u8]) -> Option<Value> {
    crate::agents::adapter(runtime).header_value(bytes)
}
pub(crate) fn text(v: &Value) -> String {
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
pub(crate) fn input(v: &Value) -> Value {
    v.as_str()
        .map(|s| serde_json::from_str(s).unwrap_or_else(|_| json!({"input":s})))
        .unwrap_or_else(|| v.clone())
}
pub(crate) fn question_summary(v: &Value) -> String {
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Call {
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
    pub(crate) calls: BTreeMap<String, Call>,
    /// Reply text a CLI streams in chunks, put on the card once its segment ends.
    #[serde(default)]
    pub(crate) reply: String,
    executions: BTreeMap<String, String>,
    /// A turn end arrived while a dialog was still shown; settles the turn once the dialog
    /// is known to be gone.
    #[serde(default)]
    completed_while_waiting: bool,
    /// The turn a subagent's dialog interrupted, restored once that dialog is gone.
    #[serde(default)]
    child_wait_from: Option<Turn>,
    /// The card's name came from a title the user or the desktop app set, which a
    /// generated title does not replace.
    #[serde(default)]
    pub(crate) custom_name: bool,
    /// The record being read was copied from another conversation (a fork's history), so
    /// the changes it shows were made by that one.
    #[serde(skip)]
    pub(crate) inherited: bool,
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
    pub(crate) fn metadata(&mut self, runtime: Agent, v: &Value) {
        crate::agents::adapter(runtime).metadata(self, v);
    }
    /// The first reason a session is not the user's own conversation wins.
    pub(crate) fn hide(&mut self, reason: &str) {
        if self.card.hidden_reason.is_empty() {
            self.card.hidden_reason = reason.into();
        }
    }
    /// Some launchers inherit the desktop entrypoint. Require an explicit machine task marker.
    pub(crate) fn machine_prompt(&mut self, prompt: &str) {
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
    pub(crate) fn message(
        &mut self,
        user: bool,
        s: String,
        at: u64,
        source: &Source,
        final_answer: bool,
    ) {
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
    pub(crate) fn phase(&mut self, turn: Turn, id: &str, at: u64, source: &Source) {
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
            self.completed_while_waiting = false;
            self.child_wait_from = None;
        }
        let still_waiting = self.card.turn == Turn::Waiting
            && !self.card.waiting.is_empty()
            && turn == Turn::Completed;
        self.completed_while_waiting |= still_waiting;
        self.card.turn = if still_waiting { Turn::Waiting } else { turn };
        self.card.updated = self.card.updated.max(at);
        self.record("turn", format!("{turn:?}"), id, None, at, source);
    }
    pub(crate) fn tool(&mut self, id: &str, name: &str, args: Value, at: u64, source: &Source) {
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
    pub(crate) fn result(
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
            && matches!(
                call.name.as_str(),
                "Bash" | "Edit" | "MultiEdit" | "Write" | "NotebookEdit"
            )
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
            self.fact("approval", self.card.waiting.clone(), None, at, source);
            self.calls.remove(id);
            self.card.updated = self.card.updated.max(at);
            return;
        }
        // A failed command is evidence, not a failed turn: agents routinely run commands
        // that exit non-zero (no grep match, an interrupted call) and still answer.
        if code.is_some_and(|n| n != 0) {
            let failed = format!("{}: {}", call.name, short(&output, 240));
            self.fact("error", failed, code, at, source);
        }
        if code == Some(0)
            && (patch || matches!(name, "Edit" | "MultiEdit" | "Write" | "NotebookEdit"))
        {
            let mut paths = Vec::new();
            for key in ["file_path", "path", "notebook_path"] {
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
                if !self.inherited {
                    self.card.touch(&path, at);
                }
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
        let from = self.records.len();
        let record = self.add_line(runtime, line, source);
        let detail = match record {
            Some(v) if self.card.turn != before => {
                log_detail(&v, self.records.get(from..).unwrap_or_default())
            }
            _ => String::new(),
        };
        self.settle(before, "日志", self.card.updated, detail);
    }
    fn add_line(&mut self, runtime: Agent, line: &[u8], source: Source) -> Option<Value> {
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            self.card.unknown_records += 1;
            return None;
        };
        self.metadata(runtime, &v);
        let at = seconds(&v["timestamp"]);
        crate::agents::adapter(runtime).add(self, &v, at, &source);
        Some(v)
    }
    pub fn hook(&mut self, event: &str, facts: &HookFacts, at: u64, source: Source) {
        let before = self.card.turn;
        self.hook_event(event, facts, at, &source);
        let detail = match &facts.notification {
            _ if self.card.turn == before => String::new(),
            Some(n) if event == "Notification" => format!("事件：{event}（{n}）"),
            _ => format!("事件：{event}"),
        };
        self.settle(before, "Hook", at, detail);
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
        let detail = match status {
            _ if self.card.turn == before => String::new(),
            Official::Waiting(reason) => format!("报告：在等（{reason}）"),
            Official::Busy => "报告：在忙".into(),
            Official::Idle => "报告：空闲".into(),
        };
        self.settle(before, "官方状态", at, detail);
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
        self.settle(before, "子 Agent", at, String::new());
    }
    /// `detail` is the card's `signal_detail` when the turn changed, built only then.
    fn settle(&mut self, before: Turn, signal: &str, at: u64, detail: String) {
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
        self.card.signal_detail = detail;
        self.transitions
            .push(format!("{before:?}→{:?} {signal}", self.card.turn));
    }
}
/// The log record that changed the turn: its type (with the payload type or subtype that
/// refines it) and a snippet of what it said, from the newest record it produced.
fn log_detail(v: &Value, records: &[Record]) -> String {
    let ty = v["type"].as_str().unwrap_or("?");
    let kind = match v["payload"]["type"].as_str().or(v["subtype"].as_str()) {
        Some(sub) => format!("{ty}·{sub}"),
        None => ty.to_owned(),
    };
    let said = records
        .iter()
        .rev()
        .find(|r| r.kind != "turn" && !r.text.trim().is_empty());
    match said {
        Some(r) => format!(
            "记录：{kind}「{}」",
            short(&keepline_core::content::plain_line(&r.text), 60)
        ),
        None => format!("记录：{kind}"),
    }
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
    fn claude_rename_names_the_card_and_the_last_one_wins() {
        let mut p = Parser::default();
        for title in ["亮色主题", " MCP CLI 系统设计 "] {
            p.add(
                Agent::Claude,
                &serde_json::to_vec(
                    &json!({"type":"custom-title","customTitle":title,"sessionId":"s"}),
                )
                .unwrap(),
                Source::default(),
            );
        }
        assert_eq!(p.card.name, "MCP CLI 系统设计");
        assert!(p.card.title.is_empty());
    }
    #[test]
    fn claude_generated_title_names_the_card_until_one_is_set() {
        let mut p = Parser::default();
        let mut add_title = |record: serde_json::Value| {
            p.add(
                Agent::Claude,
                &serde_json::to_vec(&record).unwrap(),
                Source::default(),
            );
        };
        add_title(json!({"type":"ai-title","aiTitle":" 架构清理 ","sessionId":"s"}));
        add_title(json!({"type":"custom-title","customTitle":"我起的名","sessionId":"s"}));
        add_title(json!({"type":"ai-title","aiTitle":"后来生成的","sessionId":"s"}));
        assert_eq!(p.card.name, "我起的名");
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
        assert_eq!(p.card.turn, Turn::Completed);
        assert!(p
            .card
            .evidence
            .iter()
            .any(|e| e.kind == "error" && e.exit_code == Some(101)));
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
    fn the_signal_keeps_the_record_or_event_that_changed_the_turn() {
        let mut p = Parser::default();
        let s = Source::default();
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"u","message":{"content":"跑一下测试"}}"#.as_bytes(),
            s.clone(),
        );
        assert_eq!(p.card.signal_detail, "记录：user「跑一下测试」");
        let notification = HookFacts {
            notification: Some("permission_prompt".into()),
            ..Default::default()
        };
        p.hook("Notification", &notification, 10, s.clone());
        assert_eq!(p.card.signal, "Hook");
        assert_eq!(
            p.card.signal_detail,
            "事件：Notification（permission_prompt）"
        );
        // A record that changes nothing leaves the detail of the one that did.
        p.add(
            Agent::Claude,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"还在想"}]}}"#
                .as_bytes(),
            s.clone(),
        );
        assert_eq!(
            p.card.signal_detail,
            "事件：Notification（permission_prompt）"
        );
        p.hook("Stop", &HookFacts::default(), 11, s.clone());
        p.add(
            Agent::Claude,
            r#"{"type":"user","uuid":"v","message":{"content":"继续"}}"#.as_bytes(),
            s.clone(),
        );
        p.add(
            Agent::Claude,
            r#"{"type":"assistant","message":{"stop_reason":"end_turn","content":[{"type":"text","text":"**测试都过了**，已经提交。"}]}}"#.as_bytes(),
            s,
        );
        assert_eq!(p.card.signal, "日志");
        assert_eq!(
            p.card.signal_detail,
            "记录：assistant「测试都过了，已经提交。」"
        );
    }
    #[test]
    fn changed_files_are_listed_once_each_and_failed_edits_are_not() {
        let mut p = Parser::default();
        let s = Source::default();
        for v in [
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"e","name":"Edit","input":{"file_path":"/r/a.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"e","content":"Successfully updated"}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"m","name":"MultiEdit","input":{"file_path":"/r/b.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"m","content":"Applied 2 edits"}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"x","name":"Edit","input":{"file_path":"/r/c.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"x","content":"String not found","is_error":true}]}}),
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"again","name":"Write","input":{"file_path":"/r/a.rs"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"again","content":"File written"}]}}),
        ] {
            p.add(Agent::Claude, &serde_json::to_vec(&v).unwrap(), s.clone());
        }
        let paths: Vec<_> = p.card.files.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, ["/r/b.rs", "/r/a.rs"]);

        let mut p = Parser::default();
        call(
            &mut p,
            "p",
            "functions.apply_patch",
            json!({"input":"*** Begin Patch\n*** Update File: src/a.rs\n*** Add File: src/new.rs\n*** End Patch"}),
        );
        result(
            &mut p,
            "p",
            "Success. Updated the following files:\nM src/a.rs\nA src/new.rs",
        );
        let paths: Vec<_> = p.card.files.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, ["src/a.rs", "src/new.rs"]);

        // A fork's copied history names the original session; only its own edits count.
        let mut p = Parser::default();
        let s = Source {
            path: "/p/229e2e95-cdaa-4b6b-be4b-4933f72cb5e8.jsonl".into(),
            ..Default::default()
        };
        for (session, id, file) in [
            (
                "1fc5e267-bc3e-4c8b-b4f4-3ed3a2865c58",
                "old",
                "/r/copied.rs",
            ),
            ("229e2e95-cdaa-4b6b-be4b-4933f72cb5e8", "own", "/r/own.rs"),
        ] {
            for v in [
                json!({"sessionId":session,"type":"assistant","message":{"content":[{"type":"tool_use","id":id,"name":"Edit","input":{"file_path":file}}]}}),
                json!({"sessionId":session,"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":id,"content":"ok"}]}}),
            ] {
                p.add(Agent::Claude, &serde_json::to_vec(&v).unwrap(), s.clone());
            }
        }
        let paths: Vec<_> = p.card.files.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, ["/r/own.rs"]);
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
        let corpus = include_str!("../../tests/fixtures/hook_shapes.jsonl");
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
        assert_eq!(
            user_text("<image name=[Image #1] path=\"/var/folders/x/codex-clipboard-a.png\">\n</image>\n你帮我看看[Image #1] 这是哪个环境"),
            "你帮我看看[Image #1] 这是哪个环境"
        );
        assert_eq!(
            user_text("SVG 里的 <image href=\"a.png\"> 为什么不显示"),
            "SVG 里的 <image href=\"a.png\"> 为什么不显示"
        );
        assert_eq!(
            user_text("把 <image>logo</image> 换成 img 标签"),
            "把 <image>logo</image> 换成 img 标签"
        );
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
    fn verification_reads_chains_runners_and_scripts() {
        for yes in [
            "cargo test --lib",
            "RUST_LOG=1 cargo nextest run",
            "cd next && cargo test -p keeplined",
            "cargo fmt && cargo clippy -- -D warnings && echo done",
            "cargo build; cargo test",
            "cargo test 2>&1",
            "npx vitest run",
            "bunx jest src",
            "pnpm run test:unit",
            "npm test",
            "bun run typecheck",
            "make check",
            "uv run pytest -q",
            "python3 -m pytest tests",
            "./node_modules/.bin/vitest",
            "deno test",
            "swift test",
            "go vet ./...",
        ] {
            assert!(verification(yes), "{yes}");
        }
        for no in [
            "cargo build",
            "cargo test | tail -5",
            "cargo test 2>&1 | tail",
            "cargo test &",
            "cargo test; echo ok",
            "echo $(cargo test)",
            "npm run build",
            "make",
            "cargo test --version",
            "git commit -m test",
        ] {
            assert!(!verification(no), "{no}");
        }
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
