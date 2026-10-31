//! `keeplined events`: the sessions' states, then each change, to whoever subscribed.
//! The state changes are the plugin hooks' own (`plugin_hooks::Hooks`), tracked apart so a
//! subscriber coming or going never moves what the hooks have seen. With nobody
//! subscribed nothing here runs.
use super::*;
use keepline_core::hook::Notice;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

/// Someone is subscribed: terminals read their shells' command marks even when no hook
/// listens for commands. A reader holds its terminal's lock and cannot take the host's.
pub(crate) static SUBSCRIBED: AtomicBool = AtomicBool::new(false);

/// What subscribers were last told, to send only what changed.
#[derive(Default)]
pub(super) struct Tracker {
    states: crate::plugin_hooks::Hooks,
    /// Session -> the approval question its screen shows.
    approvals: HashMap<String, String>,
    running: HashSet<String>,
}
fn approvals(w: &Workspace) -> HashMap<String, String> {
    w.agent_sessions
        .iter()
        .filter(|s| s.current)
        .filter_map(|s| {
            let c = w.content.get(&s.id)?;
            (!c.approval.is_empty()).then(|| (s.id.clone(), c.approval.clone()))
        })
        .collect()
}
fn running(w: &Workspace) -> HashSet<String> {
    w.terminals
        .iter()
        .filter(|t| t.state == ProcessState::Running)
        .map(|t| t.id.clone())
        .collect()
}
fn session_notice(event: &str, w: &Workspace, id: &str, detail: &str, at: u64) -> Notice {
    let mut n = Notice::named(event, at);
    n.session = id.to_owned();
    n.detail = detail.to_owned();
    if let Some(s) = w.agent_sessions.iter().find(|s| s.id == id) {
        n.agent = s.runtime.name().to_owned();
        n.cwd = s.cwd.clone();
        n.terminal = s.terminal.clone();
        n.title = w
            .content
            .get(id)
            .map(|c| keepline_core::content::short(&c.title, 120))
            .unwrap_or_default();
        n.space = space_of(w, s.terminal.as_deref());
    }
    n
}
fn space_of(w: &Workspace, terminal: Option<&str>) -> Option<String> {
    let t = w.terminals.iter().find(|t| Some(t.id.as_str()) == terminal)?;
    w.spaces.iter().find(|p| p.id == t.space).map(|p| p.name.clone())
}
fn terminal_notice(event: &str, w: &Workspace, id: &str, at: u64) -> Notice {
    let mut n = Notice::named(event, at);
    n.terminal = Some(id.to_owned());
    if let Some(t) = w.terminals.iter().find(|t| t.id == id) {
        n.cwd = t.cwd.clone();
    }
    n.space = space_of(w, Some(id));
    n
}
impl Tracker {
    /// Starts from `w`: the first subscriber gets it as its snapshot, not as changes.
    pub(super) fn seed(&mut self, w: &Workspace) {
        self.states = Default::default();
        self.states.changes(w);
        self.approvals = approvals(w);
        self.running = running(w);
    }
    /// The state now: every running terminal, every session in a state a hook would
    /// name, and every approval on screen.
    pub(super) fn snapshot(w: &Workspace) -> Vec<Notice> {
        let at = keepline_core::content::now();
        let mut out: Vec<Notice> = {
            let mut ids: Vec<_> = running(w).into_iter().collect();
            ids.sort();
            ids.iter().map(|id| terminal_notice("terminal_started", w, id, at)).collect()
        };
        out.extend(
            crate::plugin_hooks::Hooks::current(w)
                .into_iter()
                .map(Notice::from),
        );
        let mut asks: Vec<_> = approvals(w).into_iter().collect();
        asks.sort();
        out.extend(asks.iter().map(|(id, q)| session_notice("approval", w, id, q, at)));
        out
    }
    pub(super) fn changes(&mut self, w: &Workspace) -> Vec<Notice> {
        let at = keepline_core::content::now();
        let mut out = vec![];
        let now_running = running(w);
        let mut started: Vec<_> = now_running.difference(&self.running).cloned().collect();
        started.sort();
        out.extend(started.iter().map(|id| terminal_notice("terminal_started", w, id, at)));
        out.extend(self.states.changes(w).into_iter().map(Notice::from));
        let now_asking = approvals(w);
        let mut gone: Vec<_> = self
            .approvals
            .iter()
            .filter(|(id, q)| now_asking.get(*id) != Some(*q))
            .map(|(id, q)| (id.clone(), q.clone()))
            .collect();
        gone.sort();
        out.extend(gone.iter().map(|(id, q)| session_notice("approval_gone", w, id, q, at)));
        let mut shown: Vec<_> = now_asking
            .iter()
            .filter(|(id, q)| self.approvals.get(*id) != Some(*q))
            .collect();
        shown.sort();
        out.extend(shown.iter().map(|(id, q)| session_notice("approval", w, id, q, at)));
        let mut ended: Vec<_> = self.running.difference(&now_running).cloned().collect();
        ended.sort();
        out.extend(ended.iter().map(|id| terminal_notice("terminal_exited", w, id, at)));
        self.approvals = now_asking;
        self.running = now_running;
        out
    }
}

impl Host {
    /// Queues `notices` for every subscriber; one that has fallen behind is marked and
    /// told `lagged` once it catches up, never waited for.
    pub(super) fn send_events(&self, notices: Vec<Notice>) {
        if notices.is_empty() || self.event_subscribers.is_empty() {
            return;
        }
        let Ok(bytes) = serde_json::to_vec(&Reply::Events(notices)) else {
            return;
        };
        let frame = Arc::new(bytes);
        for s in self.event_subscribers.values() {
            s.offer((CONTROL, frame.clone()));
        }
    }
}

pub(super) fn subscribe(mut stream: UnixStream, shared: Shared, connection: u64) -> io::Result<()> {
    let (tx, rx) = sync_channel::<Frame>(256);
    let lagged = Arc::new(AtomicBool::new(false));
    {
        let mut host = lock(&shared)?;
        let w = host.snapshot();
        if host.event_subscribers.is_empty() {
            host.events.seed(&w);
        }
        let first = serde_json::to_vec(&Reply::Events(Tracker::snapshot(&w)))
            .map_err(io::Error::other)?;
        tx.send((CONTROL, Arc::new(first))).map_err(io::Error::other)?;
        host.event_subscribers.insert(
            connection,
            Subscriber {
                tx,
                socket: stream.try_clone()?,
                lagged: Arc::clone(&lagged),
            },
        );
        SUBSCRIBED.store(true, Ordering::Release);
    }
    let mut writer = stream.try_clone()?;
    thread::spawn(move || {
        for (kind, bytes) in rx {
            if write_frame(&mut writer, kind, &bytes).is_err() {
                break;
            }
            // Dropped events are not replayed; the client reads the snapshot again.
            if lagged.swap(false, Ordering::AcqRel) {
                let notice = Notice::named("lagged", keepline_core::content::now());
                let Ok(bytes) = serde_json::to_vec(&Reply::Events(vec![notice])) else {
                    break;
                };
                if write_frame(&mut writer, CONTROL, &bytes).is_err() {
                    break;
                }
            }
        }
        let _ = writer.shutdown(std::net::Shutdown::Both);
    });
    // A blocked read observes the client leaving without a heartbeat or timer.
    let result = read_frame(&mut stream);
    let mut host = lock(&shared)?;
    host.event_subscribers.remove(&connection);
    SUBSCRIBED.store(!host.event_subscribers.is_empty(), Ordering::Release);
    drop(host);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    result.map(|_| ())
}
