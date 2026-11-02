//! Claude Code: `~/.claude/projects` transcripts, every record naming its session.
use super::{home_dir, temporary, Adapter, Hint, OfficialAnswer, Pending};
use crate::content::{claims_done, text, user_text, Parser};
use keepline_core::content::{short, Source, Turn};
use keepline_core::model::AgentSession;
use keepline_core::protocol::{TerminalNote, CLAUDE_TRANSCRIPT_OFF};
use keepline_core::quota::{Quota, Window};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Claude;
impl Adapter for Claude {
    /// A Braille (up to 2.1.227) or half-circle (2.1.228 on) spinner opens the title while
    /// a turn runs; "✳" stands there between turns.
    fn title_busy(&self, title: &str) -> Option<bool> {
        let mut chars = title.trim_start().chars();
        let glyph = chars.next()?;
        if !chars.next().is_some_and(char::is_whitespace) {
            return None;
        }
        match glyph {
            '\u{2800}'..='\u{28ff}' | '◐' | '◓' | '◑' | '◒' => Some(true),
            '✳' => Some(false),
            _ => None,
        }
    }
    /// Above the input box while a turn runs: "· Seasoning… (53s · ↓ 2.0k tokens)", its
    /// glyph cycling. A finished turn leaves "✻ Worked for 16m 1s", with no ellipsis.
    fn screen_busy(&self, tail: &str) -> Option<bool> {
        Some(super::bottom_lines(tail, 12).any(|l| {
            let mut chars = l.chars();
            let spinner = chars.next().is_some_and(|c| "·✢✳✶✻✽*".contains(c))
                && chars.next() == Some(' ')
                && chars.as_str().split_once('…').is_some_and(|(verb, rest)| {
                    !verb.is_empty() && rest.trim_start().starts_with('(')
                });
            spinner || l.contains("esc to interrupt")
        }))
    }
    fn screen_rules(&self) -> &'static str {
        include_str!("screen/claude.json")
    }
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
    fn launch_flags(&self, args: &[String]) -> Vec<String> {
        super::carried(
            args,
            "claude",
            &[
                "--dangerously-skip-permissions",
                "--allow-dangerously-skip-permissions",
                "--strict-mcp-config",
                "--chrome",
                "--no-chrome",
                "--ide",
                "--verbose",
                "--bare",
                "--safe-mode",
                "--restricted",
                "--disable-slash-commands",
                "--brief",
            ],
            &[
                "--model",
                "--permission-mode",
                "--effort",
                "--fallback-model",
                "--agent",
                "--agents",
                "--settings",
                "--setting-sources",
                "--append-system-prompt",
                "--system-prompt",
                "--plugin-dir",
                "--autocompact",
            ],
            &[
                "--add-dir",
                "--mcp-config",
                "--allowedTools",
                "--allowed-tools",
                "--disallowedTools",
                "--disallowed-tools",
                "--tools",
                "--betas",
            ],
        )
    }
    /// A hand-started Claude names no session and closes its transcript between writes;
    /// its hooks are the only evidence before the first prompt reaches the screen.
    fn hooks(&self, home: &Path) -> Option<super::Hooks> {
        Some(super::Hooks {
            settings: home_dir(home, "CLAUDE_CONFIG_DIR", ".claude").join("settings.json"),
            events: &[
                "SessionStart",
                "UserPromptSubmit",
                "Notification",
                "PermissionRequest",
                "Stop",
                "SessionEnd",
            ],
        })
    }
    /// Claude Code hands its status line the account's windows (`rate_limits`), and
    /// Keepline's status-line wrapper keeps the last payload of each conversation in
    /// `$KEEPLINE_HOME/context/claude-code/<id>.json` (`~/.keepline` by default).
    fn account(&self, _transcript: &Path, id: &str) -> Option<Quota> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        let home = std::env::var_os("KEEPLINE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".keepline")))?;
        let snapshot = std::fs::read(home.join("context/claude-code").join(format!("{id}.json")));
        statusline_quota(&serde_json::from_slice(&snapshot.ok()?).ok()?)
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
        // `/rename` and the desktop app write the conversation's title; the last one wins.
        // The CLI's own generated title only names a conversation nobody titled.
        if m["type"] == "custom-title" {
            if let Some(name) = m["customTitle"].as_str() {
                parser.card.name = name.trim().to_owned();
                parser.custom_name = true;
            }
        } else if m["type"] == "ai-title" && !parser.custom_name {
            if let Some(name) = m["aiTitle"].as_str() {
                parser.card.name = name.trim().to_owned();
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
        parser.inherited = inherited(v, source);
        let msg = &v["message"];
        let blocks = &msg["content"];
        // The summary a compaction writes is shaped like a prompt but asks nothing:
        // a conversation compacted after its last reply has still finished.
        if ty == "user" && v["isMeta"] != true && v["isCompactSummary"] != true {
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
    /// Claude Code writes no transcript when it believes it is a nested child session, so
    /// nothing can ever bind it.
    fn pending(
        &self,
        process: &crate::recognition::Process,
        processes: &[crate::recognition::Process],
    ) -> Pending {
        if !crate::recognition::claude_transcript_off(process.pid) {
            return super::unclaimed(process, processes);
        }
        Pending::Unrecorded(Hint {
            text: CLAUDE_TRANSCRIPT_OFF.into(),
            note: TerminalNote {
                note: "claude · 没有保存记录".into(),
                why: "这个 Claude 继承了 CLAUDE_CODE_CHILD_SESSION，自认为是别的会话的子会话，不写对话记录，Keepline 读不到也统计不了。关掉这个终端重新打开，再运行 claude。".into(),
            },
        })
    }
    fn official(&self, due: &[&(AgentSession, Option<u32>)]) -> Option<OfficialAnswer> {
        Some(("claude agents --json", claude_status(due)))
    }
    /// A Claude started without an id on its command line keeps no transcript open, so
    /// neither argv nor lsof names it. `claude agents --json` lists each live session with
    /// its pid: exact evidence. A failed query names nothing.
    fn sessions_by_pid(&self, pids: &[u32]) -> Vec<(u32, String)> {
        let Some(program) = std::env::var("KEEPLINE_CLAUDE_AGENTS").ok().or_else(|| {
            pids.iter()
                .find_map(|&p| crate::official::executable(p).ok())
        }) else {
            return vec![];
        };
        let Ok(entries) = crate::official::query(&program) else {
            return vec![];
        };
        pids.iter()
            .filter_map(|&pid| {
                let mut named = entries.iter().filter(|e| e.pid == Some(pid));
                match (named.next(), named.next()) {
                    (Some(entry), None) => Some((pid, entry.session_id.clone())),
                    _ => None,
                }
            })
            .collect()
    }
}

fn claude_status(
    due: &[&(AgentSession, Option<u32>)],
) -> std::io::Result<Vec<(usize, crate::content::Official)>> {
    let program = match std::env::var("KEEPLINE_CLAUDE_AGENTS") {
        Ok(program) => program,
        Err(_) => due
            .iter()
            .find_map(|(_, pid)| pid.and_then(|p| crate::official::executable(p).ok()))
            .ok_or_else(|| std::io::Error::other("no live Claude process to ask"))?,
    };
    let entries = crate::official::query(&program)?;
    Ok(due
        .iter()
        .enumerate()
        .filter_map(|(i, (session, pid))| {
            let entry = entries.iter().find(|e| {
                e.session_id == session.cli_session_id || (pid.is_some() && e.pid == *pid)
            })?;
            Some((i, entry.status.clone()))
        })
        .collect())
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

/// A forked or branched conversation starts with the original's records copied in, each
/// still naming the original `sessionId`; the transcript itself is named by its own id.
fn inherited(v: &Value, source: &Source) -> bool {
    let own = Path::new(&source.path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| s.len() == 36 && s.matches('-').count() == 4);
    match (own, v["sessionId"].as_str()) {
        (Some(own), Some(id)) => own != id,
        _ => false,
    }
}
/// The status line's `rate_limits` as the wrapper saved them: `five_hour` and `seven_day`,
/// each `used_percentage` and `resets_at` (Unix seconds). API-key and Bedrock sign-ins get
/// null there and leave no reading.
fn statusline_quota(v: &Value) -> Option<Quota> {
    let at = chrono::DateTime::parse_from_rfc3339(v["at"].as_str()?).ok()?;
    let windows: Vec<Window> = [("five_hour", 300), ("seven_day", 10_080)]
        .into_iter()
        .filter_map(|(key, minutes)| {
            let w = &v["rateLimits"][key];
            Some(Window {
                minutes,
                used: w["used_percentage"].as_f64()?.round().clamp(0., 999.) as u16,
                resets_at: w["resets_at"].as_f64().map_or(0, |t| t.max(0.) as u64),
            })
        })
        .collect();
    (!windows.is_empty()).then(|| Quota {
        at: u64::try_from(at.timestamp()).unwrap_or(0),
        plan: String::new(),
        windows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_rate_limits_become_the_account_reading() {
        let v = serde_json::json!({
            "sessionId": "abc",
            "at": "2026-11-02T07:47:28.543272+00:00",
            "contextWindow": null,
            "rateLimits": {
                "five_hour": {"used_percentage": 23.6, "resets_at": 1_798_000_000},
                "seven_day": {"used_percentage": 41, "resets_at": 1_798_400_000},
                "seven_day_opus": null
            }
        });
        let q = statusline_quota(&v).unwrap();
        assert_eq!(q.at, 1_793_605_648);
        assert_eq!(
            q.windows,
            [
                Window {
                    minutes: 300,
                    used: 24,
                    resets_at: 1_798_000_000
                },
                Window {
                    minutes: 10_080,
                    used: 41,
                    resets_at: 1_798_400_000
                },
            ]
        );
        // Snapshots from before the wrapper kept them, and API-key sign-ins.
        for rate_limits in [Value::Null, serde_json::json!({"five_hour": null})] {
            let mut old = v.clone();
            old["rateLimits"] = rate_limits;
            assert_eq!(statusline_quota(&old), None);
        }
        assert_eq!(
            Claude.account(Path::new("/x.jsonl"), "../../etc/passwd"),
            None
        );
    }

    #[test]
    fn the_title_glyph_says_whether_a_turn_runs() {
        assert_eq!(Claude.title_busy("◐ 修复滚动卡顿"), Some(true));
        assert_eq!(Claude.title_busy("⠂ 修复滚动卡顿"), Some(true));
        assert_eq!(Claude.title_busy("✳ 修复滚动卡顿"), Some(false));
        // A title the user or a shell set says nothing about the turn.
        assert_eq!(Claude.title_busy("◐修复"), None);
        assert_eq!(Claude.title_busy("vim main.rs"), None);
        assert_eq!(Claude.title_busy(""), None);
    }
    #[test]
    fn the_ticking_line_above_the_input_box_is_a_running_turn() {
        let screen = |line: &str| {
            format!("● Bash(cargo test)\n  ⎿ ok\n\n{line}\n\n────\n❯ \n────\n  ⏵⏵ auto mode on\n")
        };
        for running in [
            "· Seasoning… (53s · ↓ 2.0k tokens)",
            "✻ Percolating… (19s · ↓ 1.3k tokens · thinking)",
            "✳ Running PreCompact hooks… (running PreCompact hook · 1s)",
        ] {
            assert_eq!(
                Claude.screen_busy(&screen(running)),
                Some(true),
                "{running}"
            );
        }
        for done in [
            "✻ Worked for 16m 1s · done 2:10 AM",
            "❯ 这个是怎么回事… (看一下)",
            "",
        ] {
            assert_eq!(Claude.screen_busy(&screen(done)), Some(false), "{done}");
        }
    }
}
