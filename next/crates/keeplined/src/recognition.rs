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
    pub cli_session_id: Option<String>,
    /// Codex started with `--no-daemon`: its own process opens the log, so lsof will bind it.
    pub no_daemon: bool,
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
/// Read argv boundaries from the kernel, never parse a prompt out of `ps command`.
/// KERN_PROCARGS2 also returns an environment tail; stop after argc and never retain it.
fn process_argv(pid: u32) -> io::Result<Vec<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as i32];
    let mut len = 0;
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut bytes = vec![0u8; len];
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            bytes.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    bytes.truncate(len);
    if bytes.len() < 4 {
        return Ok(vec![]);
    }
    let argc = i32::from_ne_bytes(bytes[..4].try_into().unwrap()).max(0) as usize;
    let Some(end) = bytes[4..].iter().position(|b| *b == 0) else {
        return Ok(vec![]);
    };
    let mut pos = 4 + end;
    while bytes.get(pos) == Some(&0) {
        pos += 1;
    }
    let mut args = Vec::new();
    for _ in 0..argc {
        let Some(end) = bytes[pos..].iter().position(|b| *b == 0) else {
            break;
        };
        args.push(String::from_utf8_lossy(&bytes[pos..pos + end]).into_owned());
        pos += end + 1;
    }
    Ok(args)
}
fn claude_session_id(args: &[String]) -> Option<String> {
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
// Codex options that consume the next argv element; anything else positional ends option parsing.
const CODEX_VALUE_FLAGS: &[&str] = &[
    "-c",
    "--config",
    "-m",
    "--model",
    "-p",
    "--profile",
    "-s",
    "--sandbox",
    "-a",
    "--ask-for-approval",
    "-C",
    "--cd",
    "-i",
    "--image",
    "--add-dir",
    "--enable",
    "--disable",
    "--remote",
    "--remote-auth-token-env",
    "--local-provider",
];
/// `codex [opts] resume <uuid>` names its thread exactly, with or without the shared daemon.
/// `--last`, thread names, `fork` and prompts are not identity evidence.
fn codex_args(args: &[String]) -> (Option<String>, bool) {
    let no_daemon = args
        .iter()
        .skip(1)
        .take_while(|a| *a != "--")
        .any(|a| a == "--no-daemon");
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if CODEX_VALUE_FLAGS.contains(&arg.as_str()) {
            args.next();
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        if arg != "resume" {
            break;
        }
        let mut found = None;
        while let Some(arg) = args.next() {
            if arg == "--last" {
                return (None, no_daemon);
            }
            if CODEX_VALUE_FLAGS.contains(&arg.as_str()) {
                args.next();
            } else if arg == "--" || !arg.starts_with('-') {
                let id = if arg == "--" { args.next() } else { Some(arg) };
                found = id
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    .map(|id| id.to_string());
                break;
            }
        }
        return (found, no_daemon);
    }
    (None, no_daemon)
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
        let args = if runtime.is_some() {
            process_argv(pid).unwrap_or_default()
        } else {
            vec![]
        };
        let (cli_session_id, no_daemon) = match runtime {
            Some(Agent::Claude) => (claude_session_id(&args), false),
            Some(Agent::Codex) => codex_args(&args),
            None => (None, false),
        };
        processes.push(Process {
            pid,
            parent,
            runtime,
            cli_session_id,
            no_daemon,
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
pub(crate) fn observation(t: &Transcript, terminal: Option<String>) -> AgentSession {
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
                    && match &process.cli_session_id {
                        Some(id) => id == &t.cli_id,
                        _ => open_files
                            .get(&process.pid)
                            .is_some_and(|paths| paths.contains(&t.path)),
                    }
            })
            .collect();
        // A node wrapper and its native Codex child carry the same argv: one binding, not two.
        if matching.len() == 1
            && !bindings.iter().any(|(id, t): &(String, &Transcript)| {
                id == &terminal && t.path == matching[0].path
            })
        {
            bindings.push((terminal, matching[0]));
        }
    }
    for (terminal, transcript) in &bindings {
        if bindings.iter().filter(|(id, _)| id == terminal).count() != 1
            || bindings
                .iter()
                .filter(|(_, t)| t.runtime == transcript.runtime && t.cli_id == transcript.cli_id)
                .count()
                != 1
        {
            continue;
        }
        workspace
            .observe_agent(observation(transcript, Some(terminal.clone())))
            .map_err(io::Error::other)?;
    }
    Ok(())
}
/// Shared-daemon Codex has no process or file evidence. The only identity left is the
/// thread's newest prompt shown verbatim on one managed screen. Short prompts are refused:
/// "继续" or "ok" appear everywhere.
pub fn screen_key(prompt: &str) -> Option<String> {
    let key: String = prompt
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(160)
        .collect();
    (key.chars().count() >= 12).then_some(key)
}
pub fn screen_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
pub struct ScreenThread<'a> {
    pub transcript: &'a Transcript,
    pub key: String,
    /// The log's cwd with symlinks resolved, comparable with the kernel's process cwd.
    pub cwd: String,
}
pub struct ScreenTerminal {
    pub terminal: String,
    pub cwd: Option<String>,
    /// Whitespace-free tail of this terminal's own screen.
    pub screen: String,
    /// The Codex thread this TUI showed before (`/new` hands the pane to a newer thread).
    pub current: Option<(String, u64, Option<String>)>,
}
/// Pair threads with terminals one-to-one. A thread showing on two screens, or a screen
/// showing two threads, binds nothing; the next distinct prompt can settle it.
pub fn screen_bindings<'a>(
    threads: &[ScreenThread<'a>],
    terminals: &[ScreenTerminal],
) -> Vec<(String, &'a Transcript)> {
    let fits = |thread: &ScreenThread, terminal: &ScreenTerminal| {
        terminal.cwd.as_ref().is_none_or(|cwd| cwd == &thread.cwd)
            && terminal.screen.contains(&thread.key)
            && terminal.current.as_ref().is_none_or(|(id, updated, key)| {
                id != &thread.transcript.cli_id
                    && thread.transcript.updated >= *updated
                    && key.as_ref() != Some(&thread.key)
            })
    };
    let mut out = vec![];
    for thread in threads {
        let mut matched = terminals.iter().filter(|t| fits(thread, t));
        let (Some(terminal), None) = (matched.next(), matched.next()) else {
            continue;
        };
        if threads.iter().filter(|other| fits(other, terminal)).count() == 1 {
            out.push((terminal.terminal.clone(), thread.transcript));
        }
    }
    out
}
pub fn process_cwd(pid: u32) -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as i32;
    // SAFETY: the kernel writes at most `size` bytes into the zeroed buffer.
    let n = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if n != size {
        return None;
    }
    let info = unsafe { info.assume_init() };
    let path = unsafe { std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) };
    Some(path.to_string_lossy().into_owned()).filter(|p| !p.is_empty())
}
pub fn open_transcripts(
    workspace: &Workspace,
    processes: &[Process],
) -> io::Result<HashMap<u32, Vec<String>>> {
    let pids = processes
        .iter()
        .filter(|p| {
            p.runtime.is_some()
                && p.cli_session_id.is_none()
                && terminal_for_process(p.pid, processes, workspace)
                    .is_some_and(|id| workspace.current_agent(&id).is_none())
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
    fn explicit_claude_id_uses_argv_boundaries_and_kernel_arguments() {
        let id = uuid::Uuid::new_v4().to_string();
        for flag in ["--session-id", "--resume", "-r"] {
            let args = vec!["claude".into(), flag.into(), id.clone(), "say ok".into()];
            assert_eq!(claude_session_id(&args), Some(id.clone()));
        }
        assert_eq!(
            claude_session_id(&["claude".into(), format!("--session-id={id}")]),
            Some(id.clone())
        );
        for args in [
            vec!["claude".into(), format!("please explain --session-id {id}")],
            vec![
                "claude".into(),
                "--".into(),
                "--session-id".into(),
                id.clone(),
            ],
            vec!["claude".into(), "--session-id".into(), "invalid".into()],
            vec![
                "claude".into(),
                "--session-id".into(),
                id.clone(),
                "--resume".into(),
                id,
            ],
        ] {
            assert_eq!(claude_session_id(&args), None);
        }
        assert_eq!(
            process_argv(std::process::id()).unwrap(),
            std::env::args().collect::<Vec<_>>()
        );
    }

    #[test]
    fn codex_resume_uuid_is_exact_and_everything_else_is_not_identity() {
        let id = uuid::Uuid::new_v4().to_string();
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        for args in [
            v(&["codex", "resume", &id]),
            v(&["codex", "--no-daemon", "resume", &id, "继续"]),
            v(&["codex", "-m", "gpt", "resume", "-c", "a=1", &id]),
            v(&[
                "codex",
                "--dangerously-bypass-approvals-and-sandbox",
                "resume",
                "--",
                &id,
            ]),
        ] {
            assert_eq!(codex_args(&args).0, Some(id.clone()), "{args:?}");
        }
        for args in [
            v(&["codex"]),
            v(&["codex", &format!("resume {id}")]),
            v(&["codex", "fix", "resume", &id]),
            v(&["codex", "fork", &id]),
            v(&["codex", "resume", "--last"]),
            v(&["codex", "resume", "my thread name"]),
            v(&["codex", "-m", "resume", &id]),
        ] {
            assert_eq!(codex_args(&args).0, None, "{args:?}");
        }
        assert!(codex_args(&v(&["codex", "--no-daemon"])).1);
        assert!(!codex_args(&v(&["codex", "--", "--no-daemon"])).1);
    }

    /// (current thread id, its last update, its newest prompt)
    type Current<'a> = Option<(&'a str, u64, &'a str)>;
    fn screen_case(prompts: &[(&str, u64)], screens: &[(&str, Current)]) -> Vec<(String, String)> {
        let transcripts: Vec<_> = prompts
            .iter()
            .enumerate()
            .map(|(i, (_, updated))| Transcript {
                runtime: Agent::Codex,
                cli_id: format!("t{i}"),
                path: format!("/logs/t{i}.jsonl"),
                cwd: "/same".into(),
                updated: *updated,
            })
            .collect();
        let threads: Vec<_> = transcripts
            .iter()
            .zip(prompts)
            .filter_map(|(transcript, (prompt, _))| {
                Some(ScreenThread {
                    transcript,
                    key: screen_key(prompt)?,
                    cwd: "/same".into(),
                })
            })
            .collect();
        let terminals: Vec<_> = screens
            .iter()
            .enumerate()
            .map(|(i, (screen, current))| ScreenTerminal {
                terminal: format!("pty-{i}"),
                cwd: Some("/same".into()),
                screen: screen_text(screen),
                current: current.map(|(id, at, prompt)| (id.into(), at, screen_key(prompt))),
            })
            .collect();
        screen_bindings(&threads, &terminals)
            .into_iter()
            .map(|(terminal, t)| (terminal, t.cli_id.clone()))
            .collect()
    }

    #[test]
    fn screen_prompt_binds_one_to_one_and_refuses_everything_ambiguous() {
        let a = "请检查登录页的跳转逻辑并修复回归";
        let b = "给设置页补上深色模式的截图测试";
        // Same cwd, wrapped differently on screen: each thread finds only its own pane.
        assert_eq!(
            screen_case(
                &[(a, 1), (b, 1)],
                &[
                    ("› 请检查登录页的跳转逻辑\n  并修复回归\n", None),
                    (format!("› {b}").as_str(), None)
                ]
            ),
            vec![("pty-0".into(), "t0".into()), ("pty-1".into(), "t1".into())]
        );
        // Identical prompts on two panes bind neither.
        assert!(screen_case(&[(a, 1), (a, 1)], &[(a, None), (a, None)]).is_empty());
        // One thread visible on two panes, or a pane showing two threads: nothing.
        assert!(screen_case(&[(a, 1)], &[(a, None), (a, None)]).is_empty());
        assert!(screen_case(&[(a, 1), (b, 1)], &[(format!("{a}\n{b}").as_str(), None)]).is_empty());
        // Short prompts are never evidence.
        assert!(screen_case(&[("继续", 1)], &[("继续", None)]).is_empty());
        // /new: a newer thread takes its pane; an older one never does.
        assert_eq!(
            screen_case(
                &[(b, 5)],
                &[(format!("{a}\n{b}").as_str(), Some(("old", 4, a)))]
            ),
            vec![("pty-0".into(), "t0".into())]
        );
        assert!(screen_case(&[(b, 3)], &[(b, Some(("old", 4, a)))]).is_empty());
        // The pane's current thread already has this prompt: the screen cannot tell them apart.
        assert!(screen_case(&[(a, 5)], &[(a, Some(("old", 4, a)))]).is_empty());
    }

    #[test]
    fn process_cwd_reads_the_kernel_directory() {
        assert_eq!(
            process_cwd(std::process::id()).map(PathBuf::from),
            std::env::current_dir().ok()
        );
    }

    #[test]
    fn codex_wrapper_and_native_child_with_one_resume_id_bind_once() {
        let mut w = Workspace::default();
        w.terminals.push(Terminal {
            id: "pty".into(),
            space: "s".into(),
            argv: vec!["sh".into()],
            cwd: "/same".into(),
            pid: Some(100),
            state: ProcessState::Running,
            exit_code: None,
            error: None,
        });
        let id = uuid::Uuid::new_v4().to_string();
        let process = |pid, parent| Process {
            pid,
            parent,
            runtime: Some(Agent::Codex),
            cli_session_id: Some(id.clone()),
            no_daemon: false,
        };
        let shell = Process {
            pid: 100,
            parent: 1,
            runtime: None,
            cli_session_id: None,
            no_daemon: false,
        };
        let processes = vec![shell, process(101, 100), process(102, 101)];
        let transcripts = vec![Transcript {
            runtime: Agent::Codex,
            cli_id: id.clone(),
            path: "/logs/x.jsonl".into(),
            cwd: "/same".into(),
            updated: 1,
        }];
        reconcile(&mut w, &transcripts, &processes, &HashMap::new()).unwrap();
        assert_eq!(w.current_agent("pty").unwrap().cli_session_id, id);
    }

    #[test]
    fn explicit_ids_disambiguate_same_cwd_without_open_files() {
        let mut w = Workspace::default();
        let mut processes = vec![];
        let mut transcripts = vec![];
        for i in 0..3 {
            let id = uuid::Uuid::new_v4().to_string();
            w.terminals.push(Terminal {
                id: format!("pty-{i}"),
                space: "s".into(),
                argv: vec!["claude".into()],
                cwd: "/same".into(),
                pid: Some(100 + i),
                state: ProcessState::Running,
                exit_code: None,
                error: None,
            });
            processes.push(Process {
                pid: 100 + i,
                parent: 1,
                runtime: Some(Agent::Claude),
                cli_session_id: (i < 2).then(|| id.clone()),
                no_daemon: false,
            });
            transcripts.push(Transcript {
                runtime: Agent::Claude,
                cli_id: id,
                path: format!("/same/{i}.jsonl"),
                cwd: "/same".into(),
                updated: 1,
            });
        }
        reconcile(&mut w, &transcripts, &processes, &HashMap::new()).unwrap();
        for (i, transcript) in transcripts.iter().take(2).enumerate() {
            assert_eq!(
                w.current_agent(&format!("pty-{i}")).unwrap().cli_session_id,
                transcript.cli_id
            );
        }
        assert!(w.current_agent("pty-2").is_none());
        // A managed process with a mismatched ID cannot claim another transcript even if it opens it.
        let mut mismatch = w.clone();
        mismatch.agent_sessions.clear();
        processes[0].cli_session_id = Some(uuid::Uuid::new_v4().to_string());
        reconcile(
            &mut mismatch,
            &transcripts,
            &processes,
            &HashMap::from([(100, vec![transcripts[0].path.clone()])]),
        )
        .unwrap();
        assert!(mismatch.current_agent("pty-0").is_none());
    }

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
                cli_session_id: None,
                no_daemon: false,
            },
            Process {
                pid: 101,
                parent: 100,
                runtime: Some(Agent::Codex),
                cli_session_id: None,
                no_daemon: false,
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
