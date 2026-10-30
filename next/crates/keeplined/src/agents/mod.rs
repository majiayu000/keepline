//! Per-CLI knowledge. Everything else in the daemon is CLI-agnostic and asks here: where a
//! CLI keeps its transcripts, how a transcript names its conversation, how its records map
//! onto a card, how a live process names the conversation it runs, and what to do with
//! one no transcript has claimed yet. Supporting another CLI is one `AgentSpec` (start and
//! resume commands) plus one `Adapter` here, or a declaration file and no code at all
//! (`declared`). See docs/architecture/PLUGIN_SYSTEM.md.
use crate::content::{Official, Parser};
use keepline_core::protocol::TerminalNote;
use keepline_core::{content::Source, model::Agent, model::AgentSession};
use serde_json::Value;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(crate) mod claude;
pub(crate) mod codex;
mod cursor;
mod declared;
mod grok;
pub mod hooks;
pub mod mirror;
mod qoder;

/// What the host says on a terminal about an Agent it sees but cannot read: `text` is the
/// recognition hint, `note` the pane's words for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub text: String,
    pub note: TerminalNote,
}

/// What the host does with a recognised process no session has claimed yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    /// Its transcript will name it.
    Wait,
    /// It writes no transcript: say so on its terminal and only watch its exit.
    Unrecorded(Hint),
    /// Its conversation may run outside the PTY, so only screen evidence binds a log to it
    /// (`outside_pty`). With `title_status`, the terminal title paints its state meanwhile.
    Unbound { title_status: bool },
    /// Unbound like `Unbound { title_status: false }`, with nothing to say on its terminal
    /// yet (`--no-daemon` Codex before its first prompt).
    Quiet,
}

/// Any CLI's live process no session has claimed: unbound, so screen association and hooks
/// get their chance, unless a child of the same CLI is the one that will own the session.
pub fn unclaimed(
    process: &crate::recognition::Process,
    processes: &[crate::recognition::Process],
) -> Pending {
    if processes
        .iter()
        .any(|p| p.parent == process.pid && p.runtime == process.runtime)
    {
        Pending::Wait
    } else {
        Pending::Unbound {
            title_status: false,
        }
    }
}

/// The terminal's note for a recognised Agent whose command line names no conversation.
pub fn unbound_hint(agent: Agent) -> Hint {
    let cli = agent.executable();
    Hint {
        text: format!("{}{cli}", keepline_core::protocol::AGENT_UNBOUND_PREFIX),
        note: TerminalNote {
            note: format!("{cli} · 发出第一句后对上对话"),
            why: format!(
                "Keepline 看到这个 {cli} 在跑，但它的命令行没写是哪段对话。发出第一句话后，按画面上的提示词对上；想启动就认出，运行一次 keeplined hooks install 装上 Hook。"
            ),
        },
    }
}

/// The CLI's own account of its sessions: what was asked, and per entry of `due` it
/// answered for, that session's status.
pub type OfficialAnswer = (&'static str, io::Result<Vec<(usize, Official)>>);

pub trait Adapter: Sync {
    /// The directory this CLI writes transcripts under, honouring its own home override.
    fn root(&self, home: &Path) -> PathBuf;
    /// Whether a process is this CLI: its executable, or the script an interpreter runs.
    fn runs(&self, executable: &str, script: Option<&str>) -> bool;
    /// The conversation a live process names, and whether it opens its own transcript.
    fn process_session(&self, pid: u32, args: &[String]) -> (Option<String>, bool);
    /// A file the CLI rewrites when a process opens or closes a session. A process can
    /// write its transcript before registering, so a change here identifies processes again.
    fn registry(&self, _root: &Path) -> Option<PathBuf> {
        None
    }
    /// Whether a file under the root is a transcript.
    fn is_transcript(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "jsonl")
    }
    /// Whether a directory under the root can hold transcripts; the walk skips the rest.
    fn descend(&self, _dir: &Path) -> bool {
        true
    }
    /// A record, decoded leniently enough to read metadata from a truncated header line.
    fn header_value(&self, line: &[u8]) -> Option<Value> {
        serde_json::from_slice(line).ok()
    }
    /// The conversation id and working directory a transcript record names.
    fn identity(&self, path: &Path, record: &Value) -> Option<(String, String)>;
    /// Facts kept beside the transcript rather than in it. Asked again until the session is
    /// known to be interactive or hidden.
    fn observe_file(&self, _parser: &mut Parser, _path: &Path) {}
    /// The name the CLI keeps for a transcript's conversation outside the transcript.
    fn name(&self, _root: &Path, _transcript: &Path) -> Option<String> {
        None
    }
    /// Session metadata a record carries: why it is hidden, whether it is interactive.
    fn metadata(&self, parser: &mut Parser, record: &Value);
    /// Whether a record is the user speaking; the bounded first-request search reads these.
    fn is_user(&self, record: &Value) -> bool;
    /// One transcript record onto the card.
    fn add(&self, parser: &mut Parser, record: &Value, at: u64, source: &Source);
    /// Whether subagents keep their own transcripts that name this one as their parent.
    fn links_children(&self) -> bool {
        false
    }
    /// A live process of this CLI that no session has claimed yet.
    fn pending(
        &self,
        process: &crate::recognition::Process,
        processes: &[crate::recognition::Process],
    ) -> Pending {
        unclaimed(process, processes)
    }
    /// For a `Pending::Unbound { title_status: true }` process: its state from the title.
    fn title_status(&self, _title: &str) -> Option<Hint> {
        None
    }
    /// Whether a conversation can run outside the PTY (a shared daemon), leaving no process
    /// or open-file evidence: then a log binds to a terminal only by what its screen shows,
    /// and logs the daemon keeps open are followed while one of its terminals is unbound.
    fn outside_pty(&self) -> bool {
        false
    }
    /// Asks the CLI itself how `due`, its sessions waiting on a dialog, stand. `None`: it
    /// has no such source. An error falls back to logs and screens, never fails a request.
    fn official(&self, _due: &[&(AgentSession, Option<u32>)]) -> Option<OfficialAnswer> {
        None
    }
    /// For live processes whose command line names no conversation, the one the CLI's own
    /// list of live sessions gives each pid, when exactly one. Empty when it keeps no such
    /// list or could not be asked; the screen and hooks then get their chance.
    fn sessions_by_pid(&self, _pids: &[u32]) -> Vec<(u32, String)> {
        vec![]
    }
    /// Where this CLI reads command hooks from and which events report to the host. A CLI
    /// whose processes or files already name their conversation needs none.
    fn hooks(&self, _home: &Path) -> Option<Hooks> {
        None
    }
    /// The bundled rules (`screen/<cli>.json`) that read a blocked question off this CLI's
    /// quiet screen. See `crate::screen_rules`.
    fn screen_rules(&self) -> &'static str {
        DEFAULT_SCREEN_RULES
    }
}
/// A CLI without its own screen rules is read with Claude Code's shapes, as every CLI was
/// before the rules became files.
pub(crate) const DEFAULT_SCREEN_RULES: &str = include_str!("screen/claude.json");
/// A CLI's hook settings: a JSON file with `hooks.<Event>[].hooks[]` command entries, the
/// shape Claude Code reads.
pub struct Hooks {
    pub settings: PathBuf,
    pub events: &'static [&'static str],
}

pub fn adapter(agent: Agent) -> &'static dyn Adapter {
    match agent {
        Agent::Codex => &codex::Codex,
        Agent::Claude => &claude::Claude,
        Agent::Grok => &grok::Grok,
        Agent::Cursor => &cursor::Cursor,
        Agent::Qoder => &qoder::Qoder,
        _ => declared()
            .iter()
            .find(|d| d.agent() == agent)
            .map_or(&declared::Missing as &dyn Adapter, |d| d as &dyn Adapter),
    }
}

/// Every registered CLI that is not built in, read from its declaration once.
fn declared() -> &'static [declared::Declared] {
    static DECLARED: OnceLock<Vec<declared::Declared>> = OnceLock::new();
    DECLARED.get_or_init(|| {
        Agent::all()
            .into_iter()
            .filter_map(declared::Declared::new)
            .collect()
    })
}

/// Each CLI's transcript root under `home`; a CLI that writes none has no root.
pub fn roots(home: &Path) -> Vec<(Agent, PathBuf)> {
    Agent::all()
        .into_iter()
        .map(|a| (a, adapter(a).root(home)))
        .filter(|(_, root)| !root.as_os_str().is_empty())
        .collect()
}

/// The CLI a `ps` command line runs, if any.
pub fn from_command(command: &str) -> Option<Agent> {
    let mut args = command.split_whitespace();
    let executable = Path::new(args.next()?).file_name()?.to_str()?;
    let interpreter = matches!(executable, "node" | "bun")
        || executable.to_ascii_lowercase().starts_with("python");
    let script = interpreter.then(|| args.next()).flatten();
    Agent::all()
        .into_iter()
        .find(|a| adapter(*a).runs(executable, script))
}

/// The conversation id and working directory a transcript line names.
pub fn identity(agent: Agent, path: &Path, line: &[u8]) -> Option<(String, String)> {
    let adapter = adapter(agent);
    adapter
        .identity(path, &adapter.header_value(line)?)
        .filter(|(id, _)| !id.is_empty())
}

/// The environment variable a CLI's home can be moved with, or `home/default`.
fn home_dir(home: &Path, var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(default))
}

/// Cards hide sessions run in throwaway directories; a managed terminal still shows them.
fn temporary(cwd: &str) -> bool {
    [
        "/tmp",
        "/private/tmp",
        "/var/tmp",
        "/private/var/tmp",
        "/var/folders",
        "/private/var/folders",
    ]
    .iter()
    .any(|root| cwd == *root || cwd.starts_with(&format!("{root}/")))
}
