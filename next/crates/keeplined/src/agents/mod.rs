//! Per-CLI knowledge. Everything else in the daemon is CLI-agnostic and asks here: where a
//! CLI keeps its transcripts, how a transcript names its conversation, how its records map
//! onto a card, and how a live process names the conversation it runs. Supporting another
//! CLI is one more `Agent` variant (start and resume commands) plus one `Adapter` here.
use crate::content::Parser;
use keepline_core::{content::Source, model::Agent};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) mod claude;
pub(crate) mod codex;
mod grok;

pub trait Adapter: Sync {
    /// The directory this CLI writes transcripts under, honouring its own home override.
    fn root(&self, home: &Path) -> PathBuf;
    /// Whether a process is this CLI: its executable, or the script an interpreter runs.
    fn runs(&self, executable: &str, script: Option<&str>) -> bool;
    /// The conversation a live process names, and whether it opens its own transcript.
    fn process_session(&self, pid: u32, args: &[String]) -> (Option<String>, bool);
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
}

pub fn adapter(agent: Agent) -> &'static dyn Adapter {
    match agent {
        Agent::Codex => &codex::Codex,
        Agent::Claude => &claude::Claude,
        Agent::Grok => &grok::Grok,
    }
}

/// Each CLI's transcript root under `home`.
pub fn roots(home: &Path) -> Vec<(Agent, PathBuf)> {
    Agent::ALL
        .into_iter()
        .map(|a| (a, adapter(a).root(home)))
        .collect()
}

/// The CLI a `ps` command line runs, if any.
pub fn from_command(command: &str) -> Option<Agent> {
    let mut args = command.split_whitespace();
    let executable = Path::new(args.next()?).file_name()?.to_str()?;
    let script = matches!(executable, "node" | "bun")
        .then(|| args.next())
        .flatten();
    Agent::ALL
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
