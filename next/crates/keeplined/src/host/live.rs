//! Live PTYs and their viewers: the read pump, frame fan-out to subscribers, attach,
//! resize and exit reaping.
use super::*;

pub(super) type Frame = (u8, Arc<Vec<u8>>);
/// Unread input above which mouse reports are dropped; the queue holds about 1 KiB.
pub(super) const POINTER_BACKLOG: usize = 256;
pub(super) struct Subscriber {
    pub(super) tx: SyncSender<Frame>,
    pub(super) socket: UnixStream,
    /// Frames were dropped because this viewer fell behind; its writer sends a fresh
    /// snapshot once it catches up instead of the connection being cut.
    pub(super) lagged: Arc<AtomicBool>,
}
impl Subscriber {
    /// Queues a frame. A full queue marks the viewer lagged and keeps it; only a viewer
    /// that has gone away is dropped.
    pub(super) fn offer(&self, frame: Frame) -> bool {
        match self.tx.try_send(frame) {
            Ok(()) => true,
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                self.lagged.store(true, Ordering::Release);
                true
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                let _ = self.socket.shutdown(std::net::Shutdown::Both);
                false
            }
        }
    }
}
pub(super) struct Live {
    pub(super) vt: Vt,
    pub(super) master: File,
    pub(super) cols: u16,
    pub(super) rows: u16,
    pub(super) pid: u32,
    pub(super) owner: Option<u64>,
    pub(super) subscribers: HashMap<u64, Subscriber>,
    pub(super) exited: bool,
    pub(super) drained: bool,
    /// The last STATUS sent, replayed to a viewer that missed it while lagging.
    pub(super) status: Option<Arc<Vec<u8>>>,
    pub(super) exit_code: Option<i32>,
    pub(super) title: String,
    pub(super) cell: CellPixels,
    /// Last keystrokes sent here; a menu the user just opened is not the agent asking.
    pub(super) input_at: Option<std::time::Instant>,
    /// The shell's command marks, read only while a hook listens for commands.
    pub(super) marks: crate::command_marks::Marks,
}
impl Live {
    // Event-driven handoff. No heartbeat or global process scan is needed.
    pub(super) fn reconcile_owner(&mut self) {
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
                s.offer((WRITABLE, Arc::new(writable_frame(writable))))
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
    pub(super) fn broadcast(&mut self, kind: u8, bytes: Vec<u8>) {
        let bytes = Arc::new(bytes);
        if kind == STATUS {
            self.status = Some(Arc::clone(&bytes));
        }
        self.subscribers
            .retain(|_, subscriber| subscriber.offer((kind, Arc::clone(&bytes))));
    }
}

pub(super) fn writable_frame(writable: bool) -> Vec<u8> {
    if writable {
        b"true".to_vec()
    } else {
        b"false".to_vec()
    }
}
/// Everything a viewer that fell behind needs to be current again, taken under the
/// terminal lock with its stale queue emptied: those frames are already in the snapshot.
pub(super) fn resync(
    terminal: &LiveTerminal,
    connection: u64,
    rx: &std::sync::mpsc::Receiver<Frame>,
) -> io::Result<Vec<Frame>> {
    let mut live = lock(terminal)?;
    while rx.try_recv().is_ok() {}
    if let Some(s) = live.subscribers.get(&connection) {
        s.lagged.store(false, Ordering::Release);
    }
    let snapshot = live.vt.snapshot()?;
    let mut frames: Vec<Frame> = snapshot
        .chunks(MAX_FRAME - 1)
        .map(|c| (SNAPSHOT, Arc::new(c.to_vec())))
        .collect();
    frames.push((READY, Arc::new(vec![])));
    let writable = live.owner == Some(connection) && !live.exited;
    frames.push((WRITABLE, Arc::new(writable_frame(writable))));
    if let Some(status) = &live.status {
        frames.push((STATUS, Arc::clone(status)));
    }
    Ok(frames)
}

/// Hang up one terminal, as closing a terminal window does: SIGHUP to the shell's
/// process group and to its foreground job's. An interactive zsh ignores SIGTERM and
/// runs an Agent in a group of its own, so SIGTERM to the shell's group alone ended
/// neither. A terminal that already ended is left alone.
pub(super) fn stop_terminal(terminal: &LiveTerminal) -> io::Result<()> {
    let live = lock(terminal)?;
    if live.exited && live.drained {
        return Ok(());
    }
    let shell = live.pid as i32;
    let foreground = crate::pty::foreground_group(&live.master).filter(|&g| g != shell);
    // Only this terminal's own process groups, never a name-based kill.
    for group in std::iter::once(shell).chain(foreground) {
        if unsafe { libc::kill(-group, libc::SIGHUP) } < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
    }
    Ok(())
}

pub(super) fn record_exit(host: &Shared, terminal: &LiveTerminal, id: &str) -> io::Result<()> {
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

/// How a terminal's process end is seen: a child this host spawned, or, after a handoff,
/// a process the previous host started, watched by pid (its exit code is then unknown).
pub(super) enum Reaper {
    Child(std::process::Child),
    Pid(u32),
}
/// Registers a running terminal and starts reading its output and waiting for its end.
pub(super) fn run_live(
    host: &mut Host,
    shared: &Shared,
    id: String,
    live: Live,
    reader: File,
    replies: Arc<Mutex<Vec<u8>>>,
    reaper: Reaper,
) {
    let terminal = Arc::new(Mutex::new(live));
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
    let ended = move |code: Option<i32>| {
        if let Ok(mut live) = lock(&terminal) {
            live.exited = true;
            live.exit_code = code;
        }
        if let Err(e) = record_exit(&shared_wait, &terminal, &id) {
            eprintln!("persist exit {id}: {e}");
        }
    };
    match reaper {
        Reaper::Child(mut child) => {
            thread::spawn(move || match child.wait() {
                Ok(status) => ended(status.code()),
                Err(error) => eprintln!("wait terminal: {error}"),
            });
        }
        Reaper::Pid(pid) => {
            let ended = Arc::new(Mutex::new(Some(ended)));
            let fire = Arc::clone(&ended);
            let watch = crate::process_exit::ExitWatch::new(move |_| {
                if let Some(ended) = fire.lock().ok().and_then(|mut e| e.take()) {
                    ended(None);
                }
            });
            match watch.and_then(|w| w.watch(pid)) {
                Ok(true) => (),
                // Gone before the watch began, or no watch at all: settle it now.
                _ => {
                    if let Some(ended) = ended.lock().ok().and_then(|mut e| e.take()) {
                        ended(None);
                    }
                }
            }
        }
    }
}

pub(super) fn pump(
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
        // Only reading the PTY ends this loop. Screen checks, query replies and titles are
        // side work: if they fail, the terminal must keep streaming instead of freezing.
        if ready == 0 {
            quiet_at = check_screen_wait(terminal, shared, id).unwrap_or_else(|e| {
                eprintln!("terminal {id}: screen check: {e}");
                None
            });
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
            if let Err(e) = pty::write(&mut live.master, &response) {
                eprintln!("terminal {id}: query reply: {e}");
            }
        }
        live.broadcast(OUTPUT, bytes[..n].to_vec());
        let finished = if keepline_core::hook::commands()
            || super::events::SUBSCRIBED.load(Ordering::Acquire)
        {
            live.marks.scan(&bytes[..n], Instant::now())
        } else {
            Vec::new()
        };
        if !finished.is_empty() {
            let pwd = live.vt.pwd(&super::hostname());
            // Host before Live is the lock order; release this terminal first.
            drop(live);
            match lock(shared) {
                Ok(host) => host.commands_finished(id, pwd, finished),
                Err(e) => eprintln!("terminal {id}: command hooks: {e}"),
            }
            live = lock(terminal)?;
        }
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
            if let Err(e) = lock(shared).and_then(|mut host| host.title_changed(id, &title)) {
                eprintln!("terminal {id}: title: {e}");
            }
        }
    }
}

pub(super) struct Attachment {
    pub(super) terminal: LiveTerminal,
    pub(super) connection: u64,
    pub(super) socket: UnixStream,
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
pub(super) fn attach(
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
    let lagged = Arc::new(AtomicBool::new(false));
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
                lagged: Arc::clone(&lagged),
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
    let writer_terminal = Arc::clone(&terminal);
    let writer_lagged = Arc::clone(&lagged);
    thread::spawn(move || {
        let result = (|| -> io::Result<()> {
            for chunk in snapshot.chunks(MAX_FRAME - 1) {
                write_frame(&mut writer, SNAPSHOT, chunk)?;
            }
            write_frame(&mut writer, READY, &[])?;
            writer_ready.store(true, Ordering::Release);
            for (kind, data) in rx.iter() {
                if writer_lagged.load(Ordering::Acquire) {
                    // A burst outran this viewer: skip the stale backlog and send the
                    // current screen instead of cutting the connection.
                    for (kind, data) in resync(&writer_terminal, connection, &rx)? {
                        write_frame(&mut writer, kind, &data)?;
                    }
                    continue;
                }
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
                Request::Scheme(scheme) => live.vt.set_scheme(scheme)?,
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
                        "attached connection accepts input, resize and scheme only",
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
            record_submission(&host, &terminal_id)?;
        }
        let reply = result
            .map(|_| Reply::Ok)
            .unwrap_or_else(|e| Reply::Error(e.to_string()));
        // A lagging viewer misses this acknowledgement; its resync says where it stands.
        if let Err(std::sync::mpsc::TrySendError::Full(_)) = tx.try_send((
            ACK,
            Arc::new(serde_json::to_vec(&reply).map_err(io::Error::other)?),
        )) {
            lagged.store(true, Ordering::Release);
        }
        let _ = &guard;
    }
}
