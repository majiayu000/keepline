use crate::{pty, store::Store};
use keepline_core::{
    model::*,
    protocol::*,
    terminal::{CellPixels, Vt},
};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{sync_channel, SyncSender},
    Arc, Mutex, MutexGuard,
};
use std::thread;
use std::time::{Duration, Instant};

mod association;
mod client;
mod live;
mod status;
use association::*;
use client::*;
use live::*;
use status::*;
type Shared = Arc<Mutex<Host>>;
type LiveTerminal = Arc<Mutex<Live>>;
/// Captured at startup: rebuilding the binary later must not hide that this host is old.
static BUILD: std::sync::LazyLock<keepline_core::protocol::Build> =
    std::sync::LazyLock::new(keepline_core::protocol::Build::current);
struct Host {
    store: Store,
    workspace: Workspace,
    terminals: HashMap<String, LiveTerminal>,
    instance: String,
    index: crate::log_index::Index,
    recognition: RecognitionStatus,
    log_events: Option<crate::file_events::Notifier>,
    recognition_subscribers: HashMap<u64, Subscriber>,
    exits: Option<crate::process_exit::ExitWatch>,
    agent_pids: HashMap<u32, String>,
    /// Agent processes no evidence has tied to a conversation yet: their terminal and CLI.
    unbound_pids: HashMap<u32, (String, Agent)>,
    /// Agent processes that save no transcript, so their hint goes when they exit.
    unrecorded_pids: HashMap<u32, String>,
    /// Recent Enter presses per terminal, seconds since the epoch: evidence for which
    /// pane a shared-daemon Codex prompt came from.
    submits: HashMap<String, std::collections::VecDeque<u64>>,
    input_probes: std::collections::HashSet<String>,
    deadlines: Option<SyncSender<()>>,
    /// This host behind its lock, for work started from inside a commit.
    myself: std::sync::Weak<Mutex<Host>>,
    /// Terminal -> what its last quiet screen showed: the question and the rule that read
    /// it, or why none did. Live fallback only: never stored, cleared by the user's input;
    /// only a question appearing or going away is a new revision.
    screen_waits: HashMap<String, crate::screen_rules::Explained>,
    /// Session -> last `claude agents --json` run for it, the per-session rate limit.
    reconciled: HashMap<String, Instant>,
    /// What the plugins' hooks last heard, so each state change reaches them once.
    hooks: crate::plugin_hooks::Hooks,
}
fn lock<T>(m: &Mutex<T>) -> io::Result<MutexGuard<'_, T>> {
    m.lock()
        .map_err(|_| io::Error::other("state lock poisoned"))
}
fn geometry(cols: u16, rows: u16) -> io::Result<()> {
    if !(2..=400).contains(&cols) || !(1..=200).contains(&rows) {
        return Err(io::Error::other("terminal size outside 2..400 × 1..200"));
    }
    Ok(())
}
impl Host {
    fn snapshot(&self) -> Workspace {
        let mut w = self.workspace.clone();
        w.agent_sessions
            .retain(|s| s.terminal.is_some() || self.index.recent(s.updated));
        w.content
            .retain(|id, _| w.agent_sessions.iter().any(|s| &s.id == id));
        // A prompt visible on a live agent's screen outranks a log that has not caught up.
        // The source stays marked so it is never mistaken for a transcript record.
        for s in w.agent_sessions.iter().filter(|s| s.current) {
            let Some(seen) = s.terminal.as_ref().and_then(|t| self.screen_waits.get(t)) else {
                continue;
            };
            let Some(c) = w.content.get_mut(&s.id) else {
                continue;
            };
            c.approval = seen.approval().unwrap_or_default().to_owned();
            match seen.question() {
                Some(question) if c.turn != keepline_core::content::Turn::Waiting => {
                    c.turn = keepline_core::content::Turn::Waiting;
                    c.waiting = format!("{question}（画面上的提示）");
                    // The episode the log or a hook will record for this same dialog.
                    c.waiting_episode += 1;
                    c.signal = "画面".into();
                    c.signal_detail = seen.detail();
                }
                Some(_) => (),
                None => c.screen_check = seen.fallback(),
            }
        }
        w
    }
    fn set_screen_wait(
        &mut self,
        terminal: &str,
        seen: Option<crate::screen_rules::Explained>,
    ) -> io::Result<()> {
        let before = self.screen_waits.get(terminal).and_then(|e| e.question());
        let changed = before != seen.as_ref().and_then(|e| e.question());
        match seen {
            Some(e) => self.screen_waits.insert(terminal.into(), e),
            None => self.screen_waits.remove(terminal),
        };
        // Why nothing matched rides along with the next revision; it is no change itself.
        if !changed {
            return Ok(());
        }
        // Nothing to store; a new revision makes clients fetch the overlaid snapshot.
        let next = self.workspace.clone();
        self.commit(next)
    }
    /// Asks the CLIs off the lock (`claude agents --json`, Codex `thread/read`) and applies the
    /// result. Returns when to look again for sessions held back by the rate limit.
    fn reconcile(&mut self, shared: &Shared, sessions: Vec<AgentSession>) -> Option<Instant> {
        let now = Instant::now();
        let mut later = None;
        let mut due = Vec::new();
        for s in sessions {
            match self.reconciled.get(&s.id) {
                Some(at) if now < *at + RECONCILE_EVERY => {
                    let next = *at + RECONCILE_EVERY;
                    later = Some(later.map_or(next, |l: Instant| l.min(next)));
                }
                _ => {
                    let pid = self
                        .agent_pids
                        .iter()
                        .find(|(_, id)| **id == s.id)
                        .map(|(pid, _)| *pid);
                    self.reconciled.insert(s.id.clone(), now);
                    due.push((s, pid));
                }
            }
        }
        if due.is_empty() {
            return later;
        }
        let Ok(index) = self.index.fork_current() else {
            return later;
        };
        let shared = shared.clone();
        std::thread::spawn(move || {
            if let Err(e) = apply_official(&shared, &index, &due) {
                eprintln!("keeplined: official status unavailable: {e}");
            }
        });
        later
    }
    fn publish_status(&mut self) -> io::Result<()> {
        self.recognition.workspace_revision = self.workspace.revision;
        let payload = Arc::new(
            serde_json::to_vec(&Reply::Recognition(recognition_status(self)))
                .map_err(io::Error::other)?,
        );
        self.recognition_subscribers.retain(|_, s| {
            if s.tx.try_send((CONTROL, payload.clone())).is_err() {
                let _ = s.socket.shutdown(std::net::Shutdown::Both);
                false
            } else {
                true
            }
        });
        Ok(())
    }
    fn agent_exited(&mut self, pid: u32) -> io::Result<()> {
        if let Some(terminal) = self
            .unbound_pids
            .remove(&pid)
            .map(|(terminal, _)| terminal)
            .or_else(|| self.unrecorded_pids.remove(&pid))
        {
            self.unhint(&terminal);
            self.publish_status()?;
        }
        if let Some(id) = self.agent_pids.remove(&pid) {
            let mut next = self.workspace.clone();
            if let Some(s) = next
                .agent_sessions
                .iter_mut()
                .find(|s| s.id == id && s.current)
            {
                s.current = false;
                if let Some(events) = &self.log_events {
                    events.file(s.transcript.clone().into());
                }
                self.commit(next)?;
            }
        }
        Ok(())
    }
    fn observe_processes(
        &mut self,
        processes: &[crate::recognition::Process],
        files: &HashMap<u32, Vec<String>>,
    ) -> io::Result<()> {
        let old_hints = (
            self.recognition.terminal_hints.clone(),
            self.recognition.terminal_notes.clone(),
        );
        self.recognition.terminal_hints.clear();
        self.recognition.terminal_notes.clear();
        self.recognition.terminal_agents.clear();
        for process in processes.iter().filter(|p| p.runtime.is_some()) {
            let Some(terminal) =
                crate::recognition::terminal_for_process(process.pid, processes, &self.workspace)
            else {
                continue;
            };
            if let Some(session) = self
                .workspace
                .current_agent(&terminal)
                .filter(|s| Some(s.runtime) == process.runtime)
            {
                if process.cli_session_id.as_deref() != Some(&session.cli_session_id)
                    && !files
                        .get(&process.pid)
                        .is_some_and(|paths| paths.contains(&session.transcript))
                {
                    continue;
                }
                let id = session.id.clone();
                if self.agent_pids.get(&process.pid) != Some(&id) {
                    self.agent_pids.insert(process.pid, id);
                    if let Some(exits) = &self.exits {
                        if !exits.watch(process.pid)? {
                            self.agent_exited(process.pid)?;
                        }
                    }
                }
            } else if let Some(runtime) = process.runtime {
                let adapter = crate::agents::adapter(runtime);
                let watch = match adapter.pending(process, processes) {
                    crate::agents::Pending::Wait => false,
                    crate::agents::Pending::Unrecorded(hint) => {
                        self.hint(&terminal, runtime, hint);
                        if let std::collections::hash_map::Entry::Vacant(entry) =
                            self.unrecorded_pids.entry(process.pid)
                        {
                            entry.insert(terminal);
                            true
                        } else {
                            false
                        }
                    }
                    // Unbound, whatever the CLI: screen association and hooks get their
                    // chance. The terminal says so unless the adapter reads a state from
                    // the title instead, or has nothing to say yet.
                    pending @ (crate::agents::Pending::Unbound { .. }
                    | crate::agents::Pending::Quiet) => {
                        if let crate::agents::Pending::Unbound { title_status: true } = pending {
                            let title = match self.terminals.get(&terminal) {
                                Some(live) => lock(live)?.title.clone(),
                                None => String::new(),
                            };
                            if let Some(hint) = adapter.title_status(&title) {
                                self.hint(&terminal, runtime, hint);
                            }
                        } else if pending
                            == (crate::agents::Pending::Unbound {
                                title_status: false,
                            })
                        {
                            self.hint(&terminal, runtime, crate::agents::unbound_hint(runtime));
                        }
                        if let std::collections::hash_map::Entry::Vacant(entry) =
                            self.unbound_pids.entry(process.pid)
                        {
                            entry.insert((terminal, runtime));
                            true
                        } else {
                            false
                        }
                    }
                };
                if watch {
                    if let Some(exits) = &self.exits {
                        if !exits.watch(process.pid)? {
                            self.agent_exited(process.pid)?;
                        }
                    }
                }
            }
        }
        if old_hints
            != (
                self.recognition.terminal_hints.clone(),
                self.recognition.terminal_notes.clone(),
            )
        {
            self.publish_status()?;
        }
        Ok(())
    }
    /// Every tab names itself from its program's title, front or not. An unbound Agent
    /// whose adapter reads state from the title also paints it into its note.
    /// Commands the shell in `terminal` marked finished, for the hooks that listen for them.
    /// No title: shells title the terminal with the running command, which may hold a secret.
    fn commands_finished(
        &self,
        terminal: &str,
        pwd: Option<String>,
        finished: Vec<crate::command_marks::Finished>,
    ) {
        use keepline_core::hook::{Event, Payload};
        let placed = self.workspace.terminals.iter().find(|t| t.id == terminal);
        let space = placed
            .and_then(|t| self.workspace.spaces.iter().find(|p| p.id == t.space))
            .map(|p| p.name.clone());
        let cwd = pwd
            .or_else(|| placed.map(|t| t.cwd.clone()))
            .unwrap_or_default();
        for f in finished {
            crate::plugin_hooks::command(Payload {
                event: match f.exit {
                    Some(code) if code != 0 => Event::CommandFailed,
                    _ => Event::CommandDone,
                },
                session: String::new(),
                agent: String::new(),
                title: String::new(),
                cwd: cwd.clone(),
                space: space.clone(),
                detail: String::new(),
                at: keepline_core::content::now(),
                terminal: Some(terminal.into()),
                exit_code: f.exit,
                duration_ms: Some(u64::try_from(f.took.as_millis()).unwrap_or(u64::MAX)),
            });
        }
    }
    fn title_changed(&mut self, terminal: &str, title: &str) -> io::Result<()> {
        let shown = keepline_core::terminal::shown_title(title);
        let mut changed = self.recognition.terminal_titles.get(terminal) != shown.as_ref();
        match shown {
            Some(shown) => self
                .recognition
                .terminal_titles
                .insert(terminal.into(), shown),
            None => self.recognition.terminal_titles.remove(terminal),
        };
        let runtime = self
            .unbound_pids
            .values()
            .find(|(t, _)| t == terminal)
            .map(|(_, runtime)| *runtime);
        if let Some(runtime) =
            runtime.filter(|_| self.recognition.terminal_hints.contains_key(terminal))
        {
            if let Some(hint) = crate::agents::adapter(runtime).title_status(title) {
                changed |= self.hint(terminal, runtime, hint);
            }
        }
        if changed {
            self.publish_status()?;
        }
        Ok(())
    }
    /// Sets a terminal's hint and note together; whether either changed.
    fn hint(&mut self, terminal: &str, agent: Agent, hint: crate::agents::Hint) -> bool {
        self.recognition
            .terminal_agents
            .insert(terminal.into(), agent);
        let changed = self.recognition.terminal_hints.get(terminal) != Some(&hint.text)
            || self.recognition.terminal_notes.get(terminal) != Some(&hint.note);
        self.recognition
            .terminal_hints
            .insert(terminal.into(), hint.text);
        self.recognition
            .terminal_notes
            .insert(terminal.into(), hint.note);
        changed
    }
    fn unhint(&mut self, terminal: &str) {
        self.recognition.terminal_hints.remove(terminal);
        self.recognition.terminal_notes.remove(terminal);
        self.recognition.terminal_agents.remove(terminal);
    }
    fn commit(&mut self, mut next: Workspace) -> io::Result<()> {
        next.revision = self.workspace.revision + 1;
        self.store.save(&self.workspace, &next)?;
        // Terminals that just got their Agent conversation. The quiet-screen check after
        // the last output skipped them while unbound; a dialog drawn before binding (an
        // Agent that starts on an approval, or a slow identification under load) would
        // otherwise never be seen, since the screen stays still.
        let bound: Vec<String> = next
            .agent_sessions
            .iter()
            .filter(|s| s.current)
            .filter_map(|s| s.terminal.clone())
            .filter(|t| {
                self.workspace.current_agent(t).map(|s| &s.id)
                    != next.current_agent(t).map(|s| &s.id)
            })
            .collect();
        let hooked = !keepline_core::hook::all().is_empty();
        if hooked && !self.hooks.seeded() {
            // The state before the first change is where hooks start listening.
            let before = self.snapshot();
            self.hooks.changes(&before);
        }
        self.workspace = next;
        if hooked {
            let after = self.snapshot();
            self.hooks.observe(&after);
        }
        for id in bound {
            let (Some(shared), Some(terminal)) =
                (self.myself.upgrade(), self.terminals.get(&id).cloned())
            else {
                continue;
            };
            thread::spawn(move || {
                let mut at = Instant::now() + SCREEN_QUIET;
                loop {
                    thread::sleep(at.saturating_duration_since(Instant::now()));
                    match check_screen_wait(&terminal, &shared, &id) {
                        Ok(Some(next)) => at = next,
                        Ok(None) => break,
                        Err(e) => {
                            eprintln!("screen check after binding {id}: {e}");
                            break;
                        }
                    }
                }
            });
        }
        if let Some(tx) = &self.deadlines {
            let _ = tx.try_send(());
        }
        self.publish_status()?;
        Ok(())
    }
}
// One wait until an actual outcome/group deadline. No tick, no directory or process I/O.
fn start_deadlines(shared: &Shared) -> io::Result<()> {
    let (tx, rx) = sync_channel(1);
    lock(shared)?.deadlines = Some(tx);
    let weak = Arc::downgrade(shared);
    thread::spawn(move || {
        let run = || -> io::Result<()> {
            loop {
                let Some(shared) = weak.upgrade() else {
                    return Ok(());
                };
                let deadline = {
                    let mut host = lock(&shared)?;
                    let now = keepline_core::content::now();
                    let mut next = host.workspace.clone();
                    if next.expire_outcomes(now) {
                        host.commit(next)?;
                    }
                    host.workspace.next_deadline(now)
                };
                drop(shared);
                let result = match deadline {
                    Some(at) => rx.recv_timeout(std::time::Duration::from_secs(
                        at.saturating_sub(keepline_core::content::now()),
                    )),
                    None => rx
                        .recv()
                        .map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected),
                };
                match result {
                    Ok(()) => (),
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        let Some(shared) = weak.upgrade() else {
                            return Ok(());
                        };
                        let mut host = lock(&shared)?;
                        let mut next = host.workspace.clone();
                        next.expire_outcomes(keepline_core::content::now());
                        host.commit(next)?;
                    }
                }
            }
        };
        if let Err(e) = run() {
            eprintln!("outcome deadline: {e}");
        }
    });
    Ok(())
}
pub fn serve(root: &Path) -> io::Result<()> {
    serve_with_roots(root, crate::recognition::roots()?)
}
/// Explicit roots keep fixture execution independent of the user's CLI history.
pub fn serve_with_roots(root: &Path, roots: Vec<(Agent, std::path::PathBuf)>) -> io::Result<()> {
    serve_with_options(root, roots, 7)
}
pub fn serve_with_options(
    root: &Path,
    roots: Vec<(Agent, std::path::PathBuf)>,
    recent_days: u64,
) -> io::Result<()> {
    serve_runtime(root, roots, recent_days, false)
}
/// Starts by taking every running terminal over from the host already serving `root`,
/// which then exits; its shells and Agents keep running under this host.
pub fn serve_taking_over(
    root: &Path,
    roots: Vec<(Agent, std::path::PathBuf)>,
    recent_days: u64,
) -> io::Result<()> {
    serve_runtime(root, roots, recent_days, true)
}
/// A terminal received from the previous host.
struct Taken {
    handed: Handed,
    snapshot: Vec<u8>,
    master: File,
}
fn take_over(root: &Path) -> io::Result<Vec<Taken>> {
    let mut stream = UnixStream::connect(root.join("workbench.sock"))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
    match receive::<Reply>(&mut stream)? {
        Reply::Hello { major: MAJOR, .. } => (),
        other => return Err(io::Error::other(format!("old host refused: {other:?}"))),
    }
    send(&mut stream, CONTROL, &Request::Handoff)?;
    // A host from before handoffs drops the connection at the unknown request.
    let terminals = match receive::<Reply>(&mut stream) {
        Ok(Reply::Handoff { terminals }) => terminals,
        other => {
            return Err(io::Error::other(format!(
                "the running host cannot hand its terminals over: {other:?}"
            )));
        }
    };
    let mut snapshots = vec![];
    for _ in &terminals {
        let mut snapshot = vec![];
        loop {
            match read_frame(&mut stream)? {
                (SNAPSHOT, bytes) => snapshot.extend(bytes),
                (READY, _) => break,
                (kind, _) => return Err(io::Error::other(format!("unexpected frame {kind}"))),
            }
        }
        snapshots.push(snapshot);
    }
    let fds = crate::handoff::recv_fds(&stream, terminals.len())?;
    write_frame(&mut stream, ACK, HANDOFF_TAKEN)?;
    Ok(terminals
        .into_iter()
        .zip(snapshots)
        .zip(fds)
        .map(|((handed, snapshot), fd)| Taken {
            handed,
            snapshot,
            master: File::from(fd),
        })
        .collect())
}
fn serve_runtime(
    root: &Path,
    roots: Vec<(Agent, std::path::PathBuf)>,
    recent_days: u64,
    takeover: bool,
) -> io::Result<()> {
    std::sync::LazyLock::force(&BUILD);
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e),
    }
    let meta = fs::symlink_metadata(root)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o777 != 0o700
    {
        return Err(io::Error::other(
            "runtime must be an owned, non-symlink 0700 directory",
        ));
    }
    let taken = if takeover { take_over(root)? } else { vec![] };
    let instance_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root.join("host.lock"))?;
    // After a handoff the old host exits on its own; wait for it to let go of the runtime.
    let until = Instant::now() + Duration::from_secs(if takeover { 10 } else { 0 });
    while unsafe { libc::flock(instance_lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        if Instant::now() >= until {
            return Err(io::Error::other("a daemon already owns this runtime"));
        }
        thread::sleep(Duration::from_millis(20));
    }
    let socket = root.join("workbench.sock");
    match fs::symlink_metadata(&socket) {
        Ok(meta) if meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() } => {
            fs::remove_file(&socket)?
        }
        Ok(_) => {
            return Err(io::Error::other(
                "refusing to replace a non-socket runtime entry",
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    // Without it hooks fall back to the process tree alone.
    if let Err(e) = pty::announce_host(root) {
        eprintln!("announce host to terminals: {e}");
    }
    // Without it shells still start; only their directory reports are missing.
    if let Err(e) = pty::install_shell_integration(root) {
        eprintln!("install shell integration: {e}");
    }
    let kept = taken.iter().map(|t| t.handed.id.clone()).collect();
    let (store, mut workspace) = Store::open_keeping(root, &kept)?;
    let mut index = crate::log_index::Index::open(root, roots)?;
    index.recent_days = recent_days;
    for s in &workspace.agent_sessions {
        if let Some(card) = index.card(&s.transcript)? {
            workspace.content.insert(s.id.clone(), card);
        }
    }
    let host = Arc::new(Mutex::new(Host {
        store,
        index: index.fork(root)?,
        recognition: RecognitionStatus::default(),
        log_events: None,
        recognition_subscribers: HashMap::new(),
        exits: None,
        agent_pids: HashMap::new(),
        unbound_pids: HashMap::new(),
        unrecorded_pids: HashMap::new(),
        submits: HashMap::new(),
        input_probes: Default::default(),
        deadlines: None,
        myself: std::sync::Weak::new(),
        screen_waits: HashMap::new(),
        hooks: Default::default(),
        reconciled: HashMap::new(),
        workspace,
        terminals: HashMap::new(),
        instance: uuid::Uuid::new_v4().to_string(),
    }));
    lock(&host)?.myself = Arc::downgrade(&host);
    let weak = Arc::downgrade(&host);
    lock(&host)?.exits = Some(crate::process_exit::ExitWatch::new(move |pid| {
        if let Some(host) = weak.upgrade() {
            if let Err(e) = lock(&host).and_then(|mut h| h.agent_exited(pid)) {
                eprintln!("persist Agent exit: {e}");
            }
        }
    })?);
    for t in taken {
        let mut vt = Vt::restore(&t.snapshot)?;
        let replies = Arc::new(Mutex::new(Vec::new()));
        let out = Arc::clone(&replies);
        vt.on_reply(move |bytes| {
            if let Ok(mut out) = out.lock() {
                out.extend_from_slice(bytes);
            }
        })?;
        let cell = CellPixels::default();
        cell.set(t.handed.cell.0, t.handed.cell.1);
        vt.answer_queries(cell.clone())?;
        let reader = t.master.try_clone()?;
        let mut guard = lock(&host)?;
        run_live(
            &mut guard,
            &host,
            t.handed.id,
            Live {
                vt,
                master: t.master,
                cols: t.handed.cols,
                rows: t.handed.rows,
                pid: t.handed.pid,
                owner: None,
                subscribers: HashMap::new(),
                exited: false,
                drained: false,
                status: None,
                exit_code: None,
                title: t.handed.title,
                cell,
                input_at: None,
                marks: Default::default(),
            },
            reader,
            replies,
            Reaper::Pid(t.handed.pid),
        );
    }
    start_deadlines(&host)?;
    start_subscription(Arc::clone(&host), index);
    // bind(2) creates the pathname before listen(2) makes it connectable. Under
    // scheduler pressure a client seeing that pathname can get ECONNREFUSED.
    // Publish only an initialized, listening socket, while holding host.lock.
    let pending_socket = root.join(".sock-start");
    match fs::symlink_metadata(&pending_socket) {
        Ok(meta) if meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() } => {
            fs::remove_file(&pending_socket)?;
        }
        Ok(_) => {
            return Err(io::Error::other(
                "refusing to replace pending non-socket entry",
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(&pending_socket)?;
    fs::set_permissions(&pending_socket, fs::Permissions::from_mode(0o600))?;
    fs::rename(&pending_socket, &socket)?;
    eprintln!("keeplined listening on {}", socket.display());
    for (index, stream) in listener.incoming().enumerate() {
        let stream = stream?;
        let host = Arc::clone(&host);
        thread::spawn(move || {
            if let Err(e) = serve_client(stream, host, index as u64 + 1) {
                if !matches!(
                    e.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionReset
                ) {
                    eprintln!("client disconnected: {e}");
                }
            }
        });
    }
    Ok(())
}

/// This machine's name, as the shell's $HOST spells it in OSC 7 reports.
fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: the kernel writes at most `buf.len()` bytes, NUL-terminated when it fits.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return String::new();
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(test)]
mod recognition_tests {
    use super::*;
    #[test]
    fn must_scan_repairs_only_event_subtree_and_publishes_status_then_continues() {
        let root = std::env::temp_dir().join(format!("kl-gap-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("logs/a")).unwrap();
        fs::create_dir_all(root.join("logs/b")).unwrap();
        let root = root.canonicalize().unwrap();
        let roots = vec![root.join("logs")];
        let (store, workspace) = Store::open(&root).unwrap();
        let mut index =
            crate::log_index::Index::open(&root, vec![(Agent::Codex, roots[0].clone())]).unwrap();
        index.initialize().unwrap();
        index.bootstrap().unwrap();
        let shared = Arc::new(Mutex::new(Host {
            store,
            workspace,
            terminals: HashMap::new(),
            instance: "test".into(),
            index: index.fork(&root).unwrap(),
            recognition: RecognitionStatus::default(),
            log_events: None,
            recognition_subscribers: HashMap::new(),
            exits: None,
            agent_pids: HashMap::new(),
            unbound_pids: HashMap::new(),
            unrecorded_pids: HashMap::new(),
            submits: HashMap::new(),
            input_probes: Default::default(),
            deadlines: None,
            myself: std::sync::Weak::new(),
            screen_waits: HashMap::new(),
            hooks: Default::default(),
            reconciled: HashMap::new(),
        }));
        let (tx, rx) = sync_channel(16);
        let (socket, _peer) = UnixStream::pair().unwrap();
        lock(&shared).unwrap().recognition_subscribers.insert(
            1,
            Subscriber {
                tx,
                socket,
                lagged: Default::default(),
            },
        );
        for dir in ["a", "b"] {
            fs::write(root.join(format!("logs/{dir}/new.jsonl")),format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{dir}\",\"cwd\":\"/tmp\"}}}}\n")).unwrap();
        }
        let batch = crate::file_events::Batch {
            events: vec![crate::file_events::Event {
                path: root.join("logs/a"),
                flags: 1,
                id: 1,
            }],
        };
        let mut scopes = vec![];
        assert!(!event_scopes(&batch, &roots, &mut scopes));
        let before = index.counts.clone();
        recover_logs(&shared, &mut index, &mut Association::default(), &scopes).unwrap();
        assert_eq!(index.counts.directories - before.directories, 1);
        assert_eq!(
            index
                .cached()
                .unwrap()
                .iter()
                .map(|t| t.cli_id.as_str())
                .collect::<Vec<_>>(),
            ["a"]
        );
        let mut phases: Vec<_> = rx
            .try_iter()
            .map(
                |(_, bytes)| match serde_json::from_slice::<Reply>(&bytes).unwrap() {
                    Reply::Recognition(s) => s.phase,
                    _ => panic!(),
                },
            )
            .collect();
        // Content imports publish a revision while recovery remains in progress.
        phases.dedup();
        assert_eq!(
            phases,
            [
                RecognitionPhase::Interrupted,
                RecognitionPhase::Recovering,
                RecognitionPhase::Healthy
            ]
        );
        assert!(index.recovery_scopes().unwrap().is_empty());
        index
            .update(Agent::Codex, &root.join("logs/b/new.jsonl"), true)
            .unwrap();
        assert_eq!(index.cached().unwrap().len(), 2);
        drop(shared);
        drop(index);
        fs::remove_dir_all(root).unwrap();
    }
}
