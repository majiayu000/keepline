//! Commands a plugin adds to a pane's right-click menu and to ⌘P (`actions.json`).
//!
//! An action is a command template run for one pane: `{cwd}`, `{session_id}`,
//! `{transcript}`, `{task}` and `{agent}` stand for that pane's directory, its Agent
//! session, transcript file, the tab's task and the Agent's name. Every value is shell-quoted
//! where it lands, so a title or path can never become part of the command.
use crate::agent::Loaded;
use crate::model::shell_quote;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The `actions.json` format's major version.
pub const API: u32 = 1;

const VARS: [&str; 5] = ["cwd", "session_id", "transcript", "task", "agent"];

/// How the command runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Run {
    /// In a new pane split off this one; the default.
    #[default]
    Split,
    /// In a new tab.
    Tab,
    /// Typed into this pane without Enter, for you to read and send.
    Paste,
}

/// When the action is offered for a pane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    #[default]
    Always,
    /// The pane runs an Agent session Keepline has bound.
    Agent,
    /// No Agent session in the pane: a plain shell.
    Shell,
    /// The pane's tab carries a task.
    Task,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub title: String,
    pub command: String,
    #[serde(default)]
    pub run: Run,
    #[serde(default)]
    pub when: When,
    /// The file it came from, for the settings page.
    #[serde(skip)]
    pub file: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    actions: Vec<Action>,
}

/// What a pane offers an action's template.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub cwd: String,
    pub session_id: Option<String>,
    pub transcript: Option<String>,
    pub task: Option<String>,
    pub agent: Option<String>,
}
impl Context {
    fn var(&self, name: &str) -> Option<&str> {
        match name {
            "cwd" => Some(&self.cwd),
            "session_id" => self.session_id.as_deref(),
            "transcript" => self.transcript.as_deref(),
            "task" => self.task.as_deref(),
            "agent" => self.agent.as_deref(),
            _ => None,
        }
    }
}

impl Action {
    /// Whether the action is offered for a pane with this context.
    pub fn offered(&self, ctx: &Context) -> bool {
        match self.when {
            When::Always => true,
            When::Agent => ctx.session_id.is_some(),
            When::Shell => ctx.session_id.is_none(),
            When::Task => ctx.task.is_some(),
        }
    }
    /// The command for one pane, each value shell-quoted; a value the pane lacks is an
    /// error naming it, never an empty argument.
    pub fn command(&self, ctx: &Context) -> Result<String, String> {
        let mut out = String::new();
        let mut rest = self.command.as_str();
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            match after.find('}') {
                Some(end) if VARS.contains(&&after[..end]) => {
                    let name = &after[..end];
                    let value = ctx
                        .var(name)
                        .ok_or_else(|| format!("这个画面没有 {{{name}}}"))?;
                    out.push_str(&shell_quote(value));
                    rest = &after[end + 1..];
                }
                _ => {
                    out.push('{');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// Reads one `actions.json`: every action, or why none of it loads.
fn read(file: &Path) -> Result<Vec<Action>, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let f: File = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if f.api != API {
        return Err(format!("动作格式是 api {}，这个版本只认 api {API}", f.api));
    }
    if f.actions.is_empty() {
        return Err("actions 至少写一条".into());
    }
    let mut actions = f.actions;
    for a in &mut actions {
        if !crate::plugin::valid_id(&a.id) {
            return Err(format!(
                "动作 id \"{}\" 只能用小写字母、数字和 -，不超过 40 个字",
                a.id
            ));
        }
        if a.title.trim().is_empty() || a.command.trim().is_empty() {
            return Err(format!("动作 {} 的 title 和 command 都要写", a.id));
        }
        // A brace that reads like a placeholder but names none is a typo, not text.
        for (i, _) in a.command.match_indices('{') {
            let tail = &a.command[i + 1..];
            if let Some(end) = tail.find('}') {
                let name = &tail[..end];
                let looks_like_var =
                    !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_');
                if looks_like_var && !VARS.contains(&name) {
                    return Err(format!(
                        "动作 {} 用了不认识的 {{{name}}}，能用的有 {}",
                        a.id,
                        VARS.map(|v| format!("{{{v}}}")).join("、")
                    ));
                }
            }
        }
        a.file = file.to_owned();
    }
    Ok(actions)
}

/// Each file in order; an id already taken by an earlier file is refused.
pub fn load(files: Vec<PathBuf>) -> (Vec<Action>, Vec<Loaded>) {
    let mut actions: Vec<Action> = Vec::new();
    let mut report = Vec::new();
    for file in files {
        let result = read(&file).and_then(|found| {
            if let Some(dup) = found.iter().find(|a| actions.iter().any(|b| b.id == a.id)) {
                return Err(format!("动作 id \"{}\" 和前面的插件重名", dup.id));
            }
            Ok(found)
        });
        report.push(Loaded {
            file: file.clone(),
            result: result
                .as_ref()
                .map(|found| {
                    found
                        .iter()
                        .map(|a| a.title.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                })
                .map_err(Clone::clone),
        });
        if let Ok(found) = result {
            actions.extend(found);
        }
    }
    (actions, report)
}

static ACTIONS: OnceLock<(Vec<Action>, Vec<Loaded>)> = OnceLock::new();

/// Loads the installed plugins' actions once (after `plugin::install`).
pub fn install() {
    ACTIONS.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Actions)));
}
/// Every loaded action, plugin by plugin.
pub fn all() -> &'static [Action] {
    ACTIONS.get().map_or(&[], |(a, _)| a.as_slice())
}
/// Each `actions.json` file's outcome.
pub fn report() -> &'static [Loaded] {
    ACTIONS.get().map_or(&[], |(_, r)| r.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn templates_quote_every_value_and_name_what_is_missing() {
        let a = Action {
            id: "open".into(),
            title: "t".into(),
            command: "codex exec --cd {cwd} \"看看 {task}\" {unknown}".into(),
            run: Run::Split,
            when: When::Always,
            file: PathBuf::new(),
        };
        let ctx = Context {
            cwd: "/tmp/my dir".into(),
            task: Some("修 bug; rm -rf /".into()),
            ..Context::default()
        };
        assert_eq!(
            a.command(&ctx).unwrap(),
            "codex exec --cd '/tmp/my dir' \"看看 '修 bug; rm -rf /'\" {unknown}"
        );
        let needs = Action {
            command: "claude --resume {session_id}".into(),
            ..a.clone()
        };
        assert_eq!(
            needs.command(&ctx).unwrap_err(),
            "这个画面没有 {session_id}"
        );
    }

    #[test]
    fn when_decides_which_panes_offer_it() {
        let a = |when| Action {
            id: "x".into(),
            title: "x".into(),
            command: "x".into(),
            run: Run::Paste,
            when,
            file: PathBuf::new(),
        };
        let shell = Context::default();
        let agent = Context {
            session_id: Some("s".into()),
            ..Context::default()
        };
        assert!(a(When::Always).offered(&shell));
        assert!(a(When::Shell).offered(&shell) && !a(When::Shell).offered(&agent));
        assert!(a(When::Agent).offered(&agent) && !a(When::Agent).offered(&shell));
        assert!(!a(When::Task).offered(&agent));
    }

    #[test]
    fn bad_files_are_reported_and_good_ones_load() {
        let d = std::env::temp_dir().join(format!("kl-actions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let good = write(
            &d,
            "a.json",
            r#"{"api":1,"actions":[{"id":"review","title":"审一下改动","command":"claude -p 'review' --cwd {cwd}","when":"agent"}]}"#,
        );
        let typo = write(
            &d,
            "b.json",
            r#"{"api":1,"actions":[{"id":"x","title":"x","command":"echo {sesion_id}"}]}"#,
        );
        let dup = write(
            &d,
            "c.json",
            r#"{"api":1,"actions":[{"id":"review","title":"又一个","command":"true"}]}"#,
        );
        let future = write(&d, "d.json", r#"{"api":2,"actions":[]}"#);
        let (actions, report) = load(vec![good, typo, dup, future]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].when, When::Agent);
        assert_eq!(report[0].result, Ok("审一下改动".into()));
        assert!(
            report[1]
                .result
                .as_ref()
                .unwrap_err()
                .contains("{sesion_id}")
        );
        assert!(report[2].result.as_ref().unwrap_err().contains("重名"));
        assert!(report[3].result.as_ref().unwrap_err().contains("api 2"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
