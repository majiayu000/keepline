//! Which CLIs Keepline knows, as data. The rest of Keepline asks here or the daemon's
//! `agents` adapters and never names a CLI itself (`tests/architecture_boundary.rs`).
//!
//! The built-in three are compiled in. Any other comes from a declaration in
//! `~/.config/keepline/agents/*.json`, loaded once per process by `install`; a later file
//! with the same id replaces an earlier one, and a file that cannot be used is reported,
//! never fatal. See docs/architecture/PLUGIN_SYSTEM.md.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A CLI's identity: the key stored in the database and sent over the protocol. The
/// built-in keys are the names the enum this replaced serialized as, so nothing stored
/// changes. A key no longer registered (its declaration was removed) still reads back,
/// with no adapter and its key as its name.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Agent(&'static str);

#[allow(non_upper_case_globals)]
impl Agent {
    pub const Codex: Agent = Agent("Codex");
    pub const Claude: Agent = Agent("Claude");
    pub const Grok: Agent = Agent("Grok");

    /// Every registered CLI, built-in first, in the order the app offers them.
    pub fn all() -> Vec<Agent> {
        registry().specs.iter().map(|s| Agent(s.key)).collect()
    }
    pub fn key(self) -> &'static str {
        self.0
    }
    /// The key as stored, registered or not.
    pub fn from_key(key: &str) -> Agent {
        Agent(intern(key))
    }
    pub fn spec(self) -> Option<&'static AgentSpec> {
        registry().specs.iter().find(|s| s.key == self.0)
    }
    pub fn name(self) -> &'static str {
        self.spec().map_or(self.0, |s| s.name.as_str())
    }
    pub fn executable(self) -> &'static str {
        self.spec().map_or(self.0, |s| s.executable.as_str())
    }
    pub fn from_executable(name: &str) -> Option<Self> {
        registry()
            .specs
            .iter()
            .find(|s| s.executable == name)
            .map(|s| Agent(s.key))
    }
    /// The command a new conversation starts with.
    pub fn command(self) -> String {
        render(self.spec().map_or(self.0, |s| s.start.as_str()), None)
    }
    pub fn resume_command(self, cli_id: &str) -> String {
        match self.spec() {
            Some(s) => render(&s.resume, Some(cli_id)),
            None => self.command(),
        }
    }
}
impl Default for Agent {
    fn default() -> Self {
        Agent::Codex
    }
}
impl std::fmt::Debug for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl Serialize for Agent {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}
impl<'de> Deserialize<'de> for Agent {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Agent::from_key(&String::deserialize(d)?))
    }
}

/// `{id}` is the conversation to resume, shell-quoted; `{uuid}` a fresh one for a CLI
/// that takes its id up front.
fn render(template: &str, id: Option<&str>) -> String {
    let mut out = template.replace("{uuid}", &uuid::Uuid::new_v4().to_string());
    if let Some(id) = id {
        out = out.replace("{id}", &crate::model::shell_quote(id));
    }
    out
}

/// Keys are few and live for the process, so an unknown one read back is leaked once.
fn intern(key: &str) -> &'static str {
    if let Some(s) = registry().specs.iter().find(|s| s.key == key) {
        return s.key;
    }
    static UNKNOWN: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut unknown = UNKNOWN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match unknown.get(key) {
        Some(k) => k,
        None => {
            let k: &'static str = Box::leak(key.to_owned().into_boxed_str());
            unknown.insert(k);
            k
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSpec {
    pub key: &'static str,
    pub name: String,
    pub executable: String,
    /// Path suffixes of the script an interpreter (`node`, `bun`, `python*`) runs.
    pub scripts: Vec<String>,
    pub start: String,
    pub resume: String,
    /// Built-ins read their own transcripts in code; a declaration says where they are.
    pub transcripts: Option<Transcripts>,
    /// The declaration it came from; `None` for a built-in.
    pub file: Option<PathBuf>,
}

/// Where a declared CLI writes JSON-lines transcripts and how a record names things.
/// Pointers are RFC 6901 JSON pointers into each record.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transcripts {
    /// Absolute, or under the home with `~/`.
    pub root: PathBuf,
    #[serde(default = "jsonl")]
    pub extension: String,
    pub session: String,
    pub cwd: String,
    /// Which record is whose: `role` points at a value equal to `user` or `assistant`.
    pub role: Option<String>,
    #[serde(default = "user")]
    pub user: String,
    #[serde(default = "assistant")]
    pub assistant: String,
    /// The message text of a user or assistant record.
    pub text: Option<String>,
}
fn jsonl() -> String {
    "jsonl".into()
}
fn user() -> String {
    "user".into()
}
fn assistant() -> String {
    "assistant".into()
}

/// The declaration format's major version; a file of another is refused, not guessed at.
pub const API: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    api: u32,
    id: String,
    name: String,
    executable: String,
    #[serde(default)]
    scripts: Vec<String>,
    start: String,
    resume: String,
    transcripts: Option<Transcripts>,
}

/// One declaration file's outcome, for settings and diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub file: PathBuf,
    /// The key it registered, or why it did not.
    pub result: Result<String, String>,
}

pub struct Registry {
    specs: Vec<AgentSpec>,
    report: Vec<Loaded>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| Registry {
        specs: builtin(),
        report: vec![],
    })
}

/// Each declaration file's outcome since `install`; empty before it or without files.
pub fn report() -> &'static [Loaded] {
    &registry().report
}

fn builtin() -> Vec<AgentSpec> {
    let spec = |key, name: &str, executable: &str, start: &str, resume: &str| AgentSpec {
        key,
        name: name.into(),
        executable: executable.into(),
        scripts: vec![],
        start: start.into(),
        resume: resume.into(),
        transcripts: None,
        file: None,
    };
    vec![
        // Temporary until daemon-backed Codex can prove a TUI-to-session association.
        spec("Codex", "Codex", "codex", "codex", "codex resume {id}"),
        spec(
            "Claude",
            "Claude Code",
            "claude",
            "claude --session-id {uuid}",
            "claude --resume {id}",
        ),
        // The TUI cannot be given an id; the daemon reads it from Grok's own registry.
        spec("Grok", "Grok Build", "grok", "grok", "grok --resume {id}"),
    ]
}

/// `$XDG_CONFIG_HOME/keepline`, else `~/.config/keepline`.
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))
        .map(|base| base.join("keepline"))
}

/// Registers the declarations in `<config_dir>/agents`. Call once, before any agent is
/// looked up; a process that never calls it knows the built-ins only. Returns false
/// when the registry was already in use.
pub fn install() -> bool {
    let (Some(dir), Some(home)) = (config_dir(), std::env::var_os("HOME")) else {
        return false;
    };
    install_from(&dir.join("agents"), Path::new(&home))
}

pub fn install_from(dir: &Path, home: &Path) -> bool {
    let (specs, report) = load(dir, home);
    REGISTRY.set(Registry { specs, report }).is_ok()
}

/// Built-ins, then each `*.json` in `dir` by file name. A later file with an id already
/// declared replaces it, as a later patch row does; a built-in is never replaced.
pub fn load(dir: &Path, home: &Path) -> (Vec<AgentSpec>, Vec<Loaded>) {
    let mut specs = builtin();
    let mut report = Vec::new();
    let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect(),
        Err(_) => vec![],
    };
    files.sort();
    for file in files {
        let result = declare(&file, home, &specs);
        report.push(Loaded {
            file: file.clone(),
            result: result
                .as_ref()
                .map(|s| s.key.to_owned())
                .map_err(Clone::clone),
        });
        let Ok(spec) = result else { continue };
        if let Some(old) = specs.iter_mut().find(|s| s.key == spec.key) {
            for r in report.iter_mut() {
                if r.file.as_path() == old.file.as_deref().unwrap_or(Path::new(""))
                    && r.result.is_ok()
                {
                    r.result = Err(format!("被后面的 {} 取代", file.display()));
                }
            }
            *old = spec;
        } else {
            specs.push(spec);
        }
    }
    (specs, report)
}

fn declare(file: &Path, home: &Path, known: &[AgentSpec]) -> Result<AgentSpec, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("读不了：{e}"))?;
    let d: Declaration = serde_json::from_slice(&bytes).map_err(|e| format!("格式不对：{e}"))?;
    if d.api != API {
        return Err(format!("声明格式是 api {}，这个版本只认 api {API}", d.api));
    }
    let id_ok = !d.id.is_empty()
        && d.id.len() <= 40
        && d.id
            .starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && d.id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !id_ok {
        return Err(format!(
            "id \"{}\" 只能用小写字母、数字和 -，不超过 40 个字",
            d.id
        ));
    }
    let builtin = known.iter().filter(|s| s.file.is_none());
    if let Some(s) = builtin
        .clone()
        .find(|s| s.key.eq_ignore_ascii_case(&d.id) || s.executable == d.id)
    {
        return Err(format!("id \"{}\" 和内置的 {} 重名", d.id, s.name));
    }
    if d.executable.trim().is_empty() || d.executable.contains('/') {
        return Err("executable 要写可执行文件名，不带路径".into());
    }
    if let Some(s) = known
        .iter()
        .find(|s| s.executable == d.executable && s.key != d.id)
    {
        return Err(format!(
            "executable \"{}\" 已经是 {} 的，同一个程序认不出是谁",
            d.executable, s.name
        ));
    }
    if d.start.trim().is_empty() || d.resume.trim().is_empty() {
        return Err("start 和 resume 都要写命令".into());
    }
    let mut transcripts = d.transcripts;
    if let Some(t) = transcripts.as_mut() {
        if let Ok(rest) = t.root.strip_prefix("~") {
            t.root = home.join(rest);
        }
        if !t.root.is_absolute() {
            return Err("transcripts.root 要写绝对路径或 ~/ 开头".into());
        }
        let pointers = [
            Some(&t.session),
            Some(&t.cwd),
            t.role.as_ref(),
            t.text.as_ref(),
        ];
        if let Some(p) = pointers.into_iter().flatten().find(|p| !p.starts_with('/')) {
            return Err(format!(
                "transcripts 里的 \"{p}\" 要写成 / 开头的 JSON 指针"
            ));
        }
    }
    let name = if d.name.trim().is_empty() {
        d.id.clone()
    } else {
        d.name
    };
    Ok(AgentSpec {
        key: intern_declared(&d.id),
        name,
        executable: d.executable,
        scripts: d.scripts,
        start: d.start,
        resume: d.resume,
        transcripts,
        file: Some(file.to_owned()),
    })
}

/// Declared keys are leaked once at load; there are only as many as files.
fn intern_declared(id: &str) -> &'static str {
    Box::leak(id.to_owned().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "keepline-agents-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
    const AIDER: &str = r#"{"api":1,"id":"aider","name":"Aider","executable":"aider",
        "start":"aider","resume":"aider --restore {id}",
        "transcripts":{"root":"~/.aider/s","session":"/sid","cwd":"/cwd","role":"/role","text":"/content"}}"#;

    #[test]
    fn built_in_keys_and_commands_are_what_was_stored_before() {
        assert_eq!(serde_json::to_string(&Agent::Claude).unwrap(), "\"Claude\"");
        assert_eq!(
            serde_json::from_str::<Agent>("\"Grok\"").unwrap(),
            Agent::Grok
        );
        assert_eq!(format!("{:?}", Agent::Codex), "Codex");
        assert_eq!(Agent::Codex.command(), "codex");
        assert_eq!(Agent::Claude.resume_command("x"), "claude --resume 'x'");
        assert!(Agent::Claude.command().starts_with("claude --session-id "));
    }

    #[test]
    fn a_key_no_longer_registered_reads_back_as_itself() {
        let gone: Agent = serde_json::from_str("\"removed-cli\"").unwrap();
        assert_eq!(gone.key(), "removed-cli");
        assert_eq!(gone.name(), "removed-cli");
        assert!(gone.spec().is_none());
        assert_eq!(gone, Agent::from_key("removed-cli"));
    }

    #[test]
    fn a_declaration_registers_after_the_built_ins() {
        let d = dir("ok");
        std::fs::write(d.join("aider.json"), AIDER).unwrap();
        let (specs, report) = load(&d, Path::new("/home/u"));
        assert_eq!(
            specs.iter().map(|s| s.key).collect::<Vec<_>>(),
            ["Codex", "Claude", "Grok", "aider"]
        );
        let aider = &specs[3];
        assert_eq!(
            aider.transcripts.as_ref().unwrap().root,
            Path::new("/home/u/.aider/s")
        );
        assert_eq!(render(&aider.resume, Some("a b")), "aider --restore 'a b'");
        assert_eq!(report[0].result, Ok("aider".into()));
    }

    #[test]
    fn a_later_file_replaces_an_earlier_one_with_the_same_id() {
        let d = dir("override");
        std::fs::write(d.join("a.json"), AIDER).unwrap();
        std::fs::write(
            d.join("b.json"),
            AIDER.replace("\"Aider\"", "\"Aider (mine)\""),
        )
        .unwrap();
        let (specs, report) = load(&d, Path::new("/h"));
        assert_eq!(specs.len(), 4);
        assert_eq!(specs[3].name, "Aider (mine)");
        assert!(report[0].result.as_ref().unwrap_err().contains("b.json"));
        assert_eq!(report[1].result, Ok("aider".into()));
    }

    #[test]
    fn unusable_declarations_are_reported_and_the_rest_still_load() {
        let d = dir("bad");
        let cases = [
            ("1.json", "not json".to_owned()),
            ("2.json", AIDER.replace("\"api\":1", "\"api\":2")),
            (
                "3.json",
                AIDER.replace("\"id\":\"aider\"", "\"id\":\"claude\""),
            ),
            (
                "4.json",
                AIDER.replace("\"executable\":\"aider\"", "\"executable\":\"codex\""),
            ),
            (
                "5.json",
                AIDER.replace("\"id\":\"aider\"", "\"id\":\"Bad Id\""),
            ),
            ("6.json", AIDER.replace("\"/sid\"", "\"sid\"")),
            ("7.json", AIDER.replace("\"~/.aider/s\"", "\"rel/s\"")),
            (
                "8.json",
                AIDER.replace("\"api\":1,", "\"api\":1,\"extra\":true,"),
            ),
            ("9.json", AIDER.to_owned()),
        ];
        for (name, body) in &cases {
            std::fs::write(d.join(name), body).unwrap();
        }
        let (specs, report) = load(&d, Path::new("/h"));
        assert_eq!(specs.len(), 4, "only the good one registers");
        let refused = report.iter().filter(|r| r.result.is_err()).count();
        assert_eq!(refused, 8, "{report:#?}");
        assert!(report[1].result.as_ref().unwrap_err().contains("api 2"));
        assert!(
            report[2]
                .result
                .as_ref()
                .unwrap_err()
                .contains("Claude Code")
        );
    }

    #[test]
    fn no_directory_means_built_ins_only() {
        let (specs, report) = load(Path::new("/nonexistent/keepline/agents"), Path::new("/h"));
        assert_eq!(specs.len(), 3);
        assert!(report.is_empty());
    }
}
