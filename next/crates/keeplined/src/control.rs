//! Driving Keepline terminals from scripts and other Agents, after 同类终端工具's `pane` and
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
keeplined explain --file <画面.txt> --agent <claude|codex> [--json]
keeplined explain <终端> [--json]            画面规则怎么读这块屏：状态、命中的规则、规则版本和来源、看到的行
keeplined events [--json]                    先列出现在的状态，再每有变化输出一行，直到 Ctrl-C：
                                             started / waiting / error / done、approval / approval_gone、
                                             terminal_started / terminal_exited、command_done / command_failed；
                                             lagged 表示跟不上丢了事件，应重新开一次取现状
keeplined split <终端> [--down] [--size 0.5] [--tab|--here] [--cwd 目录] [--json] [-- 命令…]
                                             在这个终端右边（--down 下面）开一块新分屏，打印新终端编号；
                                             标签已有 4 块或切开太窄时开到「旁屏」标签（--here 改为拒绝，
                                             --tab 直接开到旁屏）；给了命令就在新 shell 里跑，跑完留在 shell
keeplined close <终端>                       关掉这块分屏并结束里面的程序
keeplined remote --stdio                     给另一台机器或手机 App 用：一行一个 JSON 请求和回复（看画面、打字、按键、
                                             答审批、事件），经 ssh 连过来：ssh 主机 keeplined remote --stdio
keeplined remote --relay <中继地址> [--relay-key <密钥>]
                                             连上中继并一直挂着，配对过的手机在哪都能连过来；内容端到端加密，
                                             中继看不到。中继要密钥时也可以设 KEEPLINE_RELAY_KEY
keeplined remote --pair --relay <中继地址> [--new-code]
                                             打印配对二维码和链接；--new-code 换一个配对码，已配对的手机全部失效
keeplined --skill                            给 Coding Agent 看的用法说明（Markdown）
<终端> 可以是编号的前几位或标签名；全部命令都接受 --runtime <目录>。
Agent 藏在包装器（沙盒、虚拟机、nono 等）后面认不出时，启动前设 KEEPLINE_AGENT=<claude|codex|…>。";
/// `keeplined --skill`: how a coding Agent drives these commands, and when it must not.
pub const SKILL: &str = include_str!("skill.md");

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
    pub(crate) fn name(self) -> &'static str {
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
pub(crate) fn agent_label(agent: keepline_core::agent::Agent) -> String {
    agent.key().to_lowercase()
}
pub(crate) fn tab_title(w: &Workspace, terminal: &str) -> String {
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

pub(crate) struct Args {
    pub(crate) socket: PathBuf,
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
    /// Everything after a bare `--`, untouched: a command and its own flags.
    rest: Vec<String>,
}
impl Args {
    pub(crate) fn parse(args: impl Iterator<Item = String>, valued: &[&str]) -> io::Result<Self> {
        let mut runtime = std::env::var_os("KEEPLINE_RUNTIME").map(PathBuf::from);
        let mut positional = vec![];
        let mut flags = vec![];
        let mut rest = vec![];
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            if arg == "--" {
                rest.extend(args.by_ref());
            } else if arg == "--runtime" {
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
            rest,
        })
    }
    pub(crate) fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }
    pub(crate) fn value(&self, name: &str) -> Option<&str> {
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
        "explain" => explain(Args::parse(args, &["file", "agent"])?),
        "events" => events(Args::parse(args, &[])?),
        "split" => split(Args::parse(args, &["size", "cwd"])?),
        "close" => close(Args::parse(args, &[])?),
        _ => Err(io::Error::other(USAGE)),
    }
}
/// 同类终端工具's `agent explain`: replays a saved screen, or a live terminal's, through the
/// screen rules and says what they decided and from what.
fn explain(a: Args) -> io::Result<()> {
    use keeplined::screen_rules;
    let (agent, screen, daemon) = match a.value("file") {
        Some(file) => {
            let name = a
                .value("agent")
                .ok_or_else(|| io::Error::other(format!("--file 要配 --agent\n{USAGE}")))?;
            let agent = keepline_core::agent::Agent::all()
                .into_iter()
                .find(|g| agent_label(*g) == name.to_lowercase())
                .ok_or_else(|| io::Error::other(format!("不认识的 Agent：{name}")))?;
            (agent, std::fs::read_to_string(file)?, None)
        }
        None => {
            let w = workspace(&a.socket)?;
            let id = resolve(&w, a.arg(0, "终端或 --file")?)?;
            let s = w
                .current_agent(&id)
                .ok_or_else(|| io::Error::other("这个终端里没有可识别的 Agent 对话"))?;
            // The daemon reads the visible rows; 200 covers the tallest screen it allows.
            let screen = match rpc(&a.socket, &Request::ReadScreen { id, rows: 200 })? {
                Reply::Text(text) => text,
                other => return Err(io::Error::other(format!("unexpected reply: {other:?}"))),
            };
            (s.runtime, screen, w.content.get(&s.id).cloned())
        }
    };
    let e = screen_rules::explain(screen_rules::rules(agent), &screen);
    if a.has("json") {
        let mut json = e.to_json();
        if let Some(c) = &daemon {
            json["daemon"] = serde_json::json!({
                "phase": format!("{:?}", c.turn),
                "signal": c.signal,
                "signal_detail": c.signal_detail,
                "screen_check": c.screen_check,
            });
        }
        let text = serde_json::to_string_pretty(&json).map_err(io::Error::other)?;
        println!("{text}");
        return Ok(());
    }
    println!("状态：{}", e.state());
    match e.rule() {
        Some(r) => println!("规则：{}（{}）", r.id, r.name),
        None => println!("规则：没有命中"),
    }
    let source = match &e.rules.source {
        screen_rules::Source::Bundled => "内置".to_owned(),
        screen_rules::Source::Override(path) => format!("覆盖文件 {}", path.display()),
    };
    println!("规则集：{} {}，{source}", e.rules.id, e.rules.version);
    match &e.found {
        Ok(hit) => {
            println!("问题：{}", hit.question);
            println!("看到的画面：");
            for line in &hit.evidence {
                println!("  {line}");
            }
        }
        Err(misses) => {
            println!("原因：{}", e.fallback());
            println!("各规则：");
            for (m, r) in misses.iter().zip(&e.rules.rules) {
                println!("  {} {}：{}", r.id, r.name, m.describe(r));
            }
        }
    }
    if let Some(c) = daemon {
        println!("守护进程现在：{:?}，依据 {}", c.turn, c.signal);
        for line in c.signal_detail.lines().chain(c.screen_check.lines()) {
            println!("  {line}");
        }
    }
    Ok(())
}
/// 同类终端工具's event subscription: the state now, then one line per change until the host
/// goes away or the reader stops reading.
fn events(a: Args) -> io::Result<()> {
    let mut stream = std::os::unix::net::UnixStream::connect(&a.socket)?;
    keepline_core::protocol::send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
    match receive::<Reply>(&mut stream)? {
        Reply::Hello { major: MAJOR, .. } => (),
        r => return Err(io::Error::other(format!("handshake: {r:?}"))),
    }
    keepline_core::protocol::send(&mut stream, CONTROL, &Request::Subscribe)?;
    let json = a.has("json");
    loop {
        let notices = match receive::<Reply>(&mut stream)? {
            Reply::Events(n) => n,
            Reply::Error(e) => return Err(io::Error::other(e)),
            other => return Err(io::Error::other(format!("unexpected reply: {other:?}"))),
        };
        for n in notices {
            if json {
                println!("{}", serde_json::to_string(&n).map_err(io::Error::other)?);
                continue;
            }
            let terminal: String = n
                .terminal
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(8)
                .collect();
            let what = [n.title.as_str(), n.detail.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let code = n
                .exit_code
                .map(|c| format!(" 退出码 {c}"))
                .unwrap_or_default();
            println!("{:<16} {terminal:<8} {what}{code}", n.event);
        }
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
    // Full ids: terminals opened by one app share a long prefix, so a cut-off id names none.
    let id_width = rows.iter().map(|r| r.0.id.len()).max().unwrap_or(0);
    let tab_width = rows
        .iter()
        .map(|r| r.3.chars().count().max(1))
        .max()
        .unwrap_or(1);
    for (t, s, phase, tab) in rows {
        let agent = s.map_or_else(|| "-".into(), |s| agent_label(s.runtime));
        let tab = if tab.is_empty() { "-".to_owned() } else { tab };
        let pad = " ".repeat(tab_width - tab.chars().count());
        println!(
            "{:<id_width$}  {:<6}  {:<7}  {tab}{pad}  {}",
            t.id,
            agent,
            phase.name(),
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
        // Typing into an approval or a choice would answer it: 同类终端工具 refuses too.
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
/// A tab holds at most this many panes opened from outside the app; past it, or when a
/// split would leave a half too small, the pane goes to a side tab instead.
const MAX_PANES: usize = 4;
/// Side tabs are named after the terminal they serve: `side~<terminal>`, then `~2`, `~3`.
const SIDE: &str = "side~";

/// Where a new pane goes: beside a pane, or into a side tab that does not exist yet.
enum Placement {
    Beside {
        target: String,
        axis: Axis,
        share: u16,
        cols: u16,
        rows: u16,
    },
    NewTab {
        id: String,
        title: String,
    },
}
fn split_room(socket: &Path, target: &str, axis: Axis, share: u16) -> io::Result<(u16, u16)> {
    match rpc(
        socket,
        &Request::SplitRoom {
            target: target.to_owned(),
            axis,
            share,
        },
    )? {
        Reply::Size { cols, rows } => Ok((cols, rows)),
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    }
}
/// The terminal a tab's side tabs belong to: its top-left pane, where the user started.
/// A side pane split again fills the same side tabs rather than starting a chain.
fn side_owner(w: &Workspace, tab: &Tab, terminal: &str) -> String {
    if let Some(owner) = tab.id.strip_prefix(SIDE).and_then(|r| r.split('~').next()) {
        return owner.to_owned();
    }
    w.tab_terminals(&tab.id)
        .into_iter()
        .next()
        .unwrap_or_else(|| terminal.to_owned())
}
/// The first side tab of `owner` with room for one more pane, beside its last pane,
/// right if that fits and down otherwise; or the next side tab to open.
fn side_placement(
    socket: &Path,
    w: &Workspace,
    owner: &str,
    asking: &str,
) -> io::Result<Placement> {
    // Named for the Agent asking, or the one where the user started.
    let name = [asking, owner]
        .iter()
        .find_map(|t| w.current_agent(t))
        .map(|s| s.runtime.name().to_owned())
        .unwrap_or_else(|| "终端".into());
    // 「Claude 的旁屏」, 「终端的旁屏」.
    let gap = if name.ends_with(|c: char| c.is_ascii()) {
        " "
    } else {
        ""
    };
    for n in 1.. {
        let id = if n == 1 {
            format!("{SIDE}{owner}")
        } else {
            format!("{SIDE}{owner}~{n}")
        };
        let Some(tab) = w.tabs.iter().find(|t| t.id == id) else {
            let title = if n == 1 {
                format!("{name}{gap}的旁屏")
            } else {
                format!("{name}{gap}的旁屏 {n}")
            };
            return Ok(Placement::NewTab { id, title });
        };
        let panes = w.tab_terminals(&tab.id);
        let Some(last) = panes.last().filter(|_| panes.len() < MAX_PANES) else {
            continue;
        };
        for axis in [Axis::Right, Axis::Down] {
            if let Ok((cols, rows)) = split_room(socket, last, axis, 5000) {
                return Ok(Placement::Beside {
                    target: last.clone(),
                    axis,
                    share: 5000,
                    cols,
                    rows,
                });
            }
        }
    }
    unreachable!("there is always a next side tab")
}

/// 同类终端工具's `pane split` and Rex's `new_split`: a new shell beside a terminal, in its
/// tab, in the directory that shell is in now. New panes only go right or down, so the
/// pane the user started in keeps the top-left corner. A tab that already has four panes,
/// or a split that would leave a half too small, sends the pane to a side tab named after
/// that terminal instead (`--here` refuses then; `--tab` always goes there). The
/// workbench shows it as soon as it next reads the host and never moves the keyboard or
/// the shown tab for it.
fn split(a: Args) -> io::Result<()> {
    if a.has("before") {
        return Err(io::Error::other(
            "--before 已去掉：新分屏只往右或往下开，原来那块保持在左上角",
        ));
    }
    if a.has("here") && a.has("tab") {
        return Err(io::Error::other("--here 和 --tab 只能选一个"));
    }
    let w = workspace(&a.socket)?;
    let target = resolve(&w, a.arg(0, "终端")?)?;
    let source = w
        .terminals
        .iter()
        .find(|t| t.id == target)
        .ok_or_else(|| io::Error::other("终端不存在"))?;
    let tab = w
        .terminal_tab(&target)
        .ok_or_else(|| io::Error::other("这个终端没有摆在任何画面里，不能在它旁边分屏"))?;
    let share = match a.value("size") {
        None => 5000,
        Some(v) => {
            let f: f64 = v
                .parse()
                .map_err(|_| io::Error::other("--size 需要 0 到 1 之间的小数"))?;
            if !(f > 0.0 && f < 1.0) {
                return Err(io::Error::other("--size 需要 0 到 1 之间的小数"));
            }
            ((f * 10000.0).round() as u16).clamp(1, 9999)
        }
    };
    let axis = if a.has("down") {
        Axis::Down
    } else {
        Axis::Right
    };
    // Asked before anything starts, so a refused split leaves nothing behind; a pane
    // beside another starts at the size it will show at.
    let panes = w.tab_terminals(&tab.id).len();
    let (placement, why) = if a.has("tab") {
        (None, "按 --tab".to_owned())
    } else if panes >= MAX_PANES {
        let why = format!("这个标签已经有 {panes} 块");
        if a.has("here") {
            return Err(io::Error::other(format!(
                "{why}，不再往里加；去掉 --here 会开在旁屏标签"
            )));
        }
        (None, why)
    } else {
        match split_room(&a.socket, &target, axis, share) {
            Ok((cols, rows)) => (
                Some(Placement::Beside {
                    target: target.clone(),
                    axis,
                    share,
                    cols,
                    rows,
                }),
                String::new(),
            ),
            Err(e) if a.has("here") => return Err(e),
            Err(e) => (None, e.to_string()),
        }
    };
    let overflow = placement.is_none();
    let placement = match placement {
        Some(p) => p,
        None => side_placement(&a.socket, &w, &side_owner(&w, tab, &target), &target)?,
    };
    let cwd = match a.value("cwd") {
        Some(dir) => std::fs::canonicalize(dir)
            .map_err(|e| io::Error::other(format!("--cwd {dir}：{e}")))?
            .to_string_lossy()
            .into_owned(),
        // Where that shell is now, as the workbench's own split does.
        None => match rpc(&a.socket, &Request::TerminalCwd { id: target.clone() }) {
            Ok(Reply::Cwd(dir)) if Path::new(&dir).is_dir() => dir,
            _ => source.cwd.clone(),
        },
    };
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/zsh".into());
    let argv = split_argv(&shell, &a.rest);
    let id = format!(
        "split-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    // A new tab's pane is sized by the workbench once that tab is shown.
    let (cols, rows) = match &placement {
        Placement::Beside { cols, rows, .. } => (*cols, *rows),
        Placement::NewTab { .. } => (100, 30),
    };
    match rpc(
        &a.socket,
        &Request::Launch {
            id: id.clone(),
            space: source.space.clone(),
            argv,
            cwd,
            cols,
            rows,
            scheme: Default::default(),
        },
    )? {
        Reply::Workspace(_) => (),
        other => return Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    }
    let (edit, beside) = match placement {
        Placement::Beside {
            target,
            axis,
            share,
            ..
        } => (
            Edit::SplitPane {
                target: target.clone(),
                terminal: id.clone(),
                axis,
                before: false,
                share,
            },
            Some(target),
        ),
        Placement::NewTab { id: tab, title } => (
            Edit::PutTab {
                tab: Tab {
                    id: tab,
                    space: source.space.clone(),
                    default_task: None,
                    title,
                    layout: Some(Layout::Leaf(id.clone())),
                },
            },
            None,
        ),
    };
    if let Err(e) = rpc(&a.socket, &Request::Edit { revision: 0, edit }) {
        // Nothing shows it, so nothing would ever stop it.
        let _ = rpc(&a.socket, &Request::Stop { id });
        return Err(e);
    }
    let after = workspace(&a.socket)?;
    let placed = after.terminal_tab(&id);
    let title = placed.map(|t| t.title.clone()).unwrap_or_default();
    if a.has("json") {
        println!(
            "{}",
            serde_json::json!({
                "id": id,
                "tab": placed.map(|t| &t.id),
                "tab_title": title,
                "beside": beside,
                "overflow": overflow.then_some(&why),
            })
        );
    } else {
        println!("{id}");
        if overflow {
            // On stderr, so `id=$(keeplined split …)` still captures only the id.
            eprintln!("{why}，开在了「{title}」标签（不切换用户的画面）");
        }
    }
    Ok(())
}
/// A login shell, or one that runs `command` first and stays open after it, the way a
/// resumed conversation starts, so its output stays readable and the pane stays usable.
fn split_argv(shell: &str, command: &[String]) -> Vec<String> {
    let command = match command {
        [] => return vec![shell.into(), "-l".into()],
        // One argument is a command line already, `&&` and pipes included.
        [line] => line.clone(),
        words => words
            .iter()
            .map(|w| shell_quote(w))
            .collect::<Vec<_>>()
            .join(" "),
    };
    let then = format!("{command}; exec {} -l", shell_quote(shell));
    vec![shell.into(), "-l".into(), "-i".into(), "-c".into(), then]
}
/// Closes a pane as its × does: the tab goes with its last pane, the program ends, and
/// `events` reports `terminal_exited`.
fn close(a: Args) -> io::Result<()> {
    let w = workspace(&a.socket)?;
    let needle = a.arg(0, "终端")?;
    // An exited pane still shows; close it by its full id.
    let id = match w.terminal_tab(needle) {
        Some(_) => needle.to_owned(),
        None => resolve(&w, needle)?,
    };
    if w.terminal_tab(&id).is_none() {
        return Err(io::Error::other("这个终端没有摆在任何画面里"));
    }
    match rpc(
        &a.socket,
        &Request::Edit {
            revision: 0,
            edit: Edit::ClosePane { terminal: id },
        },
    )? {
        Reply::Workspace(_) => Ok(()),
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
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
    fn the_skill_names_the_marker_and_every_command() {
        assert!(SKILL.contains("KEEPLINE_ENV"));
        for command in [
            "list", "read", "send", "prompt", "wait", "explain", "split", "close",
        ] {
            assert!(
                SKILL.contains(&format!("keeplined {command} ")),
                "{command}"
            );
        }
        for flag in [
            "--force", "--wait", "--until", "--enter", "--lines", "--tab", "--here",
        ] {
            assert!(SKILL.contains(flag), "{flag}");
        }
    }
    #[test]
    fn a_split_runs_its_command_in_a_shell_that_stays_open() {
        assert_eq!(split_argv("/bin/zsh", &[]), ["/bin/zsh", "-l"]);
        let one = split_argv("/bin/zsh", &["npm test && echo ok".into()]);
        assert_eq!(one[4], "npm test && echo ok; exec '/bin/zsh' -l");
        let words = split_argv("/bin/zsh", &["echo".into(), "a b".into()]);
        assert_eq!(&words[..4], ["/bin/zsh", "-l", "-i", "-c"]);
        assert_eq!(words[4], "'echo' 'a b'; exec '/bin/zsh' -l");
    }
    #[test]
    fn everything_after_a_bare_double_dash_is_the_command() {
        let a = Args::parse(
            ["t", "--down", "--size", "0.3", "--", "ls", "--all"]
                .into_iter()
                .map(String::from),
            &["size"],
        )
        .unwrap();
        assert_eq!(a.positional, ["t"]);
        assert!(a.has("down"));
        assert_eq!(a.value("size"), Some("0.3"));
        assert_eq!(a.rest, ["ls", "--all"]);
        assert!(!a.has("all"));
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
