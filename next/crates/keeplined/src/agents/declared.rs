//! A CLI known from a declaration file rather than code (`keepline_core::agent`). It is
//! recognised by its executable or script; with `transcripts` its JSON-lines records bind
//! it to a terminal and fill its card with prompts and replies. Turn state, official status
//! and shared-daemon screen association need a coded adapter.
use super::{Adapter, Hint, Pending};
use crate::content::{text, Parser};
use keepline_core::agent::{AgentSpec, Transcripts};
use keepline_core::content::Source;
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
    fn root(&self, _home: &Path) -> PathBuf {
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
        };
        let Pending::Unrecorded(hint) = d.pending(&process, &[]) else {
            panic!("expected a note");
        };
        assert_eq!(hint.note.note, "fake-cli · 没有对话记录");
        assert!(hint.note.why.contains("fake.json"));
        assert!(d.identity(Path::new("x"), &json!({"sid":"s"})).is_none());
    }
}
