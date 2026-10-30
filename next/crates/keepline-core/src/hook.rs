//! What runs when a session changes state (`hooks.json`): a shell command or a webhook,
//! handed the event as JSON. Hooks only hear about sessions; they cannot change one, and a
//! slow or failing hook never holds up the host: each runs off its thread under a timeout.
use crate::agent::Loaded;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The `hooks.json` format's major version.
pub const API: u32 = 1;
/// The longest a hook may take before it is stopped.
pub const MAX_TIMEOUT_MS: u64 = 30_000;

/// A session entering a state, once per turn (and once per dialog for `waiting`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Event {
    /// A turn began.
    Started,
    /// The Agent asks you something.
    Waiting,
    Error,
    /// The turn finished and is yours to check.
    Done,
}
impl Event {
    pub fn name(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Waiting => "waiting",
            Self::Error => "error",
            Self::Done => "done",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    pub id: String,
    pub on: Vec<Event>,
    /// Run with `sh -c`; the event arrives as JSON on stdin and as `KEEPLINE_*` variables.
    #[serde(default)]
    pub command: Option<String>,
    /// POSTed the event as JSON.
    #[serde(default)]
    pub webhook: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(skip)]
    pub file: PathBuf,
}
fn default_timeout() -> u64 {
    5_000
}

/// What a hook is told. Only the session's card: never the transcript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Payload {
    pub event: Event,
    pub session: String,
    pub agent: String,
    pub title: String,
    pub cwd: String,
    pub space: Option<String>,
    /// The question, the error, or the last reply's first line.
    pub detail: String,
    pub at: u64,
}
impl Payload {
    /// The same facts as environment variables, for one-line shell hooks.
    pub fn env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("KEEPLINE_EVENT", self.event.name().into()),
            ("KEEPLINE_SESSION", self.session.clone()),
            ("KEEPLINE_AGENT", self.agent.clone()),
            ("KEEPLINE_TITLE", self.title.clone()),
            ("KEEPLINE_CWD", self.cwd.clone()),
            ("KEEPLINE_SPACE", self.space.clone().unwrap_or_default()),
            ("KEEPLINE_DETAIL", self.detail.clone()),
        ]
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    api: u32,
    hooks: Vec<Hook>,
}

fn read(file: &Path, taken: &[Hook]) -> Result<Vec<Hook>, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let f: File = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if f.api != API {
        return Err(format!(
            "hooks 格式是 api {}，这个版本只认 api {API}",
            f.api
        ));
    }
    if f.hooks.is_empty() {
        return Err("hooks 至少写一条".into());
    }
    let mut hooks = f.hooks;
    for i in 0..hooks.len() {
        let (before, rest) = hooks.split_at_mut(i);
        let h = &mut rest[0];
        if !crate::plugin::valid_id(&h.id) {
            return Err(format!(
                "hook id \"{}\" 只能用小写字母、数字和 -，不超过 40 个字",
                h.id
            ));
        }
        if taken.iter().chain(before.iter()).any(|t| t.id == h.id) {
            return Err(format!("hook id \"{}\" 和前面的插件重名", h.id));
        }
        if h.on.is_empty() {
            return Err(format!("hook {} 的 on 至少写一个事件", h.id));
        }
        match (&h.command, &h.webhook) {
            (Some(c), None) if !c.trim().is_empty() => {}
            (None, Some(u)) if u.starts_with("https://") || u.starts_with("http://") => {}
            (None, Some(_)) => {
                return Err(format!("hook {} 的 webhook 要以 http(s):// 开头", h.id));
            }
            _ => return Err(format!("hook {} 要写 command 或 webhook 其中一个", h.id)),
        }
        if !(100..=MAX_TIMEOUT_MS).contains(&h.timeout_ms) {
            return Err(format!(
                "hook {} 的 timeout_ms 要在 100 到 {MAX_TIMEOUT_MS} 之间",
                h.id
            ));
        }
        h.file = file.to_owned();
    }
    Ok(hooks)
}

pub fn load(files: Vec<PathBuf>) -> (Vec<Hook>, Vec<Loaded>) {
    let mut hooks: Vec<Hook> = Vec::new();
    let mut report = Vec::new();
    for file in files {
        let result = read(&file, &hooks);
        report.push(Loaded {
            file: file.clone(),
            result: result
                .as_ref()
                .map(|found| {
                    found
                        .iter()
                        .map(|h| {
                            let on: Vec<_> = h.on.iter().map(|e| e.name()).collect();
                            format!("{}（{}）", h.id, on.join("/"))
                        })
                        .collect::<Vec<_>>()
                        .join("、")
                })
                .map_err(Clone::clone),
        });
        if let Ok(found) = result {
            hooks.extend(found);
        }
    }
    (hooks, report)
}

static HOOKS: OnceLock<(Vec<Hook>, Vec<Loaded>)> = OnceLock::new();

/// Loads the installed plugins' hooks once (after `plugin::install`).
pub fn install() {
    HOOKS.get_or_init(|| load(crate::plugin::files(crate::plugin::Kind::Hooks)));
}
pub fn all() -> &'static [Hook] {
    HOOKS.get().map_or(&[], |(h, _)| h.as_slice())
}
pub fn report() -> &'static [Loaded] {
    HOOKS.get().map_or(&[], |(_, r)| r.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_need_one_target_a_known_event_and_a_sane_timeout() {
        let d = std::env::temp_dir().join(format!("kl-hooks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let write = |name: &str, body: &str| {
            let p = d.join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        let good = write(
            "a.json",
            r#"{"api":1,"hooks":[{"id":"say","on":["waiting","error"],"command":"say \"$KEEPLINE_TITLE\""},{"id":"slack","on":["done"],"webhook":"https://hooks.example.com/x","timeout_ms":2000}]}"#,
        );
        let both = write(
            "b.json",
            r#"{"api":1,"hooks":[{"id":"x","on":["done"],"command":"true","webhook":"https://a"}]}"#,
        );
        let event = write(
            "c.json",
            r#"{"api":1,"hooks":[{"id":"y","on":["finished"],"command":"true"}]}"#,
        );
        let slow = write(
            "d.json",
            r#"{"api":1,"hooks":[{"id":"z","on":["done"],"command":"true","timeout_ms":600000}]}"#,
        );
        let dup = write(
            "e.json",
            r#"{"api":1,"hooks":[{"id":"say","on":["done"],"command":"true"}]}"#,
        );
        let (hooks, report) = load(vec![good, both, event, slow, dup]);
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].timeout_ms, 5_000);
        assert_eq!(
            report[0].result,
            Ok("say（waiting/error）、slack（done）".into())
        );
        assert!(report[1].result.as_ref().unwrap_err().contains("其中一个"));
        assert!(report[2].result.as_ref().unwrap_err().contains("格式不对"));
        assert!(
            report[3]
                .result
                .as_ref()
                .unwrap_err()
                .contains("timeout_ms")
        );
        assert!(report[4].result.as_ref().unwrap_err().contains("重名"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
