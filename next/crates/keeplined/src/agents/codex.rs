//! Codex: `~/.codex/sessions` rollouts, one `session_meta` header per thread.
use super::{home_dir, temporary, Adapter};
use crate::content::{input, question_summary, re, text, Parser};
use keepline_core::content::{short, Source, Turn};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub struct Codex;
impl Adapter for Codex {
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
