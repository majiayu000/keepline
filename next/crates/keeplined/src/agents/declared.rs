//! A CLI known from a declaration file rather than code (`keepline_core::agent`). It is
//! recognised by its executable or script; with `transcripts` its JSON-lines records bind
//! it to a terminal and fill its card with prompts and replies. Turn state, official status
//! and shared-daemon screen association need a coded adapter.
use super::{Adapter, Hint, Pending};
use crate::content::{text, Parser};
use keepline_core::agent::{AgentSpec, EventRule, ToolKind, Transcripts, TurnEvent};
use keepline_core::content::Source;
use keepline_core::content::Turn;
use keepline_core::model::Agent;
use keepline_core::protocol::TerminalNote;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Declared {
    agent: Agent,
    spec: &'static AgentSpec,
}

impl Declared {
    /// Only a registered CLI that came from a file.
    pub fn new(agent: Agent) -> Option<Self> {
        let spec = agent.spec().filter(|s| s.file.is_some())?;
        Some(Self { agent, spec })
    }
    pub fn agent(&self) -> Agent {
        self.agent
    }
    fn transcripts(&self) -> Option<&'static Transcripts> {
        self.spec.transcripts.as_ref()
    }
    fn role<'a>(&self, record: &'a Value) -> Option<&'a str> {
        record
            .pointer(self.transcripts()?.role.as_deref()?)?
            .as_str()
    }
    fn text(&self, record: &Value) -> String {
        self.transcripts()
            .and_then(|t| record.pointer(t.text.as_deref()?))
            .map(text)
            .unwrap_or_default()
    }
}

impl Adapter for Declared {
    fn root(&self, home: &Path) -> PathBuf {
        if self.spec.source.is_some() {
            return super::mirror::dir(home, self.agent);
        }
        self.transcripts()
            .map(|t| t.root.clone())
            .unwrap_or_default()
    }
    fn runs(&self, executable: &str, script: Option<&str>) -> bool {
        executable == self.spec.executable
            || script.is_some_and(|s| {
                Path::new(s).file_name().and_then(|n| n.to_str()) == Some(&self.spec.executable)
                    || self
                        .spec
                        .scripts
                        .iter()
                        .any(|suffix| s.ends_with(suffix.as_str()))
            })
    }
    fn process_session(&self, _pid: u32, _args: &[String]) -> (Option<String>, bool) {
        (None, false)
    }
    fn is_transcript(&self, path: &Path) -> bool {
        self.transcripts()
            .is_some_and(|t| path.extension().is_some_and(|e| *e == *t.extension))
    }
    fn identity(&self, _path: &Path, record: &Value) -> Option<(String, String)> {
        let t = self.transcripts()?;
        Some((
            record.pointer(&t.session)?.as_str()?.into(),
            record.pointer(&t.cwd)?.as_str()?.into(),
        ))
    }
    fn metadata(&self, parser: &mut Parser, record: &Value) {
        // Someone ran it in a terminal; a declaration cannot say otherwise.
        parser.card.interactive = true;
        if self.is_user(record) {
            parser.machine_prompt(&self.text(record));
        }
    }
    fn is_user(&self, record: &Value) -> bool {
        self.transcripts()
            .is_some_and(|t| self.role(record) == Some(t.user.as_str()))
    }
    fn add(&self, parser: &mut Parser, record: &Value, at: u64, source: &Source) {
        let Some(t) = self.transcripts() else {
            return;
        };
        // A record the declaration does not describe is skipped, never an error.
        match self.role(record) {
            Some(r) if r == t.user => parser.message(true, self.text(record), at, source, false),
            Some(r) if r == t.assistant => {
                parser.message(false, self.text(record), at, source, false)
            }
            _ => {}
        }
        apply_events(&t.events, parser, record, at, source);
        if let Some(name) = t
            .title
            .as_deref()
            .and_then(|p| record.pointer(p))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            if !parser.custom_name {
                parser.card.name = name.to_owned();
            }
        }
    }
    fn pending(
        &self,
        process: &crate::recognition::Process,
        processes: &[crate::recognition::Process],
    ) -> Pending {
        if self.transcripts().is_some() {
            return super::unclaimed(process, processes);
        }
        let name = &self.spec.name;
        Pending::Unrecorded(Hint {
            text: format!("{name} 不写对话记录，无法关联"),
            note: TerminalNote {
                note: format!("{} · 没有对话记录", self.spec.executable),
                why: format!(
                    "{name} 的声明（{}）没有写 transcripts，Keepline 认得出它在运行，但读不到对话，所以不能挂到待办或从菜单停止。",
                    self.spec.file.as_deref().unwrap_or(Path::new("")).display()
                ),
            },
        })
    }
}

/// An Agent read back from storage whose declaration is gone: nothing to recognise or
/// read, so its history stays as stored.
pub struct Missing;
impl Adapter for Missing {
    fn root(&self, _home: &Path) -> PathBuf {
        PathBuf::new()
    }
    fn runs(&self, _executable: &str, _script: Option<&str>) -> bool {
        false
    }
    fn process_session(&self, _pid: u32, _args: &[String]) -> (Option<String>, bool) {
        (None, false)
    }
    fn is_transcript(&self, _path: &Path) -> bool {
        false
    }
    fn identity(&self, _path: &Path, _record: &Value) -> Option<(String, String)> {
        None
    }
    fn metadata(&self, _parser: &mut Parser, _record: &Value) {}
    fn is_user(&self, _record: &Value) -> bool {
        false
    }
    fn add(&self, _parser: &mut Parser, _record: &Value, _at: u64, _source: &Source) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(transcripts: bool) -> &'static AgentSpec {
        let dir = std::env::temp_dir().join(format!(
            "keepline-declared-{}-{transcripts}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let t = r#","transcripts":{"root":"/logs","session":"/sid","cwd":"/cwd","role":"/role","text":"/content"}"#;
        std::fs::write(
            dir.join("fake.json"),
            format!(
                r#"{{"api":1,"id":"fake","name":"Fake","executable":"fake-cli","scripts":["/fake/cli.py"],"start":"fake-cli","resume":"fake-cli -r {{id}}"{}}}"#,
                if transcripts { t } else { "" }
            ),
        )
        .unwrap();
        let (specs, report) = keepline_core::agent::load(&dir, Path::new("/h"));
        assert!(report[0].result.is_ok(), "{report:?}");
        Box::leak(Box::new(specs.into_iter().last().unwrap()))
    }
    fn declared(transcripts: bool) -> Declared {
        Declared {
            agent: Agent::from_key("fake"),
            spec: spec(transcripts),
        }
    }

    #[test]
    fn recognised_by_executable_or_script() {
        let d = declared(true);
        assert!(d.runs("fake-cli", None));
        assert!(d.runs("python3", Some("/opt/fake/cli.py")));
        assert!(d.runs("node", Some("/usr/local/bin/fake-cli")));
        assert!(!d.runs("claude", None));
        assert!(!d.runs("python3", Some("/other.py")));
    }

    #[test]
    fn records_bind_and_fill_the_card() {
        let d = declared(true);
        assert_eq!(d.root(Path::new("/h")), Path::new("/logs"));
        let user = json!({"sid":"s1","cwd":"/repo","role":"user","content":"修一下登录"});
        assert_eq!(
            d.identity(Path::new("x"), &user),
            Some(("s1".into(), "/repo".into()))
        );
        assert!(d.is_user(&user));
        let mut p = Parser::default();
        let source = Source::default();
        d.metadata(&mut p, &user);
        d.add(&mut p, &user, 1, &source);
        d.add(
            &mut p,
            &json!({"role":"assistant","content":[{"text":"好了"}]}),
            2,
            &source,
        );
        d.add(
            &mut p,
            &json!({"role":"tool","content":"ignored"}),
            3,
            &source,
        );
        d.add(&mut p, &json!("not an object"), 4, &source);
        assert!(p.card.interactive);
        assert_eq!(p.card.title, "修一下登录");
        assert_eq!(p.card.last_reply, "好了");
    }

    #[test]
    fn without_transcripts_it_is_seen_but_never_bound() {
        let d = declared(false);
        assert!(d.root(Path::new("/h")).as_os_str().is_empty());
        let process = crate::recognition::Process {
            pid: 1,
            parent: 0,
            runtime: Some(d.agent()),
            cli_session_id: None,
            no_daemon: false,
            launch: vec![],
        };
        let Pending::Unrecorded(hint) = d.pending(&process, &[]) else {
            panic!("expected a note");
        };
        assert_eq!(hint.note.note, "fake-cli · 没有对话记录");
        assert!(hint.note.why.contains("fake.json"));
        assert!(d.identity(Path::new("x"), &json!({"sid":"s"})).is_none());
    }
}

/// api 2: what each matching rule says a record means, fed to the same parser calls the
/// built-in adapters make, so turn state and evidence work the same for a declared CLI.
pub(crate) fn apply_events(
    rules: &[EventRule],
    parser: &mut Parser,
    record: &Value,
    at: u64,
    source: &Source,
) {
    let str_at = |p: &Option<String>| {
        p.as_deref()
            .and_then(|p| record.pointer(p))
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
            .unwrap_or_default()
    };
    for rule in rules.iter().filter(|r| r.matches(record)) {
        if let Some(tool) = &rule.tool {
            let id = str_at(&Some(tool.id.clone()));
            // A declared kind speaks the built-ins' vocabulary, so the parser's evidence
            // rules (commands, tests, commits, edited files) apply unchanged.
            let (name, args) = match tool.kind {
                Some(ToolKind::Command) => (
                    "Bash".to_owned(),
                    serde_json::json!({ "command": str_at(&tool.command) }),
                ),
                Some(ToolKind::Edit) => (
                    "Edit".to_owned(),
                    serde_json::json!({ "file_path": str_at(&tool.path) }),
                ),
                None => (
                    str_at(&Some(tool.name.clone())),
                    tool.args
                        .as_deref()
                        .and_then(|p| record.pointer(p))
                        .cloned()
                        .unwrap_or(Value::Null),
                ),
            };
            parser.tool(&id, &name, args, at, source);
        }
        if let Some(result) = &rule.result {
            let id = str_at(&Some(result.id.clone()));
            let output = result
                .output
                .as_deref()
                .and_then(|p| record.pointer(p))
                .cloned()
                .unwrap_or(Value::Null);
            let code = result
                .exit_code
                .as_deref()
                .and_then(|p| record.pointer(p))
                .and_then(Value::as_i64)
                .map(|n| n as i32);
            // A result record means the call finished: no exit code reads as success,
            // as it does for the built-ins.
            parser.result(&id, &output, code, true, at, source);
        }
        if let Some(turn) = rule.turn {
            let turn = match turn {
                TurnEvent::Running => Turn::Running,
                TurnEvent::Waiting => Turn::Waiting,
                TurnEvent::Completed => Turn::Completed,
                TurnEvent::Interrupted => Turn::Interrupted,
                TurnEvent::Error => Turn::Error,
            };
            parser.phase(turn, &str_at(&rule.id), at, source);
            if turn == Turn::Waiting {
                let asked = str_at(&rule.waiting);
                parser.card.waiting = if asked.is_empty() {
                    "等你处理".into()
                } else {
                    asked
                };
            }
        }
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use serde_json::json;

    fn rules() -> Vec<EventRule> {
        serde_json::from_value(json!([
            {"when": {"/type": "turn_start"}, "turn": "running", "id": "/turn"},
            {"when": {"/type": "turn_end"}, "turn": "completed"},
            {"when": {"/type": "approval"}, "turn": "waiting", "waiting": "/message"},
            {"when": {"/type": "tool_call", "/tool": "run_terminal_cmd"},
             "tool": {"id": "/call", "name": "/tool", "kind": "command", "command": "/input/cmd"}},
            {"when": {"/type": "tool_call", "/tool": "edit_file"},
             "tool": {"id": "/call", "name": "/tool", "kind": "edit", "path": "/input/target"}},
            {"when": {"/type": "tool_result"},
             "result": {"id": "/call", "output": "/output", "exit_code": "/exit"}}
        ]))
        .unwrap()
    }

    fn feed(parser: &mut Parser, records: &[Value]) {
        for (i, r) in records.iter().enumerate() {
            let source = Source {
                offset: i as u64,
                ..Source::default()
            };
            apply_events(&rules(), parser, r, 100 + i as u64, &source);
        }
    }

    #[test]
    fn declared_events_give_turn_state_and_evidence_like_a_built_in() {
        let mut parser = Parser::default();
        feed(
            &mut parser,
            &[
                json!({"type": "turn_start", "turn": "t1"}),
                json!({"type": "tool_call", "tool": "run_terminal_cmd", "call": "c1",
                       "input": {"cmd": "cargo test -q"}}),
                json!({"type": "tool_result", "call": "c1", "exit": 0,
                       "output": "test result: ok. 3 passed; 0 failed"}),
                json!({"type": "tool_call", "tool": "edit_file", "call": "c2",
                       "input": {"target": "src/lib.rs"}}),
                json!({"type": "tool_result", "call": "c2", "exit": 0, "output": "ok"}),
                json!({"type": "something_else"}),
                json!({"type": "turn_end"}),
            ],
        );
        assert_eq!(parser.card.turn, Turn::Completed);
        let kinds: Vec<_> = parser
            .card
            .evidence
            .iter()
            .map(|e| e.kind.as_str())
            .collect();
        assert!(kinds.contains(&"command"), "{kinds:?}");
        assert!(
            kinds.contains(&"test"),
            "a test run is verification: {kinds:?}"
        );
        assert!(kinds.contains(&"file"), "{kinds:?}");
        assert_eq!(parser.card.turn_key, "t1");
    }

    #[test]
    fn a_question_puts_the_turn_in_waiting_with_what_it_asks() {
        let mut parser = Parser::default();
        feed(
            &mut parser,
            &[
                json!({"type": "turn_start", "turn": "t2"}),
                json!({"type": "approval", "message": "允许运行 rm -rf build 吗？"}),
            ],
        );
        assert_eq!(parser.card.turn, Turn::Waiting);
        assert_eq!(parser.card.waiting, "允许运行 rm -rf build 吗？");
    }

    #[test]
    fn a_failed_command_is_error_evidence_not_a_failed_turn() {
        let mut parser = Parser::default();
        feed(
            &mut parser,
            &[
                json!({"type": "turn_start", "turn": "t3"}),
                json!({"type": "tool_call", "tool": "run_terminal_cmd", "call": "c1",
                       "input": {"cmd": "grep nothing ."}}),
                json!({"type": "tool_result", "call": "c1", "exit": 1, "output": ""}),
            ],
        );
        assert_eq!(parser.card.turn, Turn::Running);
        assert!(parser
            .card
            .evidence
            .iter()
            .any(|e| e.kind == "error" && e.exit_code == Some(1)));
    }
}
