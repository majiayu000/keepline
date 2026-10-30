//! Driving Keepline terminals from scripts and other Agents, after herdr's `pane` and
//! `agent` commands: list them, read a screen, send text, prompt an Agent and wait.
use keepline_core::{content::Turn, model::*, protocol::*};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const USAGE: &str = "\
keeplined list [--json]                      终端列表：编号、Agent 状态、标签、目录
keeplined read <终端> [--lines N]            画面最后 N 行（默认 50）
keeplined send <终端> <文字|-> [--enter]     把文字作为粘贴放进终端；- 从标准输入读
keeplined prompt <终端> <文字|-> [--wait] [--timeout 秒] [--force]
                                             给 Agent 发一条并回车；它正等你回答时拒绝，除非 --force
keeplined wait <终端> [--until idle|waiting|working] [--timeout 秒]
                                             等 Agent 进入某个状态（默认 idle：这一轮做完）
<终端> 可以是编号的前几位或标签名；全部命令都接受 --runtime <目录>。";

/// What the Agent in a terminal is doing, read like the workbench's pane states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Working,
    Waiting,
    Idle,
    /// No Agent conversation is current in this terminal: a shell or another program.
    None,
}
impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Waiting => "waiting",
            Self::Idle => "idle",
            Self::None => "-",
        }
    }
}
pub fn phase(w: &Workspace, terminal: &str) -> Phase {
    let Some(s) = w.current_agent(terminal) else {
        return Phase::None;
    };
    match w.content.get(&s.id) {
        None => Phase::Idle,
        Some(c) => match c.turn {
            Turn::Waiting => Phase::Waiting,
            Turn::Running => Phase::Working,
            Turn::Completed if c.background.is_some_and(|b| b.keeps_working()) => Phase::Working,
            _ => Phase::Idle,
        },
    }
}
/// The registry key in lower case, the way the CLI is usually typed.
fn agent_label(agent: keepline_core::agent::Agent) -> String {
    agent.key().to_lowercase()
}
fn tab_title(w: &Workspace, terminal: &str) -> String {
    w.terminal_tab(terminal)
        .map(|t| t.title.clone())
        .unwrap_or_default()
}
/// A full id, a unique id prefix, or a unique tab title, among running terminals.
pub fn resolve(w: &Workspace, needle: &str) -> io::Result<String> {
    let running: Vec<&Terminal> = w
        .terminals
        .iter()
        .filter(|t| t.state == ProcessState::Running)
        .collect();
    if let Some(t) = running.iter().find(|t| t.id == needle) {
        return Ok(t.id.clone());
    }
    let by_prefix: Vec<_> = running
        .iter()
        .filter(|t| t.id.starts_with(needle))
        .collect();
    let found: Vec<_> = if by_prefix.is_empty() {
        running
            .iter()
            .filter(|t| tab_title(w, &t.id) == needle)
            .collect()
    } else {
        by_prefix
    };
    match found.as_slice() {
        [one] => Ok(one.id.clone()),
        [] => Err(io::Error::other(format!("没有正在运行的终端匹配 {needle}"))),
        many => Err(io::Error::other(format!(
            "{needle} 匹配到 {} 个终端，请写更长的编号",
            many.len()
        ))),
    }
}

struct Args {
    socket: PathBuf,
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}
impl Args {
    fn parse(args: impl Iterator<Item = String>, valued: &[&str]) -> io::Result<Self> {
        let mut runtime = std::env::var_os("KEEPLINE_RUNTIME").map(PathBuf::from);
        let mut positional = vec![];
        let mut flags = vec![];
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            if arg == "--runtime" {
                runtime = Some(
                    args.next()
                        .ok_or_else(|| io::Error::other("--runtime 需要目录"))?
                        .into(),
                );
            } else if let Some(name) = arg.strip_prefix("--") {
                let value = if valued.contains(&name) {
                    Some(
                        args.next()
                            .ok_or_else(|| io::Error::other(format!("--{name} 需要一个值")))?,
                    )
                } else {
                    None
                };
                flags.push((name.to_owned(), value));
            } else {
                positional.push(arg);
            }
        }
        let runtime = match runtime {
            Some(r) => r,
            // Where the app keeps its host, so a script needs no flag next to it.
            None => PathBuf::from(
                std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?,
            )
            .join(".local/share/keepline-workbench"),
        };
        Ok(Self {
            socket: runtime.join("workbench.sock"),
            positional,
            flags,
        })
    }
    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
    }
    fn number(&self, name: &str, default: u64) -> io::Result<u64> {
        self.value(name)
            .map(|v| {
                v.parse()
                    .map_err(|_| io::Error::other(format!("--{name} 需要数字")))
            })
            .unwrap_or(Ok(default))
    }
    fn arg(&self, i: usize, what: &str) -> io::Result<&str> {
        self.positional
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| io::Error::other(format!("缺少{what}\n{USAGE}")))
    }
    /// `-` reads the text from standard input, so long prompts need no shell quoting.
    fn text(&self, i: usize) -> io::Result<String> {
        let text = self.arg(i, "文字")?;
        if text != "-" {
            return Ok(self.positional[i..].join(" "));
        }
        let mut s = String::new();
        io::stdin().take(64 * 1024 + 1).read_to_string(&mut s)?;
        Ok(s.trim_end_matches('\n').to_owned())
    }
}
fn workspace(socket: &Path) -> io::Result<Workspace> {
    match rpc(socket, &Request::Workspace)? {
        Reply::Workspace(w) => Ok(w),
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    }
}
fn expect_ok(reply: Reply) -> io::Result<()> {
    match reply {
        Reply::Ok => Ok(()),
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    }
}

pub fn run(command: &str, args: impl Iterator<Item = String>) -> io::Result<()> {
    match command {
        "list" => list(Args::parse(args, &[])?),
        "read" => read(Args::parse(args, &["lines"])?),
        "send" => send(Args::parse(args, &[])?),
        "prompt" => prompt(Args::parse(args, &["timeout"])?),
        "wait" => wait_cmd(Args::parse(args, &["until", "timeout"])?),
        _ => Err(io::Error::other(USAGE)),
    }
}
fn list(a: Args) -> io::Result<()> {
    let w = workspace(&a.socket)?;
    let rows: Vec<_> = w
        .terminals
        .iter()
        .filter(|t| t.state == ProcessState::Running)
        .map(|t| {
            let s = w.current_agent(&t.id);
            (t, s, phase(&w, &t.id), tab_title(&w, &t.id))
        })
        .collect();
    if a.has("json") {
        let json: Vec<_> = rows
            .iter()
            .map(|(t, s, phase, tab)| {
                serde_json::json!({
                    "id": t.id, "cwd": t.cwd, "tab": tab,
                    "agent": s.map(|s| agent_label(s.runtime)),
                    "phase": phase.name(),
                    "session": s.map(|s| &s.cli_session_id),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json).map_err(io::Error::other)?
        );
        return Ok(());
    }
    for (t, s, phase, tab) in rows {
        let agent = s.map_or_else(|| "-".into(), |s| agent_label(s.runtime));
        println!(
            "{:<8}  {:<6}  {:<7}  {}  {}",
            &t.id[..t.id.len().min(8)],
            agent,
            phase.name(),
            tab,
            t.cwd
        );
    }
    Ok(())
}
fn read(a: Args) -> io::Result<()> {
    let w = workspace(&a.socket)?;
    let id = resolve(&w, a.arg(0, "终端")?)?;
    let rows = a.number("lines", 50)? as usize;
    match rpc(
        &a.socket,
        &Request::ReadScreen {
            id,
            rows: rows + 200,
        },
    )? {
        Reply::Text(text) => {
            // The screen's empty rows below the cursor say nothing.
            let lines: Vec<_> = text.trim_end().lines().collect();
            println!("{}", lines[lines.len().saturating_sub(rows)..].join("\n"));
            Ok(())
        }
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    }
}
fn send(a: Args) -> io::Result<()> {
    let w = workspace(&a.socket)?;
    let id = resolve(&w, a.arg(0, "终端")?)?;
    let text = a.text(1)?;
    expect_ok(rpc(
        &a.socket,
        &Request::SendText {
            id,
            text,
            submit: a.has("enter"),
        },
    )?)
}
fn prompt(a: Args) -> io::Result<()> {
    let w = workspace(&a.socket)?;
    let id = resolve(&w, a.arg(0, "终端")?)?;
    let text = a.text(1)?;
    match phase(&w, &id) {
        Phase::None => {
            return Err(io::Error::other(
                "这个终端里没有可识别的 Agent 对话；发给 shell 请用 send",
            ))
        }
        // Typing into an approval or a choice would answer it: herdr refuses too.
        Phase::Waiting if !a.has("force") => {
            return Err(io::Error::other(
                "Agent 正在等你回答（审批或选择题），没有发送；确认要发请加 --force",
            ));
        }
        _ => (),
    }
    let started = Instant::now();
    expect_ok(rpc(
        &a.socket,
        &Request::SendText {
            id: id.clone(),
            text,
            submit: true,
        },
    )?)?;
    if a.has("wait") {
        let timeout = Duration::from_secs(a.number("timeout", 1800)?);
        let done = wait(
            &a.socket,
            &id,
            &[Phase::Idle, Phase::Waiting],
            timeout,
            started,
        )?;
        println!("{}", done.name());
    }
    Ok(())
}
fn wait_cmd(a: Args) -> io::Result<()> {
    let w = workspace(&a.socket)?;
    let id = resolve(&w, a.arg(0, "终端")?)?;
    let target = match a.value("until").unwrap_or("idle") {
        "idle" => Phase::Idle,
        "waiting" => Phase::Waiting,
        "working" => Phase::Working,
        other => {
            return Err(io::Error::other(format!(
                "--until 只接受 idle|waiting|working，不是 {other}"
            )))
        }
    };
    let timeout = Duration::from_secs(a.number("timeout", 1800)?);
    let done = wait(&a.socket, &id, &[target], timeout, Instant::now())?;
    println!("{}", done.name());
    Ok(())
}
/// Polls until the Agent reaches one of `targets`. Right after a prompt the state may
/// still read idle from before it, so idle only counts once the turn started or after
/// a grace period with no sign of work.
fn wait(
    socket: &Path,
    id: &str,
    targets: &[Phase],
    timeout: Duration,
    since: Instant,
) -> io::Result<Phase> {
    const GRACE: Duration = Duration::from_secs(20);
    let since_secs = epoch_secs(since);
    let mut started = !targets.contains(&Phase::Idle);
    loop {
        let w = workspace(socket)?;
        if !w
            .terminals
            .iter()
            .any(|t| t.id == id && t.state == ProcessState::Running)
        {
            return Err(io::Error::other("终端已退出"));
        }
        let now = phase(&w, id);
        started |= now == Phase::Working
            || w.current_agent(id).is_some_and(|s| {
                w.content
                    .get(&s.id)
                    .is_some_and(|c| c.updated >= since_secs)
            });
        if targets.contains(&now) && (now != Phase::Idle || started || since.elapsed() >= GRACE) {
            return Ok(now);
        }
        if since.elapsed() >= timeout {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("等待超时，当前 {}", now.name()),
            ));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
/// `at` in whole seconds since the epoch, the unit content cards record `updated` in.
fn epoch_secs(at: Instant) -> u64 {
    std::time::SystemTime::now()
        .checked_sub(at.elapsed())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepline_core::agent::Agent;
    fn fixture() -> Workspace {
        let mut w = Workspace::default();
        for (id, state) in [
            ("aaaa1111", ProcessState::Running),
            ("aaaa2222", ProcessState::Running),
            ("bbbb", ProcessState::Exited),
        ] {
            w.terminals.push(Terminal {
                id: id.into(),
                space: "s".into(),
                argv: vec![],
                cwd: "/tmp".into(),
                pid: None,
                state,
                exit_code: None,
                error: None,
            });
        }
        w.tabs.push(Tab {
            id: "t".into(),
            space: "s".into(),
            title: "review".into(),
            layout: Some(Layout::Leaf("aaaa2222".into())),
            default_task: None,
        });
        w.agent_sessions.push(AgentSession {
            id: "s1".into(),
            runtime: Agent::all()[0],
            cli_session_id: "c1".into(),
            terminal: Some("aaaa1111".into()),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: 1,
            cwd: "/tmp".into(),
            transcript: "".into(),
            name: String::new(),
            binding: None,
        });
        w
    }
    #[test]
    fn terminals_resolve_by_unique_prefix_or_tab_title_among_running_ones() {
        let w = fixture();
        assert_eq!(resolve(&w, "aaaa1").unwrap(), "aaaa1111");
        assert_eq!(resolve(&w, "review").unwrap(), "aaaa2222");
        assert!(resolve(&w, "aaaa").is_err(), "ambiguous");
        assert!(resolve(&w, "bbbb").is_err(), "exited");
    }
    #[test]
    fn phase_follows_the_current_conversation() {
        let mut w = fixture();
        assert_eq!(phase(&w, "aaaa2222"), Phase::None);
        assert_eq!(phase(&w, "aaaa1111"), Phase::Idle);
        let mut c = keepline_core::content::Content {
            turn: Turn::Waiting,
            ..Default::default()
        };
        w.content.insert("s1".into(), c.clone());
        assert_eq!(phase(&w, "aaaa1111"), Phase::Waiting);
        c.turn = Turn::Running;
        w.content.insert("s1".into(), c);
        assert_eq!(phase(&w, "aaaa1111"), Phase::Working);
    }
}
