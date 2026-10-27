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

type Shared = Arc<Mutex<Host>>;
type LiveTerminal = Arc<Mutex<Live>>;
/// Captured at startup: rebuilding the binary later must not hide that this host is old.
static BUILD: std::sync::LazyLock<keepline_core::protocol::Build> =
    std::sync::LazyLock::new(keepline_core::protocol::Build::current);
type Frame = (u8, Arc<Vec<u8>>);
/// Unread input above which mouse reports are dropped; the queue holds about 1 KiB.
const POINTER_BACKLOG: usize = 256;
struct Subscriber {
    tx: SyncSender<Frame>,
    socket: UnixStream,
}
struct Live {
    vt: Vt,
    master: File,
    cols: u16,
    rows: u16,
    pid: u32,
    owner: Option<u64>,
    subscribers: HashMap<u64, Subscriber>,
    exited: bool,
    drained: bool,
    exit_code: Option<i32>,
    title: String,
    cell: CellPixels,
    /// Last keystrokes sent here; a menu the user just opened is not the agent asking.
    input_at: Option<std::time::Instant>,
}
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
    unbound_pids: HashMap<u32, String>,
    /// Recent Enter presses per terminal, seconds since the epoch: evidence for which
    /// pane a shared-daemon Codex prompt came from.
    submits: HashMap<String, std::collections::VecDeque<u64>>,
    input_probes: std::collections::HashSet<String>,
    deadlines: Option<SyncSender<()>>,
    /// Terminal -> question seen on its quiet screen. Live fallback only: never stored,
    /// cleared by the user's input or by a later quiet screen without the prompt.
    screen_waits: HashMap<String, String>,
    /// Session -> last `claude agents --json` run for it, the per-session rate limit.
    reconciled: HashMap<String, Instant>,
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
            let Some(question) = s.terminal.as_ref().and_then(|t| self.screen_waits.get(t)) else {
                continue;
            };
            if let Some(c) = w.content.get_mut(&s.id) {
                if c.turn != keepline_core::content::Turn::Waiting {
                    c.turn = keepline_core::content::Turn::Waiting;
                    c.waiting = format!("{question}（画面上的提示）");
                    // The episode the log or a hook will record for this same dialog.
                    c.waiting_episode += 1;
                    c.signal = "画面".into();
                }
            }
        }
        w
    }
    fn set_screen_wait(&mut self, terminal: &str, question: Option<String>) -> io::Result<()> {
        if self.screen_waits.get(terminal) == question.as_ref() {
            return Ok(());
        }
        match question {
            Some(q) => self.screen_waits.insert(terminal.into(), q),
            None => self.screen_waits.remove(terminal),
        };
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
        if let Some(terminal) = self.unbound_pids.remove(&pid) {
            self.recognition.terminal_hints.remove(&terminal);
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
        let old_hints = self.recognition.terminal_hints.clone();
        self.recognition.terminal_hints.clear();
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
            } else if process.runtime == Some(Agent::Codex)
                && !processes
                    .iter()
                    .any(|p| p.parent == process.pid && p.runtime == process.runtime)
            {
                // `--no-daemon` opens its own log; before the first prompt there is nothing to say.
                if !process.no_daemon {
                    let title = match self.terminals.get(&terminal) {
                        Some(live) => lock(live)?.title.clone(),
                        None => String::new(),
                    };
                    self.recognition
                        .terminal_hints
                        .insert(terminal.clone(), codex_hint(&title));
                }
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.unbound_pids.entry(process.pid)
                {
                    entry.insert(terminal);
                    if let Some(exits) = &self.exits {
                        if !exits.watch(process.pid)? {
                            self.agent_exited(process.pid)?;
                        }
                    }
                }
            }
        }
        if old_hints != self.recognition.terminal_hints {
            self.publish_status()?;
        }
        Ok(())
    }
    /// Every tab names itself from its program's title, front or not. An unbound
    /// shared-daemon Codex also paints its state into it.
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
        if self.unbound_pids.values().any(|t| t == terminal)
            && self.recognition.terminal_hints.contains_key(terminal)
        {
            let hint = codex_hint(title);
            if self.recognition.terminal_hints.get(terminal) != Some(&hint) {
                self.recognition
                    .terminal_hints
                    .insert(terminal.into(), hint);
                changed = true;
            }
        }
        if changed {
            self.publish_status()?;
        }
        Ok(())
    }
    fn commit(&mut self, mut next: Workspace) -> io::Result<()> {
        next.revision = self.workspace.revision + 1;
        self.store.save(&self.workspace, &next)?;
        self.workspace = next;
        if let Some(tx) = &self.deadlines {
            let _ = tx.try_send(());
        }
        self.publish_status()?;
        Ok(())
    }
}
impl Live {
    // Event-driven handoff. No heartbeat or global process scan is needed.
    fn reconcile_owner(&mut self) {
        self.subscribers.retain(|_, s| {
            let mut byte = 0u8;
            // SAFETY: a one-byte peek on this subscriber socket never consumes input.
            let n = unsafe {
                libc::recv(
                    s.socket.as_raw_fd(),
                    (&mut byte as *mut u8).cast(),
                    1,
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                )
            };
            n > 0 || (n < 0 && io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock)
        });
        if self
            .owner
            .is_none_or(|id| !self.subscribers.contains_key(&id))
        {
            self.owner = self.subscribers.keys().min().copied();
        }
        loop {
            let owner = self.owner;
            self.subscribers.retain(|id, s| {
                let writable = Some(*id) == owner && !self.exited;
                if s.tx
                    .try_send((
                        WRITABLE,
                        Arc::new(if writable {
                            b"true".to_vec()
                        } else {
                            b"false".to_vec()
                        }),
                    ))
                    .is_err()
                {
                    let _ = s.socket.shutdown(std::net::Shutdown::Both);
                    false
                } else {
                    true
                }
            });
            if self
                .owner
                .is_some_and(|id| !self.subscribers.contains_key(&id))
            {
                self.owner = self.subscribers.keys().min().copied();
            } else {
                break;
            }
        }
    }
    fn broadcast(&mut self, kind: u8, bytes: Vec<u8>) {
        let bytes = Arc::new(bytes);
        self.subscribers.retain(|_, subscriber| {
            if subscriber.tx.try_send((kind, Arc::clone(&bytes))).is_err() {
                let _ = subscriber.socket.shutdown(std::net::Shutdown::Both);
                false
            } else {
                true
            }
        });
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
    let instance_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root.join("host.lock"))?;
    if unsafe { libc::flock(instance_lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err(io::Error::other("a daemon already owns this runtime"));
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
    let (store, mut workspace) = Store::open(root)?;
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
        submits: HashMap::new(),
        input_probes: Default::default(),
        deadlines: None,
        screen_waits: HashMap::new(),
        reconciled: HashMap::new(),
        workspace,
        terminals: HashMap::new(),
        instance: uuid::Uuid::new_v4().to_string(),
    }));
    let weak = Arc::downgrade(&host);
    lock(&host)?.exits = Some(crate::process_exit::ExitWatch::new(move |pid| {
        if let Some(host) = weak.upgrade() {
            if let Err(e) = lock(&host).and_then(|mut h| h.agent_exited(pid)) {
                eprintln!("persist Agent exit: {e}");
            }
        }
    })?);
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
            ))
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

/// Codex's own title rules (the ones Herdr reads): "Action Required" while blocked,
/// a braille spinner while working, anything else once idle. Status only, never identity.
pub(crate) fn codex_state(title: &str) -> Option<&'static str> {
    if title.trim().is_empty() {
        None
    } else if title.contains("Action Required") {
        Some("需要你处理")
    } else if title.chars().any(|c| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".contains(c)) {
        Some("工作中")
    } else {
        Some("空闲")
    }
}
pub(crate) fn codex_hint(title: &str) -> String {
    match codex_state(title) {
        Some(state) => format!("仅显示状态：{state}；看对话内容请用 codex --no-daemon"),
        None => "未关联：看对话内容请用 codex --no-daemon".into(),
    }
}
fn recognition_status(host: &Host) -> RecognitionStatus {
    let mut status = host.recognition.clone();
    let (ps, lsof) = crate::recognition::probe_counts();
    status.metrics.ps_spawns = ps;
    status.metrics.lsof_spawns = lsof;
    status
}
fn publish_recognition(
    shared: &Shared,
    index: &crate::log_index::Index,
    phase: RecognitionPhase,
    detail: &str,
    scopes: &[std::path::PathBuf],
) -> io::Result<()> {
    let mut host = lock(shared)?;
    let scope_names = scopes
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let changed = host.recognition.phase != phase
        || host.recognition.detail != detail
        || host.recognition.scopes != scope_names;
    host.recognition.phase = phase;
    host.recognition.detail = detail.into();
    host.recognition.scopes = scope_names;
    host.recognition.metrics.directories = index.counts.directories;
    host.recognition.metrics.log_opens = index.counts.opens;
    host.recognition.metrics.log_bytes = index.counts.bytes;
    host.recognition.metrics.index_writes = index.counts.writes;
    if changed {
        let payload = Arc::new(
            serde_json::to_vec(&Reply::Recognition(recognition_status(&host)))
                .map_err(io::Error::other)?,
        );
        host.recognition_subscribers.retain(|_, subscriber| {
            if subscriber.tx.try_send((CONTROL, payload.clone())).is_err() {
                let _ = subscriber.socket.shutdown(std::net::Shutdown::Both);
                false
            } else {
                true
            }
        });
    }
    Ok(())
}
fn watch_recognition(mut stream: UnixStream, shared: Shared, connection: u64) -> io::Result<()> {
    let (tx, rx) = sync_channel::<Frame>(16);
    {
        let mut host = lock(&shared)?;
        let payload = Arc::new(
            serde_json::to_vec(&Reply::Recognition(recognition_status(&host)))
                .map_err(io::Error::other)?,
        );
        tx.send((CONTROL, payload)).map_err(io::Error::other)?;
        host.recognition_subscribers.insert(
            connection,
            Subscriber {
                tx,
                socket: stream.try_clone()?,
            },
        );
    }
    let mut writer = stream.try_clone()?;
    thread::spawn(move || {
        for (kind, bytes) in rx {
            if write_frame(&mut writer, kind, &bytes).is_err() {
                break;
            }
        }
        let _ = writer.shutdown(std::net::Shutdown::Both);
    });
    // A blocked read observes client disconnect without a heartbeat/timer.
    let result = read_frame(&mut stream);
    lock(&shared)?.recognition_subscribers.remove(&connection);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    result.map(|_| ())
}

fn refresh_agents(
    shared: &Shared,
    transcripts: &[crate::recognition::Transcript],
) -> io::Result<()> {
    let workspace = lock(shared)?.workspace.clone();
    let processes = crate::recognition::processes(&workspace)?;
    let files = crate::recognition::open_transcripts(&workspace, &processes)?;
    let mut host = lock(shared)?;
    let mut next = host.workspace.clone();
    crate::recognition::reconcile(&mut next, transcripts, &processes, &files)?;
    if next.agent_sessions != host.workspace.agent_sessions {
        host.commit(next)?;
    }
    host.observe_processes(&processes, &files)?;
    Ok(())
}
#[derive(Default)]
struct Association {
    attempted: std::collections::HashSet<(String, String)>,
    inventory: Vec<u32>,
    /// (log, newest prompt) → screen comparisons so far. The TUI may paint the prompt a
    /// little after the log records it, so a few appends may retry; then it waits for a new prompt.
    screened: HashMap<(String, String), u8>,
}
impl Association {
    fn apply(
        &mut self,
        shared: &Shared,
        index: &crate::log_index::Index,
        transcripts: &[crate::recognition::Transcript],
    ) -> io::Result<()> {
        if transcripts.is_empty() {
            return Ok(());
        }
        let workspace = lock(shared)?.workspace.clone();
        let running: Vec<_> = workspace
            .terminals
            .iter()
            .filter(|t| t.state == ProcessState::Running)
            .filter_map(|t| t.pid)
            .collect();
        let unbound: Vec<_> = transcripts
            .iter()
            .filter(|t| {
                !workspace.agent_sessions.iter().any(|s| {
                    s.current
                        && s.runtime == t.runtime
                        && s.cli_session_id == t.cli_id
                        && s.transcript == t.path
                })
            })
            .collect();
        // A new identity/file (or a changed managed tree) gets one probe. An
        // unresolved manual Claude appending the same file is not a polling clock.
        let needs_probe = !running.is_empty()
            && unbound.iter().any(|t| {
                running != self.inventory
                    || !self.attempted.contains(&(t.path.clone(), t.cli_id.clone()))
            });
        // Known files only advance metadata. Never enumerate processes or query the whole index here.
        {
            let mut host = lock(shared)?;
            let mut next = host.workspace.clone();
            for t in transcripts {
                let known = next
                    .agent_sessions
                    .iter()
                    .any(|s| s.runtime == t.runtime && s.cli_session_id == t.cli_id);
                if !known && index.recent(t.updated) {
                    next.observe_agent(crate::recognition::observation(t, None))
                        .map_err(io::Error::other)?;
                }
            }
            crate::recognition::reconcile(&mut next, transcripts, &[], &HashMap::new())?;
            for t in transcripts {
                if let Some(s) = next
                    .agent_sessions
                    .iter_mut()
                    .find(|s| s.runtime == t.runtime && s.cli_session_id == t.cli_id)
                {
                    if let Some(card) = index.card(&t.path)? {
                        if next.content.get(&s.id).is_some_and(|old| {
                            !card.turn_key.is_empty() && card.turn_key != old.turn_key
                        }) && s.outcome != keepline_core::model::Outcome::Open
                        {
                            next.outcome_history
                                .push(keepline_core::model::OutcomeChange {
                                    session: s.id.clone(),
                                    before: s.outcome.clone(),
                                    after: keepline_core::model::Outcome::Open,
                                    at: card.turn_started,
                                    reason: "新回合".into(),
                                });
                            s.outcome = keepline_core::model::Outcome::Open;
                        }
                        s.updated = s.updated.max(card.updated);
                        next.content.insert(s.id.clone(), card);
                    }
                }
            }
            if next.agent_sessions != host.workspace.agent_sessions
                || next.content != host.workspace.content
            {
                host.commit(next)?;
            }
        }
        if needs_probe {
            self.inventory = running;
            for t in unbound {
                self.attempted.insert((t.path.clone(), t.cli_id.clone()));
            }
            refresh_agents(shared, &index.cached()?)?;
        }
        if transcripts.iter().any(|t| t.runtime == Agent::Codex) {
            self.associate_screens(shared, index, transcripts)?;
        }
        Ok(())
    }
    /// Shared-daemon Codex runs its thread outside the PTY, so neither process nor open-file
    /// evidence exists. Bind only when a thread's newest prompt is on exactly one managed
    /// Codex screen and that screen shows no other candidate thread.
    fn associate_screens(
        &mut self,
        shared: &Shared,
        index: &crate::log_index::Index,
        batch: &[crate::recognition::Transcript],
    ) -> io::Result<()> {
        let (panes, workspace) = {
            let host = lock(shared)?;
            let mut panes = vec![];
            for (pid, terminal) in &host.unbound_pids {
                if host.workspace.current_agent(terminal).is_none() {
                    panes.push((*pid, terminal.clone(), None));
                }
            }
            for (pid, id) in &host.agent_pids {
                if let Some(s) = host.workspace.agent_sessions.iter().find(|s| {
                    &s.id == id && s.current && s.runtime == Agent::Codex && s.terminal.is_some()
                }) {
                    panes.push((*pid, s.terminal.clone().unwrap(), Some(s.clone())));
                }
            }
            panes.retain(|(_, t, _)| host.terminals.contains_key(t));
            (
                panes
                    .into_iter()
                    .map(|(pid, t, s)| (pid, host.terminals[&t].clone(), t, s))
                    .collect::<Vec<_>>(),
                host.workspace.clone(),
            )
        };
        if panes.is_empty() {
            return Ok(());
        }
        let window = keepline_core::content::now().saturating_sub(600);
        let unbound = |t: &crate::recognition::Transcript| {
            t.runtime == Agent::Codex
                && t.updated >= window
                && !workspace
                    .agent_sessions
                    .iter()
                    .any(|s| s.current && s.runtime == t.runtime && s.cli_session_id == t.cli_id)
        };
        // The newest prompt: its text when long enough to be screen evidence, and its time.
        let prompt =
            |t: &crate::recognition::Transcript| -> io::Result<Option<(Option<String>, u64)>> {
                Ok(index
                    .latest_user(&t.path)?
                    .map(|(text, at)| (crate::recognition::screen_key(&text), at)))
            };
        // Only a prompt with comparisons left justifies reading screens.
        let mut fresh = false;
        for t in batch.iter().filter(|t| unbound(t)) {
            if let Some((key, at)) = prompt(t)? {
                let tries = self
                    .screened
                    .entry((t.path.clone(), key.unwrap_or_else(|| at.to_string())))
                    .or_default();
                if *tries < 5 {
                    *tries += 1;
                    fresh = true;
                }
            }
        }
        if !fresh {
            return Ok(());
        }
        let cached = index.cached()?;
        let mut threads = vec![];
        for t in cached.iter().filter(|t| unbound(t)) {
            if let Some((key, at)) = prompt(t)? {
                let cwd = fs::canonicalize(&t.cwd)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| t.cwd.clone());
                threads.push(crate::recognition::ScreenThread {
                    transcript: t,
                    key,
                    at: Some(at),
                    cwd,
                });
            }
        }
        let submits = lock(shared)?.submits.clone();
        let mut terminals = vec![];
        for (pid, live, terminal, current) in &panes {
            let screen = lock(live)?.vt.tail_text(200)?;
            let current = match current {
                Some(s) => Some((
                    s.cli_session_id.clone(),
                    s.updated,
                    index
                        .latest_user_text(&s.transcript)?
                        .and_then(|p| crate::recognition::screen_key(&p)),
                )),
                None => None,
            };
            terminals.push(crate::recognition::ScreenTerminal {
                terminal: terminal.clone(),
                cwd: crate::recognition::process_cwd(*pid),
                screen: crate::recognition::screen_text(&screen),
                current,
                submits: submits
                    .get(terminal)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default(),
            });
        }
        let bindings = crate::recognition::screen_bindings(&threads, &terminals);
        if bindings.is_empty() {
            return Ok(());
        }
        let mut host = lock(shared)?;
        let mut next = host.workspace.clone();
        let mut bound = vec![];
        for (terminal, t) in bindings {
            if next
                .observe_agent(crate::recognition::observation(t, Some(terminal.clone())))
                .is_ok()
            {
                bound.push(terminal);
            }
        }
        if next.agent_sessions == host.workspace.agent_sessions {
            return Ok(());
        }
        host.commit(next)?;
        // The TUI pid now stands for the bound session: its exit clears `current`.
        for terminal in bound {
            let Some(id) = host
                .workspace
                .current_agent(&terminal)
                .map(|s| s.id.clone())
            else {
                continue;
            };
            let pids: Vec<_> = panes
                .iter()
                .filter(|(_, _, t, _)| t == &terminal)
                .map(|(pid, ..)| *pid)
                .collect();
            for pid in pids {
                host.unbound_pids.remove(&pid);
                host.agent_pids.insert(pid, id.clone());
            }
            host.recognition.terminal_hints.remove(&terminal);
        }
        host.publish_status()
    }
}
fn recover_logs(
    shared: &Shared,
    index: &mut crate::log_index::Index,
    association: &mut Association,
    scopes: &[std::path::PathBuf],
) -> io::Result<()> {
    if scopes.is_empty() {
        return Ok(());
    }
    // Persist scope before repairing: restart resumes only this unfinished subtree.
    index.queue_recovery(scopes)?;
    publish_recognition(
        shared,
        index,
        RecognitionPhase::Interrupted,
        "识别中断：文件事件不完整",
        scopes,
    )?;
    publish_recognition(
        shared,
        index,
        RecognitionPhase::Recovering,
        "识别中断，正在补扫",
        scopes,
    )?;
    let mut recovery = crate::log_index::Recovery::new(index, scopes);
    loop {
        let (done, transcripts) = recovery.step(index)?;
        if let Err(error) = association.apply(shared, index, &transcripts) {
            eprintln!("Agent association deferred after subtree repair: {error}");
        }
        if done {
            break;
        }
        thread::yield_now();
    }
    index.finish_recovery(scopes)?;
    if scopes.iter().any(|p| !p.exists()) {
        publish_recognition(
            shared,
            index,
            RecognitionPhase::Interrupted,
            "日志目录暂不可用，等待重建事件",
            scopes,
        )
    } else {
        publish_recognition(shared, index, RecognitionPhase::Healthy, "识别正常", &[])
    }
}
fn event_scopes(
    batch: &crate::file_events::Batch,
    roots: &[std::path::PathBuf],
    scopes: &mut Vec<std::path::PathBuf>,
) -> bool {
    let mut restart = false;
    for event in &batch.events {
        if event.flags & 0x8 != 0 {
            // IDs wrapped: the journal cannot identify a narrower lost range.
            for root in roots {
                crate::file_events::add_scope(scopes, root, roots);
            }
            restart = true;
        } else if event.flags & 0x27 != 0 {
            let path = if event.path.extension().is_some_and(|e| e == "jsonl") {
                event.path.parent().unwrap_or(&event.path)
            } else {
                &event.path
            };
            crate::file_events::add_scope(scopes, path, roots);
            restart |= event.flags & 0x20 != 0;
        } else if event.flags & 0x20000 != 0 && event.flags & (0x100 | 0x800) != 0 {
            // A new/renamed populated directory may produce only a directory event.
            crate::file_events::add_scope(scopes, &event.path, roots);
        }
    }
    restart
}
// Bootstrap can establish ownership without any queued FSEvents batch. Install
// vnode listeners before the first blocking receive, too: open writers need not
// close their files to deliver FSEvents. Register then read closes the append race.
fn follow_associated(
    shared: &Shared,
    index: &mut crate::log_index::Index,
    association: &mut Association,
    watch: &crate::file_events::Watch,
) -> io::Result<()> {
    let (mut associated, unbound) = {
        let host = lock(shared)?;
        let associated = host
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| s.current)
            .map(|s| std::path::PathBuf::from(&s.transcript))
            .collect::<Vec<_>>();
        (
            associated,
            host.unbound_pids.keys().copied().collect::<Vec<_>>(),
        )
    };
    // The shared Codex daemon keeps an unbound thread's log open, so FSEvents would hold its
    // prompt back until exit and screen association would never see it. While a managed
    // Codex is unbound, also follow recent unassociated Codex logs from that pane's cwd.
    if !unbound.is_empty() {
        let cwds: Vec<_> = unbound
            .iter()
            .filter_map(|pid| crate::recognition::process_cwd(*pid))
            .collect();
        let window = keepline_core::content::now().saturating_sub(600);
        let mut candidates: Vec<_> = index
            .cached()?
            .into_iter()
            .filter(|t| {
                t.runtime == Agent::Codex
                    && t.updated >= window
                    && !associated.iter().any(|p| p.as_os_str() == t.path.as_str())
                    && fs::canonicalize(&t.cwd)
                        .is_ok_and(|c| cwds.iter().any(|cwd| c.as_os_str() == cwd.as_str()))
            })
            .collect();
        candidates.sort_by_key(|t| std::cmp::Reverse(t.updated));
        associated.extend(
            candidates
                .into_iter()
                .take(16)
                .map(|t| std::path::PathBuf::from(t.path)),
        );
    }
    // The same holds for a bound Codex session's subagents: their logs stay open too.
    let parents: Vec<String> = {
        let host = lock(shared)?;
        host.workspace
            .agent_sessions
            .iter()
            .filter(|s| s.current && s.runtime == Agent::Codex)
            .map(|s| s.cli_session_id.clone())
            .collect()
    };
    if !parents.is_empty() {
        associated.extend(
            index
                .child_logs(&parents, 32)?
                .into_iter()
                .map(std::path::PathBuf::from),
        );
    }
    // Register first, then close the read/register race using the existing cursor.
    for path in watch.follow(&associated)? {
        if let Some(runtime) = index.runtime(&path) {
            if let Some(t) = index.update(runtime, &path, true)? {
                let mut found = vec![t];
                found.extend(index.take_touched());
                association.apply(shared, index, &found)?;
            }
        }
    }
    Ok(())
}
fn start_subscription(shared: Shared, mut index: crate::log_index::Index) {
    thread::spawn(move || {
        let result = (|| -> io::Result<()> {
            let mut checkpoint = index.initialize()?;
            let roots = index
                .roots
                .iter()
                .map(|(_, p)| p.clone())
                .collect::<Vec<_>>();
            let mut watch = crate::file_events::Watch::start(&roots, checkpoint)?;
            lock(&shared)?.log_events = Some(watch.notifier());
            index.bootstrap()?;
            publish_recognition(&shared, &index, RecognitionPhase::Healthy, "识别正常", &[])?;
            let mut association = Association::default();
            association.apply(&shared, &index, &index.cached()?)?;
            follow_associated(&shared, &mut index, &mut association, &watch)?;
            loop {
                let mut batch = watch.rx.recv().map_err(io::Error::other)?;
                // A single bounded burst window starts ONLY after a native event arrives.
                // This matches FSEvents' 200 ms latency for eager vnode writes too. It never
                // wakes an idle subscriber, and continuous writers cannot extend the deadline.
                let deadline = std::time::Instant::now() + Duration::from_millis(200);
                for _ in 0..63 {
                    match watch
                        .rx
                        .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    {
                        Ok(next) => batch.events.extend(next.events),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                        Err(e) => return Err(io::Error::other(e)),
                    }
                }
                let mut scopes = watch.take_overflow();
                let overflowed = !scopes.is_empty();
                let restart = event_scopes(&batch, &roots, &mut scopes) || overflowed;
                let wrapped = batch.events.iter().any(|e| e.flags & 0x8 != 0);
                if restart {
                    // Watch before repairing. Replay from the last committed checkpoint, not 'now',
                    // so restarting one stream cannot discard queued events for the other CLI root.
                    if wrapped {
                        checkpoint = crate::file_events::Watch::current_id();
                        index.reset_checkpoint(checkpoint)?;
                    }
                    watch = crate::file_events::Watch::start(&roots, checkpoint)?;
                    lock(&shared)?.log_events = Some(watch.notifier());
                }
                if !scopes.is_empty() {
                    if let Err(error) = recover_logs(&shared, &mut index, &mut association, &scopes)
                    {
                        index.fail(&error)?;
                        publish_recognition(
                            &shared,
                            &index,
                            RecognitionPhase::Interrupted,
                            &format!("补扫未完成：{error}；等待后续文件事件重试"),
                            &scopes,
                        )?;
                        // Leave the durable scope worklist. No busy retry or permanent error latch.
                        continue;
                    }
                }
                let mut paths = std::collections::BTreeSet::new();
                let mut event_id = checkpoint;
                for event in batch.events {
                    if !wrapped {
                        event_id = event_id.max(event.id);
                    }
                    if event.flags & 0x10 == 0
                        && event.path.extension().is_some_and(|e| e == "jsonl")
                    {
                        paths.insert(event.path);
                    }
                }
                let transcripts = index.update_many(paths, event_id)?;
                if let Err(error) = association.apply(&shared, &index, &transcripts) {
                    eprintln!("Agent association deferred: {error}");
                }
                follow_associated(&shared, &mut index, &mut association, &watch)?;
                // Only a real event drives retries of an earlier incomplete subtree.
                let pending = index.recovery_scopes()?;
                if !pending.is_empty() {
                    if let Err(error) =
                        recover_logs(&shared, &mut index, &mut association, &pending)
                    {
                        index.fail(&error)?;
                        publish_recognition(
                            &shared,
                            &index,
                            RecognitionPhase::Interrupted,
                            &format!("补扫未完成：{error}"),
                            &pending,
                        )?;
                        continue;
                    }
                }
                if event_id > checkpoint {
                    // Checkpoint is atomic with this batch of content/cursor writes.
                    checkpoint = event_id;
                }
                let status = lock(&shared)?.recognition.clone();
                publish_recognition(
                    &shared,
                    &index,
                    status.phase,
                    &status.detail,
                    &status
                        .scopes
                        .iter()
                        .map(std::path::PathBuf::from)
                        .collect::<Vec<_>>(),
                )?;
            }
        })();
        if let Err(error) = result {
            if let Err(persist) = index.fail(&error) {
                eprintln!("persist subscription failure: {persist}");
            }
            let _ = publish_recognition(
                &shared,
                &index,
                RecognitionPhase::Interrupted,
                &format!("识别中断：{error}"),
                &[],
            );
            eprintln!("log subscription failed: {error}");
        }
    });
}

fn serve_client(mut stream: UnixStream, host: Shared, connection: u64) -> io::Result<()> {
    let mut uid = 0;
    let mut gid = 0;
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0
        || uid != unsafe { libc::geteuid() }
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "peer uid mismatch",
        ));
    }
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    match receive::<Request>(&mut stream)? {
        Request::Hello { major: MAJOR } => send(
            &mut stream,
            CONTROL,
            &Reply::Hello {
                major: MAJOR,
                instance: lock(&host)?.instance.clone(),
                build: BUILD.clone(),
            },
        )?,
        _ => {
            send(
                &mut stream,
                CONTROL,
                &Reply::Error(format!("protocol major {MAJOR} required")),
            )?;
            return Ok(());
        }
    }
    loop {
        let request = receive::<Request>(&mut stream)?;
        if matches!(request, Request::WatchRecognition) {
            return watch_recognition(stream, host, connection);
        }
        if let Request::Attach { id, takeover } = request {
            let terminal = lock(&host)?.terminals.get(&id).cloned();
            match terminal {
                Some(terminal) => return attach(stream, terminal, connection, takeover, host, id),
                None => send(
                    &mut stream,
                    CONTROL,
                    &Reply::Error("terminal is not available in this daemon instance".into()),
                )?,
            }
            continue;
        }
        let result = dispatch(&host, request);
        let reply = result.unwrap_or_else(|e| Reply::Error(e.to_string()));
        send(&mut stream, CONTROL, &reply)?;
    }
}

fn dispatch(shared: &Shared, request: Request) -> io::Result<Reply> {
    match request {
        Request::RecognitionStatus => Ok(Reply::Recognition(recognition_status(&*lock(shared)?))),
        Request::RefreshAgents => {
            let transcripts = {
                let host = lock(shared)?;
                if let Some(error) = host.store.subscription_error()? {
                    return Err(io::Error::other(error));
                }
                host.store.transcripts()?
            };
            refresh_agents(shared, &transcripts)?;
            let mut host = lock(shared)?;
            let waiting: Vec<_> = host
                .workspace
                .agent_sessions
                .iter()
                .filter(|s| s.current)
                .filter(|s| {
                    host.workspace
                        .content
                        .get(&s.id)
                        .is_some_and(|c| c.turn == keepline_core::content::Turn::Waiting)
                })
                .cloned()
                .collect();
            if !waiting.is_empty() {
                host.reconcile(shared, waiting);
            }
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::Hook {
            runtime,
            pid,
            cli_session_id,
            transcript,
            ended,
            event,
            facts,
        } => {
            let (workspace, mut index, cached) = {
                let host = lock(shared)?;
                (
                    host.workspace.clone(),
                    host.index.fork_current()?,
                    host.store.transcripts()?,
                )
            };
            let processes = crate::recognition::processes(&workspace)?;
            // Reject unrelated callers before opening any supplied transcript.
            if crate::recognition::terminal_for_process(pid, &processes, &workspace).is_none() {
                return Err(io::Error::other(
                    "Hook process is not a descendant of a managed terminal",
                ));
            }
            let path = if transcript.is_empty() {
                let matches: Vec<_> = cached
                    .iter()
                    .filter(|t| t.runtime == runtime && t.cli_id == cli_session_id)
                    .collect();
                if matches.len() != 1 {
                    return Err(io::Error::other(
                        "Hook transcript absent or ambiguous in index",
                    ));
                }
                std::path::PathBuf::from(&matches[0].path)
            } else {
                crate::file_events::normalized(Path::new(&transcript))?
            };
            let observed = index
                .update(runtime, &path, true)?
                .ok_or_else(|| io::Error::other("transcript has no CLI identity"))?;
            let mut host = lock(shared)?;
            let mut next = host.workspace.clone();
            crate::recognition::apply_hook(
                &mut next,
                runtime,
                pid,
                &cli_session_id,
                &observed,
                &processes,
                ended,
            )?;
            if let Some(event) = event {
                index.hook(&observed.path, &event, &facts)?;
            }
            if let Some(s) = next
                .agent_sessions
                .iter()
                .find(|s| s.runtime == runtime && s.cli_session_id == cli_session_id)
            {
                if let Some(card) = index.card(&observed.path)? {
                    next.content.insert(s.id.clone(), card);
                }
            }
            host.commit(next)?;
            let mut ancestor = pid;
            let mut hook_files = HashMap::new();
            while let Some(process) = processes.iter().find(|p| p.pid == ancestor) {
                if process.runtime == Some(runtime) {
                    hook_files.insert(ancestor, vec![observed.path.clone()]);
                    break;
                }
                if ancestor == process.parent {
                    break;
                }
                ancestor = process.parent;
            }
            host.observe_processes(&processes, &hook_files)?;
            if let Some(events) = &host.log_events {
                events.file(path);
            }
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::ClaimNotification { session, key } => {
            let host = lock(shared)?;
            if !host.workspace.settings.notifications
                || !host
                    .workspace
                    .agent_sessions
                    .iter()
                    .any(|s| s.id == session)
            {
                return Ok(Reply::NotificationClaimed(false));
            }
            Ok(Reply::NotificationClaimed(
                host.store.claim_notification(&session, &key)?,
            ))
        }
        Request::ClientLeft { at } => {
            let mut host = lock(shared)?;
            let mut next = host.workspace.clone();
            next.last_left = at;
            host.commit(next)?;
            Ok(Reply::Ok)
        }
        Request::Workspace => Ok(Reply::Workspace(lock(shared)?.snapshot())),
        Request::Edit { revision, mut edit } => {
            let mut host = lock(shared)?;
            if edit.checks_revision() && host.workspace.revision != revision {
                return Err(io::Error::other("工作台已变化，请刷新后重试"));
            }
            if let Edit::SaveSessionTask { session, task } = &mut edit {
                let s = host
                    .workspace
                    .agent_sessions
                    .iter()
                    .find(|s| &s.id == session)
                    .ok_or_else(|| io::Error::other("会话不存在"))?;
                task.description = host.index.original_words(&s.transcript)?;
                if task.description.is_empty() {
                    return Err(io::Error::other("已索引内容没有用户原话，不能建立待办"));
                }
                if host
                    .workspace
                    .content
                    .get(session)
                    .is_some_and(|c| c.partial_history)
                {
                    task.description
                        .push_str("\n\n（历史仅覆盖索引尾段及之后的增量；此前要求未知。）");
                }
            }
            // Closing a tab or a pane ends what ran in it; placement alone never does.
            let stopped = match &edit {
                Edit::CloseTab { id } => host.workspace.tab_terminals(id),
                Edit::ClosePane { terminal } => vec![terminal.clone()],
                _ => Vec::new(),
            };
            let mut next = host.workspace.clone();
            next.edit(edit).map_err(io::Error::other)?;
            host.commit(next)?;
            for id in &stopped {
                if let Some(Err(e)) = host.terminals.get(id).map(stop_terminal) {
                    eprintln!("stop closed terminal {id}: {e}");
                }
            }
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::Launch {
            id,
            space,
            argv,
            cwd,
            cols,
            rows,
        } => {
            geometry(cols, rows)?;
            let mut host = lock(shared)?;
            if let Some(existing) = host.workspace.terminals.iter().find(|s| s.id == id) {
                if existing.argv != argv || existing.cwd != cwd || existing.space != space {
                    return Err(io::Error::other(
                        "launch ID was already used with different arguments",
                    ));
                }
                return Ok(Reply::Workspace(host.snapshot()));
            }
            if id.is_empty()
                || argv.is_empty()
                || argv[0].is_empty()
                || !Path::new(&cwd).is_absolute()
                || !Path::new(&cwd).is_dir()
            {
                return Err(io::Error::other(
                    "launch needs an ID, argv and an absolute existing working directory",
                ));
            }
            if !host.workspace.spaces.iter().any(|s| s.id == space) {
                return Err(io::Error::other("space not found"));
            }
            let mut vt = Vt::new(cols, rows)?;
            let replies = Arc::new(Mutex::new(Vec::new()));
            let out = Arc::clone(&replies);
            vt.on_reply(move |bytes| {
                if let Ok(mut out) = out.lock() {
                    out.extend_from_slice(bytes);
                }
            })?;
            let cell = CellPixels::default();
            vt.answer_queries(cell.clone())?;
            let mut next = host.workspace.clone();
            next.terminals.push(Terminal {
                id: id.clone(),
                space,
                argv: argv.clone(),
                cwd: cwd.clone(),
                pid: None,
                state: ProcessState::Starting,
                exit_code: None,
                error: None,
            });
            host.commit(next)?;
            let (mut child, master) = match pty::spawn(&argv, Path::new(&cwd), cols, rows) {
                Ok(result) => result,
                Err(error) => {
                    let mut next = host.workspace.clone();
                    let session = next.terminals.last_mut().unwrap();
                    session.state = ProcessState::Failed;
                    session.error = Some(error.to_string());
                    host.commit(next)?;
                    return Err(error);
                }
            };
            let reader = match master.try_clone() {
                Ok(reader) => reader,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e);
                }
            };
            let pid = child.id();
            let mut next = host.workspace.clone();
            let session = next.terminals.last_mut().unwrap();
            session.pid = Some(pid);
            session.state = ProcessState::Running;
            if let Err(e) = host.commit(next) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
            let terminal = Arc::new(Mutex::new(Live {
                vt,
                master,
                cols,
                rows,
                pid,
                owner: None,
                subscribers: HashMap::new(),
                exited: false,
                drained: false,
                exit_code: None,
                title: String::new(),
                cell,
                input_at: None,
            }));
            host.terminals.insert(id.clone(), Arc::clone(&terminal));
            let shared_read = Arc::clone(shared);
            let terminal_read = Arc::clone(&terminal);
            let id_read = id.clone();
            thread::spawn(move || {
                if let Err(e) = pump(reader, &terminal_read, replies, &shared_read, &id_read) {
                    eprintln!("terminal {id_read}: {e}");
                }
                if let Ok(mut live) = lock(&terminal_read) {
                    live.drained = true;
                }
                if let Err(e) = record_exit(&shared_read, &terminal_read, &id_read) {
                    eprintln!("persist terminal {id_read}: {e}");
                }
            });
            let shared_wait = Arc::clone(shared);
            thread::spawn(move || match child.wait() {
                Ok(status) => {
                    if let Ok(mut live) = lock(&terminal) {
                        live.exited = true;
                        live.exit_code = status.code();
                    }
                    if let Err(e) = record_exit(&shared_wait, &terminal, &id) {
                        eprintln!("persist exit {id}: {e}");
                    }
                }
                Err(error) => eprintln!("wait terminal {id}: {error}"),
            });
            Ok(Reply::Workspace(host.snapshot()))
        }
        Request::TerminalCwd { id } => {
            let host = lock(shared)?;
            let terminal = host
                .terminals
                .get(&id)
                .ok_or_else(|| io::Error::other("terminal not found"))?;
            let live = lock(terminal)?;
            if live.exited {
                return Err(io::Error::other("terminal has exited"));
            }
            let pid = host
                .workspace
                .terminals
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.pid)
                .ok_or_else(|| io::Error::other("terminal PID missing"))?;
            // Explicit split action only, querying this managed shell, never polling.
            let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
            let len = std::mem::size_of_val(&info) as i32;
            let n = unsafe {
                libc::proc_pidinfo(
                    pid as i32,
                    libc::PROC_PIDVNODEPATHINFO,
                    0,
                    (&mut info as *mut libc::proc_vnodepathinfo).cast(),
                    len,
                )
            };
            if n != len {
                return Err(io::Error::last_os_error());
            }
            let path = unsafe { std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) }
                .to_str()
                .map_err(io::Error::other)?
                .to_owned();
            Ok(Reply::Cwd(path))
        }
        Request::StopAgent { id } => {
            let host = lock(shared)?;
            let session = host
                .workspace
                .current_agent(&id)
                .ok_or_else(|| io::Error::other("没有已识别的活动 Agent"))?;
            let pid = host
                .agent_pids
                .iter()
                .find(|(_, sid)| *sid == &session.id)
                .map(|(pid, _)| *pid)
                .ok_or_else(|| io::Error::other("Agent 进程身份未确认，未发送停止信号"))?;
            let processes = crate::recognition::processes(&host.workspace)?;
            if !processes
                .iter()
                .any(|p| p.pid == pid && p.runtime == Some(session.runtime))
                || crate::recognition::terminal_for_process(pid, &processes, &host.workspace)
                    .as_deref()
                    != Some(id.as_str())
            {
                return Err(io::Error::other("Agent 进程身份已变化，未发送停止信号"));
            }
            // A freshly verified Agent PID only; never kill the PTY shell or group.
            if unsafe { libc::kill(pid as i32, libc::SIGTERM) } < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Reply::Ok)
        }
        Request::Stop { id } => {
            let terminal = lock(shared)?
                .terminals
                .get(&id)
                .cloned()
                .ok_or_else(|| io::Error::other("terminal not found"))?;
            stop_terminal(&terminal)?;
            Ok(Reply::Ok)
        }
        _ => Err(io::Error::other("operation requires an attached terminal")),
    }
}

/// SIGTERM one terminal's process group; a terminal that already ended is left alone.
fn stop_terminal(terminal: &LiveTerminal) -> io::Result<()> {
    let live = lock(terminal)?;
    if live.exited && live.drained {
        return Ok(());
    }
    // Only this daemon's process group, never a name-based kill.
    if unsafe { libc::kill(-(live.pid as i32), libc::SIGTERM) } < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

fn record_exit(host: &Shared, terminal: &LiveTerminal, id: &str) -> io::Result<()> {
    // Always acquire global state before terminal state; PTY output never takes global state.
    let mut host = lock(host)?;
    let mut live = lock(terminal)?;
    let mut next = host.workspace.clone();
    let session = next
        .terminals
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| io::Error::other("missing session"))?;
    session.state = if live.exited && live.drained {
        ProcessState::Exited
    } else if live.exited {
        ProcessState::Draining
    } else {
        ProcessState::Running
    };
    session.exit_code = live.exit_code;
    let status = serde_json::to_vec(session).map_err(io::Error::other)?;
    if live.exited {
        for agent in &mut next.agent_sessions {
            if agent.terminal.as_deref() == Some(id) {
                agent.current = false;
            }
        }
    }
    host.commit(next)?;
    if live.exited {
        if let Some(events) = &host.log_events {
            for agent in host
                .workspace
                .agent_sessions
                .iter()
                .filter(|a| a.terminal.as_deref() == Some(id))
            {
                events.file(std::path::PathBuf::from(&agent.transcript));
            }
        }
    }
    live.broadcast(STATUS, status);
    Ok(())
}

fn pump(
    mut reader: File,
    terminal: &LiveTerminal,
    replies: Arc<Mutex<Vec<u8>>>,
    shared: &Shared,
    id: &str,
) -> io::Result<()> {
    let mut bytes = vec![0; 64 * 1024];
    // One screen check per output burst once the screen has been still; no timer otherwise.
    let mut quiet_at: Option<Instant> = None;
    loop {
        let mut poll = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout = quiet_at.map_or(-1, |at| {
            i32::try_from(at.saturating_duration_since(Instant::now()).as_millis())
                .unwrap_or(i32::MAX)
        });
        let ready = unsafe { libc::poll(&mut poll, 1, timeout) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            quiet_at = check_screen_wait(terminal, shared, id)?;
            continue;
        }
        let n = match reader.read(&mut bytes) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) =>
            {
                continue;
            }
            Err(e) if e.raw_os_error() == Some(libc::EIO) => return Ok(()),
            Err(e) => return Err(e),
        };
        quiet_at = Some(Instant::now() + SCREEN_QUIET);
        let mut live = lock(terminal)?;
        live.vt.feed(&bytes[..n]);
        let response = std::mem::take(&mut *lock(&replies)?);
        if !response.is_empty() {
            pty::write(&mut live.master, &response)?;
        }
        live.broadcast(OUTPUT, bytes[..n].to_vec());
        // Compare in place: output chunks are hot, title changes are rare.
        let changed = live
            .vt
            .terminal()
            .title()
            .ok()
            .filter(|t| *t != live.title)
            .map(str::to_owned);
        if let Some(title) = changed {
            live.title = title.clone();
            // Host before Live is the lock order; release this terminal first.
            drop(live);
            lock(shared)?.title_changed(id, &title)?;
        }
    }
}

const SCREEN_QUIET: Duration = Duration::from_secs(1);
/// A fresh hook-reported dialog is not vetoed by the screen yet: it may still be drawing.
const RECONCILE_HOLD: Duration = Duration::from_secs(5);
/// At most one `claude agents --json` per session in this window.
const RECONCILE_EVERY: Duration = Duration::from_secs(5);
/// Menus the user opens appear right after a keystroke; an agent's question does not.
const INPUT_SETTLE: Duration = Duration::from_secs(3);

/// Returns when to look again, if the user typed too recently to attribute the prompt.
fn check_screen_wait(
    terminal: &LiveTerminal,
    shared: &Shared,
    id: &str,
) -> io::Result<Option<Instant>> {
    {
        let host = lock(shared)?;
        if host.workspace.current_agent(id).is_none() && !host.screen_waits.contains_key(id) {
            return Ok(None);
        }
    }
    let (tail, input_at) = {
        let live = lock(terminal)?;
        if live.exited {
            return Ok(None);
        }
        (live.vt.tail_text(usize::from(live.rows))?, live.input_at)
    };
    let question = crate::recognition::screen_wait(&tail);
    let settle = input_at.map(|at| at + INPUT_SETTLE);
    if let Some(settle) = settle.filter(|at| question.is_some() && *at > Instant::now()) {
        return Ok(Some(settle));
    }
    let mut host = lock(shared)?;
    let question = question.filter(|_| host.workspace.current_agent(id).is_some());
    let veto = question.is_none();
    host.set_screen_wait(id, question)?;
    if !veto {
        return Ok(None);
    }
    // A dialog the hooks or the log reported but the screen no longer shows: ask the CLI.
    let Some(session) = host.workspace.current_agent(id).cloned() else {
        return Ok(None);
    };
    let waiting = host
        .workspace
        .content
        .get(&session.id)
        .filter(|c| c.turn == keepline_core::content::Turn::Waiting);
    let Some(card) = waiting else {
        return Ok(None);
    };
    let held = card.signal_at + RECONCILE_HOLD.as_secs();
    let now = keepline_core::content::now();
    if held > now {
        return Ok(Some(Instant::now() + Duration::from_secs(held - now)));
    }
    Ok(host.reconcile(shared, vec![session]))
}

fn apply_official(
    shared: &Shared,
    index: &crate::log_index::Index,
    due: &[(AgentSession, Option<u32>)],
) -> io::Result<()> {
    let (claude, codex): (Vec<_>, Vec<_>) =
        due.iter().partition(|(s, _)| s.runtime == Agent::Claude);
    let mut found: Vec<(&AgentSession, crate::content::Official)> = Vec::new();
    let mut errors = Vec::new();
    // Each CLI's last outcome, shown in the app's recognition notice until one succeeds.
    let mut outcomes: Vec<(&str, Option<String>)> = Vec::new();
    if !claude.is_empty() {
        match claude_status(&claude) {
            Ok(mut f) => {
                found.append(&mut f);
                outcomes.push(("Claude", None));
            }
            Err(e) => {
                errors.push(format!("claude agents --json: {e}"));
                outcomes.push(("Claude", Some(e.to_string())));
            }
        }
    }
    if !codex.is_empty() {
        // A Codex thread id is its session id; `--no-daemon` threads are not loaded there.
        let threads: Vec<String> = codex
            .iter()
            .map(|(s, _)| s.cli_session_id.clone())
            .collect();
        let result = crate::codex_status::socket()
            .ok_or_else(|| io::Error::other("no HOME for the Codex control socket"))
            .and_then(|socket| crate::codex_status::query(&socket, &threads));
        match result {
            Ok(statuses) => {
                found.extend(statuses.into_iter().filter_map(|(thread, status)| {
                    let (s, _) = codex.iter().find(|(s, _)| s.cli_session_id == thread)?;
                    Some((s, status))
                }));
                outcomes.push(("Codex", None));
            }
            Err(e) => {
                errors.push(format!("Codex app-server thread/read: {e}"));
                outcomes.push(("Codex", Some(e.to_string())));
            }
        }
    }
    {
        let mut host = lock(shared)?;
        let before = host.recognition.official.clone();
        for (cli, error) in outcomes {
            match error {
                Some(e) => host.recognition.official.insert(
                    cli.into(),
                    format!("{cli} 官方状态查询不可用，已改为只看日志和画面：{e}"),
                ),
                None => host.recognition.official.remove(cli),
            };
        }
        if host.recognition.official != before {
            host.publish_status()?;
        }
    }
    let mut cards = Vec::new();
    for (session, status) in found {
        index.official(&session.transcript, &status)?;
        if let Some(card) = index.card(&session.transcript)? {
            cards.push((session.id.clone(), card));
        }
    }
    if !cards.is_empty() {
        let mut host = lock(shared)?;
        let mut next = host.workspace.clone();
        for (id, card) in cards {
            next.content.insert(id, card);
        }
        host.commit(next)?;
    }
    match errors.is_empty() {
        true => Ok(()),
        false => Err(io::Error::other(errors.join("; "))),
    }
}

fn claude_status<'a>(
    due: &[&'a (AgentSession, Option<u32>)],
) -> io::Result<Vec<(&'a AgentSession, crate::content::Official)>> {
    let program = match std::env::var("KEEPLINE_CLAUDE_AGENTS") {
        Ok(program) => program,
        Err(_) => due
            .iter()
            .find_map(|(_, pid)| pid.and_then(|p| crate::official::executable(p).ok()))
            .ok_or_else(|| io::Error::other("no live Claude process to ask"))?,
    };
    let entries = crate::official::query(&program)?;
    Ok(due
        .iter()
        .filter_map(|(session, pid)| {
            let entry = entries.iter().find(|e| {
                e.session_id == session.cli_session_id || (pid.is_some() && e.pid == *pid)
            })?;
            Some((session, entry.status.clone()))
        })
        .collect())
}

fn probe_after_submission(shared: &Shared, terminal: &str) -> io::Result<()> {
    let mut host = lock(shared)?;
    if host.workspace.current_agent(terminal).is_some()
        || !host.input_probes.insert(terminal.into())
    {
        return Ok(());
    }
    let shared = shared.clone();
    let terminal = terminal.to_owned();
    thread::spawn(move || {
        // One startup grace after user Enter, not periodic process or filesystem polling.
        thread::sleep(Duration::from_millis(500));
        let result = (|| -> io::Result<()> {
            let transcripts = {
                let mut host = lock(&shared)?;
                host.input_probes.remove(&terminal);
                if host.workspace.current_agent(&terminal).is_some() {
                    return Ok(());
                }
                host.store.transcripts()?
            };
            refresh_agents(&shared, &transcripts)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Agent startup identification: {e}");
        }
    });
    Ok(())
}

struct Attachment {
    terminal: LiveTerminal,
    connection: u64,
    socket: UnixStream,
}
impl Drop for Attachment {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        if let Ok(mut live) = self.terminal.lock() {
            live.subscribers.remove(&self.connection);
            live.reconcile_owner();
        }
    }
}
fn attach(
    mut stream: UnixStream,
    terminal: LiveTerminal,
    connection: u64,
    takeover: bool,
    host: Shared,
    terminal_id: String,
) -> io::Result<()> {
    let guard = Attachment {
        terminal: Arc::clone(&terminal),
        connection,
        socket: stream.try_clone()?,
    };
    let (tx, rx) = sync_channel::<Frame>(128);
    let (snapshot, writable, cols, rows) = {
        let mut live = lock(&terminal)?;
        let snapshot = live.vt.snapshot()?;
        live.reconcile_owner();
        if takeover || live.owner.is_none() {
            live.owner = Some(connection);
        }
        let writable = live.owner == Some(connection) && !live.exited;
        live.subscribers.insert(
            connection,
            Subscriber {
                tx: tx.clone(),
                socket: stream.try_clone()?,
            },
        );
        live.reconcile_owner();
        (snapshot, writable, live.cols, live.rows)
    };
    send(
        &mut stream,
        CONTROL,
        &Reply::Attached {
            writable,
            cols,
            rows,
        },
    )?;
    let ready = Arc::new(AtomicBool::new(false));
    let writer_ready = Arc::clone(&ready);
    let mut writer = stream.try_clone()?;
    let writer_guard = stream.try_clone()?;
    thread::spawn(move || {
        let result = (|| -> io::Result<()> {
            for chunk in snapshot.chunks(MAX_FRAME - 1) {
                write_frame(&mut writer, SNAPSHOT, chunk)?;
            }
            write_frame(&mut writer, READY, &[])?;
            writer_ready.store(true, Ordering::Release);
            for (kind, data) in rx {
                write_frame(&mut writer, kind, &data)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("terminal stream ended: {e}");
        }
        let _ = writer_guard.shutdown(std::net::Shutdown::Both);
    });
    loop {
        let request = receive::<Request>(&mut stream)?;
        let submitted = matches!(&request, Request::Input {bytes, ..} if bytes.contains(&b'\r') || bytes.contains(&b'\n'));
        let typed = matches!(&request, Request::Input { .. });
        let result = (|| -> io::Result<()> {
            let mut live = lock(&terminal)?;
            if !ready.load(Ordering::Acquire) {
                return Err(io::Error::other("snapshot not ready"));
            }
            if live.owner != Some(connection) || live.exited {
                return Err(io::Error::other("只读或进程已退出；需明确接管"));
            }
            match request {
                // A mouse report is dropped once the program falls behind: written into
                // a nearly full input queue it can be cut after its ESC, and the program
                // then reads the rest as typed text ("[<65;20;10M" in Codex's input box).
                Request::Input { lossy: true, .. }
                    if pty::backlog(&live.master).is_some_and(|n| n > POINTER_BACKLOG) => {}
                Request::Input { bytes, lossy } => {
                    if bytes.len() > 64 * 1024 {
                        return Err(io::Error::other("单次输入超过 64 KiB"));
                    }
                    pty::write(&mut live.master, &bytes)?;
                    if !lossy {
                        live.input_at = Some(Instant::now());
                    }
                }
                Request::ClearHistory => {
                    let clear = keepline_core::terminal::clear_history(live.vt.terminal())?;
                    if !clear.is_empty() {
                        // Same bytes into the daemon's terminal and every display, in output order.
                        live.vt.feed(&clear);
                        live.broadcast(OUTPUT, clear);
                    }
                }
                Request::Resize {
                    cols,
                    rows,
                    cell_width,
                    cell_height,
                } => {
                    geometry(cols, rows)?;
                    live.cell.set(cell_width, cell_height);
                    pty::resize(&live.master, cols, rows, (cell_width, cell_height))?;
                    live.vt.resize(cols, rows)?;
                    live.cols = cols;
                    live.rows = rows;
                    live.broadcast(
                        RESIZED,
                        serde_json::to_vec(&(cols, rows)).map_err(io::Error::other)?,
                    );
                }
                _ => {
                    return Err(io::Error::other(
                        "attached connection accepts input and resize only",
                    ));
                }
            }
            Ok(())
        })();
        if typed && result.is_ok() {
            // The user is answering here; the prompt is no longer waiting on them.
            lock(&host)?.set_screen_wait(&terminal_id, None)?;
        }
        if submitted && result.is_ok() {
            {
                let mut shared = lock(&host)?;
                let submits = shared.submits.entry(terminal_id.clone()).or_default();
                submits.push_back(keepline_core::content::now());
                if submits.len() > 8 {
                    submits.pop_front();
                }
            }
            probe_after_submission(&host, &terminal_id)?;
        }
        let reply = result
            .map(|_| Reply::Ok)
            .unwrap_or_else(|e| Reply::Error(e.to_string()));
        tx.try_send((
            ACK,
            Arc::new(serde_json::to_vec(&reply).map_err(io::Error::other)?),
        ))
        .map_err(|_| io::Error::other("client is too slow; reconnect for a new snapshot"))?;
        let _ = &guard;
    }
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
            submits: HashMap::new(),
            input_probes: Default::default(),
            deadlines: None,
            screen_waits: HashMap::new(),
            reconciled: HashMap::new(),
        }));
        let (tx, rx) = sync_channel(16);
        let (socket, _peer) = UnixStream::pair().unwrap();
        lock(&shared)
            .unwrap()
            .recognition_subscribers
            .insert(1, Subscriber { tx, socket });
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
