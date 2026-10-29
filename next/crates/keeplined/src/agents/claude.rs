//! Claude Code: `~/.claude/projects` transcripts, every record naming its session.
use super::{home_dir, temporary, Adapter};
use crate::content::{claims_done, text, user_text, Parser};
use keepline_core::content::{short, Source, Turn};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Claude;
impl Adapter for Claude {
    fn root(&self, home: &Path) -> PathBuf {
        home_dir(home, "CLAUDE_CONFIG_DIR", ".claude").join("projects")
    }
    fn runs(&self, executable: &str, script: Option<&str>) -> bool {
        executable == "claude"
            || script.is_some_and(|s| s.ends_with("/@anthropic-ai/claude-code/cli.js"))
    }
    fn process_session(&self, _pid: u32, args: &[String]) -> (Option<String>, bool) {
        (claude_session_id(args), false)
    }
    fn identity(&self, _path: &Path, value: &Value) -> Option<(String, String)> {
        if value["isSidechain"] == true {
            return Some((
                format!(
                    "{}:sidechain:{}",
                    value["sessionId"].as_str()?,
                    value["agentId"]
                        .as_str()
                        .or_else(|| value["uuid"].as_str())?
                ),
                value["cwd"].as_str()?.into(),
            ));
        }
        Some((
            value["sessionId"].as_str()?.into(),
            value["cwd"].as_str()?.into(),
        ))
    }
    fn metadata(&self, parser: &mut Parser, m: &Value) {
        if temporary(m["cwd"].as_str().unwrap_or("")) {
            parser.hide(keepline_core::content::TEMPORARY_DIRECTORY);
        } else {
            let entry = m["entrypoint"].as_str().unwrap_or("");
            let user_type = m["userType"].as_str().unwrap_or("");
            if m["isSidechain"] == true
                || m.get("agentId")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty())
            {
                parser.hide("Claude 子代理 / sidechain");
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
                parser.hide("Claude 非交互 / SDK");
            } else if !user_type.is_empty() && user_type != "external" {
                parser.hide("Claude 内部 / Hook 用户来源");
            } else if matches!(entry, "cli" | "claude-desktop" | "vscode")
                && user_type == "external"
            {
                parser.card.interactive = true;
            }
        }
        if m["type"] == "queue-operation" {
            parser.machine_prompt(&text(&m["content"]));
        } else if m["type"] == "user" {
            parser.machine_prompt(&text(&m["message"]["content"]));
        }
    }
    fn is_user(&self, v: &Value) -> bool {
        v["type"] == "user"
    }
    fn add(&self, parser: &mut Parser, v: &Value, at: u64, source: &Source) {
        let ty = v["type"].as_str().unwrap_or("");
        if v["isSidechain"] == true {
            return;
        }
        let msg = &v["message"];
        let blocks = &msg["content"];
        if ty == "user" && v["isMeta"] != true {
            let s = user_text(&text(blocks));
            if !s.is_empty() {
                parser.phase(Turn::Running, v["uuid"].as_str().unwrap_or(""), at, source);
                parser.message(true, s, at, source, false);
            }
        }
        if let Some(blocks) = blocks.as_array() {
            for b in blocks {
                match b["type"].as_str() {
                    Some("tool_use") => parser.tool(
                        b["id"].as_str().unwrap_or(""),
                        b["name"].as_str().unwrap_or(""),
                        b["input"].clone(),
                        at,
                        source,
                    ),
                    Some("tool_result") => parser.result(
                        b["tool_use_id"].as_str().unwrap_or(""),
                        &b["content"],
                        if b["is_error"] == true { Some(1) } else { None },
                        v["toolUseResult"].get("backgroundTaskId").is_none(),
                        at,
                        source,
                    ),
                    _ => (),
                }
            }
        }
        if v["isApiErrorMessage"] == true {
            parser.card.error = short(&text(blocks), 600);
            parser.phase(Turn::Error, "", at, source);
            return;
        }
        if ty == "assistant" && msg["model"] != "<synthetic>" {
            let final_answer = matches!(
                msg["stop_reason"].as_str(),
                Some("end_turn" | "stop_sequence")
            );
            parser.message(false, text(blocks), at, source, final_answer);
            if final_answer {
                parser.phase(Turn::Completed, "", at, source);
            }
        }
        if ty == "system" && matches!(v["subtype"].as_str(), Some("turn_duration" | "stop")) {
            parser.card.completed_claim = claims_done(&parser.card.last_reply);
            parser.phase(Turn::Completed, "", at, source);
        }
    }
}

pub(crate) fn claude_session_id(args: &[String]) -> Option<String> {
    let mut found = None;
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        let value = match arg.as_str() {
            "--session-id" | "--resume" | "-r" => args.next().map(String::as_str),
            _ => arg
                .strip_prefix("--session-id=")
                .or_else(|| arg.strip_prefix("--resume=")),
        };
        if let Some(value) = value {
            let id = uuid::Uuid::parse_str(value).ok()?.to_string();
            if found.is_some() {
                return None;
            } // Conflicting selectors aren't evidence.
            found = Some(id);
        }
    }
    found
}
