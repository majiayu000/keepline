//! Read-only CLI identity discovery. A shared cwd is never association evidence.
use keepline_core::model::{Agent, AgentSession, Outcome, ProcessState, Workspace};
use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    process::Command,
};

use std::sync::atomic::{AtomicU64, Ordering};
static PS_SPAWNS: AtomicU64 = AtomicU64::new(0);
static LSOF_SPAWNS: AtomicU64 = AtomicU64::new(0);
pub fn probe_counts() -> (u64, u64) {
    (
        PS_SPAWNS.load(Ordering::Relaxed),
        LSOF_SPAWNS.load(Ordering::Relaxed),
    )
}

#[derive(Clone, Debug)]
pub struct Process {
    pub pid: u32,
    pub parent: u32,
    pub runtime: Option<Agent>,
}
#[derive(Clone, Debug)]
pub struct Transcript {
    pub runtime: Agent,
    pub cli_id: String,
    pub path: String,
    pub cwd: String,
    pub updated: u64,
}

pub fn roots() -> io::Result<Vec<(Agent, PathBuf)>> {
    let home = PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?,
    );
    let codex = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"));
    Ok(vec![
        (Agent::Codex, codex.join("sessions")),
        (Agent::Claude, claude.join("projects")),
    ])
}
fn runtime_from_command(command: &str) -> Option<Agent> {
    let mut args = command.split_whitespace();
    let executable = Path::new(args.next()?).file_name()?.to_str()?;
    match executable {
        "codex" => Some(Agent::Codex),
        "claude" => Some(Agent::Claude),
        "node" | "bun" => {
            let script = args.next()?;
            if script.ends_with("/@anthropic-ai/claude-code/cli.js") {
                Some(Agent::Claude)
            } else if script.ends_with("/@openai/codex/bin/codex.js") {
                Some(Agent::Codex)
            } else {
                None
            }
        }
        _ => None,
    }
}
/// Enumerate only the trees rooted at live PTYs. No global ps or process-name scan.
pub fn processes(workspace: &Workspace) -> io::Result<Vec<Process>> {
    let mut pending: Vec<_> = workspace
        .terminals
        .iter()
        .filter(|t| t.state == ProcessState::Running)
        .filter_map(|t| t.pid)
        .collect();
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    while let Some(pid) = pending.pop() {
        if !seen.insert(pid) {
            continue;
        }
        selected.push(pid);
        let count = unsafe { libc::proc_listchildpids(pid as i32, std::ptr::null_mut(), 0) };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                continue;
            }
            return Err(error);
        }
        let mut children = vec![0i32; count as usize + 32];
        let count = unsafe {
            libc::proc_listchildpids(
                pid as i32,
                children.as_mut_ptr().cast(),
                (children.len() * std::mem::size_of::<i32>()) as i32,
            )
        };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                continue;
            }
            return Err(error);
        }
        if count as usize >= children.len() {
            return Err(io::Error::other(
                "managed process tree changed during enumeration; retry on next event",
            ));
        }
        pending.extend(
            children[..count as usize]
                .iter()
                .filter(|p| **p > 0)
                .map(|p| *p as u32),
        );
    }
    if selected.is_empty() {
        return Ok(vec![]);
    }
    let pids = selected
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    PS_SPAWNS.fetch_add(1, Ordering::Relaxed);
    let result = Command::new("/bin/ps")
        .args(["-p", &pids, "-o", "pid=,ppid=,command="])
        .output()?;
    if !result.status.success() && result.stdout.is_empty() {
        return Ok(vec![]);
    }
    let mut processes = vec![];
    for line in String::from_utf8_lossy(&result.stdout).lines() {
        let mut fields = line.trim().splitn(2, char::is_whitespace);
        let Some(pid) = fields.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        let rest = fields.next().unwrap_or("").trim_start();
        let mut fields = rest.splitn(2, char::is_whitespace);
        let Some(parent) = fields.next().and_then(|s| s.parse().ok()) else {
            continue;
        };
        let runtime = runtime_from_command(fields.next().unwrap_or("").trim_start());
        processes.push(Process {
            pid,
            parent,
            runtime,
        });
    }
    Ok(processes)
}
fn ancestors(pid: u32, processes: &[Process]) -> Vec<&Process> {
    let by_id: HashMap<_, _> = processes.iter().map(|p| (p.pid, p)).collect();
    let mut seen = HashSet::new();
    let mut pid = pid;
    let mut out = vec![];
    while seen.insert(pid) {
        let Some(process) = by_id.get(&pid) else {
            break;
        };
        out.push(*process);
        pid = process.parent;
    }
    out
}
pub fn terminal_for_process(
    pid: u32,
    processes: &[Process],
    workspace: &Workspace,
) -> Option<String> {
    for process in ancestors(pid, processes) {
        if let Some(t) = workspace
            .terminals
            .iter()
            .find(|t| t.pid == Some(process.pid) && t.state == ProcessState::Running)
        {
            return Some(t.id.clone());
        }
    }
    None
}
fn observation(t: &Transcript, terminal: Option<String>) -> AgentSession {
    AgentSession {
        id: uuid::Uuid::new_v4().to_string(),
        runtime: t.runtime,
        cli_session_id: t.cli_id.clone(),
        current: terminal.is_some(),
        terminal,
        task: None,
        outcome: Outcome::Open,
        updated: t.updated,
        cwd: t.cwd.clone(),
        transcript: t.path.clone(),
    }
}
pub fn apply_hook(
    workspace: &mut Workspace,
    runtime: Agent,
    pid: u32,
    cli_id: &str,
    transcript: &Transcript,
    processes: &[Process],
    ended: bool,
) -> io::Result<()> {
    let agent = ancestors(pid, processes)
        .into_iter()
        .find(|p| p.runtime.is_some())
        .ok_or_else(|| io::Error::other("Hook ancestry contains no recognized Agent process"))?;
    if agent.runtime != Some(runtime) {
        return Err(io::Error::other(
            "Hook runtime disagrees with process identity",
        ));
    }
    let terminal = terminal_for_process(agent.pid, processes, workspace).ok_or_else(|| {
        io::Error::other("Hook process is not a descendant of a managed terminal")
    })?;
    if transcript.cli_id != cli_id {
        return Err(io::Error::other(
            "Hook session ID disagrees with transcript",
        ));
    }
    let mut observed = observation(transcript, Some(terminal));
    observed.current = !ended;
    workspace.observe_agent(observed).map_err(io::Error::other)
}
/// Refresh existing conversations, then bind only a single matching open transcript per live Agent.
/// Unassociated historical files stay in the SQLite index, outside the live workspace payload.
/// Ambiguous open-file evidence remains unbound rather than choosing by directory.
pub fn reconcile(
    workspace: &mut Workspace,
    transcripts: &[Transcript],
    processes: &[Process],
    open_files: &HashMap<u32, Vec<String>>,
) -> io::Result<()> {
    for transcript in transcripts
        .iter()
        .filter(|t| {
            workspace
                .agent_sessions
                .iter()
                .any(|s| s.runtime == t.runtime && s.cli_session_id == t.cli_id)
        })
        .cloned()
        .collect::<Vec<_>>()
    {
        workspace
            .observe_agent(observation(&transcript, None))
            .map_err(io::Error::other)?;
    }
    let mut bindings = Vec::new();
    for process in processes.iter().filter(|p| p.runtime.is_some()) {
        let Some(terminal) = terminal_for_process(process.pid, processes, workspace) else {
            continue;
        };
        let matching: Vec<_> = transcripts
            .iter()
            .filter(|t| {
                Some(t.runtime) == process.runtime
                    && open_files
                        .get(&process.pid)
                        .is_some_and(|paths| paths.contains(&t.path))
            })
            .collect();
        if matching.len() == 1 {
            bindings.push((terminal, matching[0]));
        }
    }
    for (terminal, transcript) in &bindings {
        if bindings.iter().filter(|(id, _)| id == terminal).count() != 1 {
            continue;
        }
        workspace
            .observe_agent(observation(transcript, Some(terminal.clone())))
            .map_err(io::Error::other)?;
    }
    Ok(())
}
pub fn open_transcripts(
    workspace: &Workspace,
    processes: &[Process],
) -> io::Result<HashMap<u32, Vec<String>>> {
    let pids = processes
        .iter()
        .filter(|p| {
            p.runtime.is_some() && terminal_for_process(p.pid, processes, workspace).is_some()
        })
        .map(|p| p.pid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if pids.is_empty() {
        return Ok(HashMap::new());
    }
    LSOF_SPAWNS.fetch_add(1, Ordering::Relaxed);
    let result = Command::new("/usr/sbin/lsof")
        .args(["-nP", "-p", &pids, "-Fpn"])
        .output()?;
    // lsof can return 1 when one process exits during enumeration; valid rows remain useful.
    if !result.status.success() && result.stdout.is_empty() {
        if result.status.code() == Some(1) {
            return Ok(HashMap::new());
        }
        return Err(io::Error::other(
            "open-file inventory failed; no association was inferred",
        ));
    }
    let mut files: HashMap<u32, Vec<String>> = HashMap::new();
    let mut pid = None;
    for line in String::from_utf8_lossy(&result.stdout).lines() {
        if let Some(value) = line.strip_prefix('p') {
            pid = value.parse().ok();
        } else if let (Some(pid), Some(path)) = (pid, line.strip_prefix('n')) {
            if path.ends_with(".jsonl") {
                files.entry(pid).or_default().push(path.into());
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::model::{Space, Terminal};
    #[test]
    fn log_identity_alone_does_not_bind_by_directory_and_ambiguity_stays_unbound() {
        let mut workspace = Workspace::default();
        workspace.spaces.push(Space {
            id: "s".into(),
            name: "s".into(),
            cwd: "/tmp".into(),
        });
        workspace.terminals.push(Terminal {
            id: "pty".into(),
            space: "s".into(),
            argv: vec!["sh".into()],
            cwd: "/tmp".into(),
            pid: Some(100),
            state: ProcessState::Running,
            exit_code: None,
            error: None,
        });
        let transcripts = vec![
            Transcript {
                runtime: Agent::Codex,
                cli_id: "one".into(),
                path: "/logs/one.jsonl".into(),
                cwd: "/tmp".into(),
                updated: 1,
            },
            Transcript {
                runtime: Agent::Codex,
                cli_id: "two".into(),
                path: "/logs/two.jsonl".into(),
                cwd: "/tmp".into(),
                updated: 2,
            },
        ];
        let processes = vec![
            Process {
                pid: 100,
                parent: 1,
                runtime: None,
            },
            Process {
                pid: 101,
                parent: 100,
                runtime: Some(Agent::Codex),
            },
        ];
        reconcile(&mut workspace, &transcripts, &processes, &HashMap::new()).unwrap();
        assert!(workspace
            .agent_sessions
            .iter()
            .all(|s| s.terminal.is_none()));
        let files = HashMap::from([(
            101,
            vec!["/logs/one.jsonl".into(), "/logs/two.jsonl".into()],
        )]);
        reconcile(&mut workspace, &transcripts, &processes, &files).unwrap();
        assert!(workspace
            .agent_sessions
            .iter()
            .all(|s| s.terminal.is_none()));
        let files = HashMap::from([(101, vec!["/logs/two.jsonl".into()])]);
        reconcile(&mut workspace, &transcripts, &processes, &files).unwrap();
        assert_eq!(
            workspace.current_agent("pty").unwrap().cli_session_id,
            "two"
        );
    }
}
