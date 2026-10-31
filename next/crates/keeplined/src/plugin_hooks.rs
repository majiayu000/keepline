//! Runs the plugins' hooks (`keepline_core::hook`) when a session in a Keepline terminal
//! enters a state, or a command typed in one finishes. The host calls `observe` after every
//! commit and `command` when a terminal's shell marks a command finished; with no hook
//! installed both return at once. Each hook runs on its own thread under its timeout, so a slow or broken
//! hook costs the host nothing; outcomes go to the host log.
use keepline_core::content::{now, short, Inbox, Turn};
use keepline_core::hook::{self, Event, Hook, Payload};
use keepline_core::model::Workspace;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Hooks still running; past this, new events are dropped and logged, not queued.
const MAX_IN_FLIGHT: usize = 8;
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

/// The state each session was last seen in, so a hook fires on entering it, once.
#[derive(Default)]
pub struct Hooks {
    last: HashMap<String, String>,
    /// The first look only records: a host restart is not every session changing state.
    seeded: bool,
}

/// `done` is any turn that ended, claimed finished or not: paused is still yours to look at.
fn event(group: Inbox, turn: Turn) -> Option<Event> {
    match group {
        Inbox::Running => Some(Event::Started),
        Inbox::Waiting => Some(Event::Waiting),
        Inbox::Error => Some(Event::Error),
        Inbox::Review => Some(Event::Done),
        Inbox::Paused if matches!(turn, Turn::Completed | Turn::Interrupted) => Some(Event::Done),
        _ => None,
    }
}

impl Hooks {
    pub fn seeded(&self) -> bool {
        self.seeded
    }
    pub fn observe(&mut self, w: &Workspace) {
        if hook::all().is_empty() {
            return;
        }
        let fired = self.changes(w);
        for p in fired {
            for h in hook::all().iter().filter(|h| h.on.contains(&p.event)) {
                spawn(h, p.clone());
            }
        }
    }
    /// The sessions that entered a hookable state since the last look.
    pub fn changes(&mut self, w: &Workspace) -> Vec<Payload> {
        let at = now();
        let mut seen = HashMap::new();
        let mut fired = vec![];
        for s in w.agent_sessions.iter().filter(|s| s.managed()) {
            let Some(c) = w.content.get(&s.id) else {
                continue;
            };
            if c.hidden_for(s).is_some() {
                continue;
            }
            let Some(e) = event(c.group(s, w, at), c.turn) else {
                continue;
            };
            // One turn is one start and one finish; one dialog is one wait however often
            // its text is refined; an error is keyed by its text.
            let detail = match e {
                Event::Waiting => c.waiting.clone(),
                Event::Error => c.error.clone(),
                Event::Done => c.last_reply.clone(),
                Event::Started | Event::CommandDone | Event::CommandFailed => String::new(),
            };
            let key = match e {
                Event::Waiting => {
                    format!("{}:{}:{}", c.turn_key, c.turn_started, c.waiting_episode)
                }
                Event::Error => format!("{}:{}:{}", c.turn_key, c.turn_started, c.error),
                _ => format!("{}:{}", c.turn_key, c.turn_started),
            };
            let key = format!("{}:{key}", e.name());
            if self.seeded && self.last.get(&s.id) != Some(&key) {
                let space = s
                    .terminal
                    .as_ref()
                    .and_then(|t| w.terminals.iter().find(|x| &x.id == t))
                    .and_then(|t| w.spaces.iter().find(|p| p.id == t.space))
                    .map(|p| p.name.clone());
                fired.push(Payload {
                    event: e,
                    session: s.id.clone(),
                    agent: s.runtime.name().to_owned(),
                    title: short(&c.title, 120),
                    cwd: s.cwd.clone(),
                    space,
                    detail: short(
                        detail
                            .lines()
                            .map(str::trim)
                            .find(|l| !l.is_empty())
                            .unwrap_or(""),
                        500,
                    ),
                    at,
                    terminal: None,
                    exit_code: None,
                    duration_ms: None,
                });
            }
            seen.insert(s.id.clone(), key);
        }
        self.last = seen;
        self.seeded = true;
        fired
    }
}

/// A command finished in a terminal: the hooks that listen for it and its duration.
pub fn command(p: Payload) {
    let took = p.duration_ms.unwrap_or(0);
    for h in hook::all()
        .iter()
        .filter(|h| h.on.contains(&p.event) && took >= h.min_ms)
    {
        spawn(h, p.clone());
    }
}

fn spawn(h: &Hook, p: Payload) {
    if IN_FLIGHT.fetch_add(1, Ordering::SeqCst) >= MAX_IN_FLIGHT {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        eprintln!(
            "keeplined: hook {} skipped: {MAX_IN_FLIGHT} hooks still running",
            h.id
        );
        return;
    }
    let h = h.clone();
    std::thread::spawn(move || {
        let result = run(&h, &p);
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        if let Err(e) = result {
            eprintln!("keeplined: hook {} ({}) failed: {e}", h.id, p.event.name());
        }
    });
}

/// Runs one hook to completion or its timeout, the event on stdin.
pub fn run(h: &Hook, p: &Payload) -> Result<(), String> {
    let body = serde_json::to_vec(p).map_err(|e| e.to_string())?;
    let mut cmd = match (&h.command, &h.webhook) {
        (Some(c), _) => {
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg(c).envs(p.env());
            cmd
        }
        (None, Some(url)) => {
            // curl ships with macOS; its own limit backs up ours.
            let secs = h.timeout_ms.div_ceil(1000).to_string();
            let mut cmd = Command::new("/usr/bin/curl");
            cmd.args(["-sS", "-f", "-m", &secs, "-X", "POST"])
                .args([
                    "-H",
                    "Content-Type: application/json",
                    "--data-binary",
                    "@-",
                ])
                .arg(url);
            cmd
        }
        (None, None) => return Err("没有 command 也没有 webhook".into()),
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动不了：{e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A hook that ignores stdin closes it early; that is not a failure.
        let _ = stdin.write_all(&body);
    }
    let deadline = Instant::now() + Duration::from_millis(h.timeout_ms);
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                let mut err = String::new();
                if let Some(mut e) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut e, &mut err);
                }
                return Err(format!("{status} {}", short(err.trim(), 200)));
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("超过 {} ms，已停止", h.timeout_ms));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn hook(command: &str, timeout_ms: u64) -> Hook {
        Hook {
            id: "t".into(),
            on: vec![Event::Waiting],
            command: Some(command.into()),
            webhook: None,
            timeout_ms,
            min_ms: 0,
            file: PathBuf::new(),
        }
    }
    fn payload() -> Payload {
        Payload {
            event: Event::Waiting,
            session: "s1".into(),
            agent: "Codex".into(),
            title: "修 bug; rm -rf /".into(),
            cwd: "/tmp".into(),
            space: Some("lib".into()),
            detail: "是否允许推送？".into(),
            at: 1,
            terminal: None,
            exit_code: None,
            duration_ms: None,
        }
    }

    #[test]
    fn a_command_hook_gets_the_event_on_stdin_and_in_its_environment() {
        let out = std::env::temp_dir().join(format!("kl-hook-out-{}", std::process::id()));
        let cmd = format!(
            "cat > {0}; printf '\\n%s|%s' \"$KEEPLINE_EVENT\" \"$KEEPLINE_TITLE\" >> {0}",
            out.display()
        );
        run(&hook(&cmd, 5_000), &payload()).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        let (json, env) = text.split_once('\n').unwrap();
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(v["event"], "waiting");
        assert_eq!(v["space"], "lib");
        // The title stays data: it reaches the hook verbatim and runs nothing.
        assert_eq!(env, "waiting|修 bug; rm -rf /");
        let _ = std::fs::remove_file(out);
    }

    #[test]
    fn each_state_reaches_hooks_once_and_a_restart_replays_nothing() {
        use keepline_core::content::Content;
        use keepline_core::model::{AgentSession, Outcome};
        let mut w = Workspace::default();
        w.agent_sessions.push(AgentSession {
            id: "s".into(),
            runtime: keepline_core::model::Agent::Codex,
            cli_session_id: "cli".into(),
            terminal: Some("pty".into()),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: now(),
            cwd: "/project".into(),
            transcript: "log".into(),
            binding: None,
            name: String::new(),
        });
        let content = |turn, key: &str, waiting: &str, episode| Content {
            interactive: true,
            title: "修复登录".into(),
            turn,
            turn_key: key.into(),
            turn_started: now(),
            waiting: waiting.into(),
            waiting_episode: episode,
            last_reply: "改好了\n细节".into(),
            updated: now(),
            ..Default::default()
        };
        let mut hooks = Hooks::default();
        w.content
            .insert("s".into(), content(Turn::Waiting, "t1", "推送吗？", 1));
        assert!(hooks.changes(&w).is_empty(), "the first look only records");
        assert!(hooks.changes(&w).is_empty());
        // A refined question in the same dialog is not a new event; a new dialog is.
        w.content.insert(
            "s".into(),
            content(Turn::Waiting, "t1", "推送到 main 吗？", 1),
        );
        assert!(hooks.changes(&w).is_empty());
        w.content
            .insert("s".into(), content(Turn::Waiting, "t1", "再确认？", 2));
        let fired = hooks.changes(&w);
        assert_eq!(fired.len(), 1);
        assert_eq!(
            (fired[0].event, fired[0].detail.as_str()),
            (Event::Waiting, "再确认？")
        );
        w.content
            .insert("s".into(), content(Turn::Running, "t2", "", 2));
        assert_eq!(hooks.changes(&w)[0].event, Event::Started);
        w.content
            .insert("s".into(), content(Turn::Completed, "t2", "", 2));
        let fired = hooks.changes(&w);
        assert_eq!(
            (fired[0].event, fired[0].detail.as_str()),
            (Event::Done, "改好了")
        );
        assert!(hooks.changes(&w).is_empty());
    }

    #[test]
    fn a_slow_or_failing_hook_is_stopped_and_reported() {
        let started = Instant::now();
        let err = run(&hook("sleep 5", 200), &payload()).unwrap_err();
        assert!(err.contains("200 ms"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(2));
        let err = run(&hook("echo nope >&2; exit 3", 2_000), &payload()).unwrap_err();
        assert!(err.contains("nope"), "{err}");
    }
}
