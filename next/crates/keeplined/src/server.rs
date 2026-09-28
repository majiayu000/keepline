use std::collections::HashMap;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{json, Value};

use crate::grid::{self, PtyProcess};
use crate::intent::{self, IntentRecord, IntentState};
use crate::protocol::{
    self, error_response, ok_response, PullClass, Request, DELTA_LIMIT, INTENT_DIR_NAME,
    PROTOCOL_MAJOR, PROTOCOL_MINOR, SOCKET_FILE_NAME,
};
use crate::session::{spawn_reader, DaemonState, Lease, LiveSession, Slot};

static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

struct OpError {
    code: &'static str,
    message: String,
}

impl OpError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn io(code: &'static str, err: io::Error) -> Self {
        Self::new(code, err.to_string())
    }
}

struct LeaseGuard {
    state: Arc<Mutex<DaemonState>>,
    conn_id: u64,
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("keeplined: state lock poisoned while revoking a lease");
                poisoned.into_inner()
            }
        };
        for slot in guard.terminals.values_mut() {
            if let Slot::Live(live) = slot {
                if live
                    .lease
                    .as_ref()
                    .is_some_and(|lease| lease.owner == self.conn_id)
                {
                    live.lease = None;
                }
            }
        }
    }
}

pub fn serve(runtime_dir: &Path) -> io::Result<()> {
    // SAFETY: umask is process-global and this runs before worker threads exist.
    unsafe {
        libc::umask(0o077);
    }
    fs::create_dir_all(runtime_dir)?;
    let runtime_dir = runtime_dir.canonicalize()?;
    intent::set_mode(&runtime_dir, 0o700)?;
    intent::require_mode(&runtime_dir, 0o700, "runtime directory")?;
    let intent_dir = runtime_dir.join(INTENT_DIR_NAME);
    fs::create_dir_all(&intent_dir)?;
    intent::set_mode(&intent_dir, 0o700)?;

    let socket_path = runtime_dir.join(SOCKET_FILE_NAME);
    claim_idle_socket(&socket_path)?;
    let listener = UnixListener::bind(&socket_path)?;
    intent::set_mode(&socket_path, 0o600)?;
    intent::require_mode(&socket_path, 0o600, "socket")?;

    let state = Arc::new(Mutex::new(load_state(&runtime_dir)?));
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let state = Arc::clone(&state);
                let conn_id = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
                thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, state, conn_id) {
                        if !protocol::is_disconnect(&err) {
                            eprintln!("keeplined: connection {conn_id} closed: {err}");
                        }
                    }
                });
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        }
    }
}

/// Unlink the socket only when connect proves nothing is listening.
fn claim_idle_socket(socket_path: &Path) -> io::Result<()> {
    match UnixStream::connect(socket_path) {
        Ok(_live) => Err(io::Error::new(
            ErrorKind::AddrInUse,
            "runtime socket already accepts connections",
        )),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == ErrorKind::ConnectionRefused => {
            match fs::remove_file(socket_path) {
                Ok(()) => Ok(()),
                Err(remove) if remove.kind() == ErrorKind::NotFound => Ok(()),
                Err(remove) => Err(remove),
            }
        }
        Err(err) => Err(err),
    }
}

fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<DaemonState>>,
    conn_id: u64,
) -> io::Result<()> {
    let _lease_guard = LeaseGuard {
        state: Arc::clone(&state),
        conn_id,
    };
    let peer_uid = peer_uid(&stream)?;
    if peer_uid != current_uid() {
        return Err(io::Error::new(
            ErrorKind::PermissionDenied,
            "peer uid does not match the daemon user",
        ));
    }

    let mut negotiated = false;
    loop {
        let frame = match protocol::read_frame(&mut stream) {
            Ok(frame) => frame,
            Err(err) if protocol::is_disconnect(&err) => return Ok(()),
            Err(err) => return Err(err),
        };
        let response = dispatch(&state, conn_id, &mut negotiated, &frame);
        let bytes = serde_json::to_vec(&response).map_err(|err| {
            io::Error::new(ErrorKind::InvalidData, format!("encode response: {err}"))
        })?;
        protocol::write_frame(&mut stream, &bytes)?;
    }
}

fn dispatch(
    state: &Arc<Mutex<DaemonState>>,
    conn_id: u64,
    negotiated: &mut bool,
    frame: &[u8],
) -> Value {
    let request = match protocol::decode_request(frame) {
        Ok(request) => request,
        Err(response) => return response,
    };
    if let Some(major) = request.major {
        if major != PROTOCOL_MAJOR {
            return error_response(
                &request.id,
                "unsupported_protocol",
                format!(
                    "protocol major {major} is not supported; this daemon speaks major {PROTOCOL_MAJOR}"
                ),
            );
        }
    }
    if request.op == "hello" {
        if request.major != Some(PROTOCOL_MAJOR) {
            return error_response(
                &request.id,
                "unsupported_protocol",
                "hello must set major to 0",
            );
        }
        *negotiated = true;
        return ok_response(
            &request.id,
            json!({
                "major": PROTOCOL_MAJOR,
                "minor": PROTOCOL_MINOR,
                "experimental": true,
                "delta_limit": DELTA_LIMIT,
            }),
        );
    }
    if !*negotiated {
        return error_response(
            &request.id,
            "not_negotiated",
            "send hello before other operations",
        );
    }

    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => return error_response(&request.id, "internal", "daemon state lock is poisoned"),
    };
    let result = match request.op.as_str() {
        "launch" => launch(state, &mut guard, &request),
        "attach" => attach(&mut guard, &request),
        "pull" => pull(&mut guard, &request),
        "acquire" => acquire(&mut guard, conn_id, &request),
        "takeover" => takeover(&mut guard, conn_id, &request),
        "input" => input(&mut guard, &request),
        "resize" => resize(&mut guard, &request),
        other => Err(OpError::new(
            "unknown_op",
            format!("unknown operation {other}"),
        )),
    };
    match result {
        Ok(body) => ok_response(&request.id, body),
        Err(err) => error_response(&request.id, err.code, err.message),
    }
}

fn launch(
    arc: &Arc<Mutex<DaemonState>>,
    state: &mut DaemonState,
    request: &Request,
) -> Result<Value, OpError> {
    let operation_id = required_str(request.operation_id.as_deref(), "operation_id")?;
    if !intent::valid_operation_id(operation_id) {
        return Err(OpError::new(
            "operation_rejected",
            "operation_id must be 1 to 80 characters of [A-Za-z0-9_-]",
        ));
    }
    let argv = request
        .argv
        .clone()
        .ok_or_else(|| OpError::new("argv_rejected", "argv is required"))?;
    intent::validate_argv(&argv).map_err(|err| OpError::new("argv_rejected", err))?;
    let cwd = required_str(request.cwd.as_deref(), "cwd")?;
    let cwd_path = intent::intent_cwd(cwd);
    let cols = request.cols.unwrap_or(80);
    let rows = request.rows.unwrap_or(24);
    intent::validate_geometry(cols, rows).map_err(|err| OpError::new("geometry_rejected", err))?;
    let payload = intent::canonical_payload(&argv, cwd, cols, rows)
        .map_err(|err| OpError::io("persist_failed", err))?;

    if let Some(terminal_id) = state.operations.get(operation_id).cloned() {
        return replay(state, &terminal_id, &payload);
    }
    intent::validate_cwd(&cwd_path).map_err(|err| OpError::new("cwd_rejected", err))?;

    let terminal_id = format!(
        "t{}",
        intent::random_hex(16).map_err(|err| OpError::io("internal", err))?
    );
    let intent = IntentRecord {
        operation_id: operation_id.to_owned(),
        terminal_id: terminal_id.clone(),
        argv: argv.clone(),
        cwd: cwd.to_owned(),
        cols,
        rows,
        payload,
        state: IntentState::Intent,
        pid: None,
        instance_generation: 1,
        error: None,
    };
    intent::write_intent(&state.runtime_dir, &intent)
        .map_err(|err| OpError::io("persist_failed", err))?;
    state
        .operations
        .insert(operation_id.to_owned(), terminal_id.clone());
    state
        .terminals
        .insert(terminal_id, Slot::Stored(intent.clone()));

    match grid::spawn_pty(&argv, &cwd_path, cols, rows) {
        Ok(pty) => finish_launch(arc, state, intent, pty),
        Err(err) => fail_spawn(state, intent, err),
    }
}

fn fail_spawn(
    state: &mut DaemonState,
    mut intent: IntentRecord,
    err: io::Error,
) -> Result<Value, OpError> {
    intent.state = IntentState::Failed;
    intent.error = Some(err.to_string());
    let persisted = intent::write_intent(&state.runtime_dir, &intent);
    state
        .terminals
        .insert(intent.terminal_id.clone(), Slot::Stored(intent));
    if let Err(write_err) = persisted {
        return Err(OpError::new(
            "persist_failed",
            format!("{err}; failed to record it: {write_err}"),
        ));
    }
    Err(OpError::new("spawn_failed", err.to_string()))
}

fn finish_launch(
    arc: &Arc<Mutex<DaemonState>>,
    state: &mut DaemonState,
    mut intent: IntentRecord,
    pty: PtyProcess,
) -> Result<Value, OpError> {
    let (mut child, master, cols, rows) = match pty.into_parts() {
        Ok(parts) => parts,
        Err(err) => return fail_spawn(state, intent, err),
    };
    intent.state = IntentState::Running;
    intent.pid = Some(child.id());
    intent.cols = cols;
    intent.rows = rows;
    intent.error = None;
    if let Err(err) = intent::write_intent(&state.runtime_dir, &intent) {
        return abort_spawn(state, intent, &mut child, err);
    }
    state
        .terminals
        .insert(intent.terminal_id.clone(), Slot::Stored(intent.clone()));

    let reader = match master.try_clone() {
        Ok(reader) => reader,
        Err(err) => return abort_spawn(state, intent, &mut child, err),
    };
    let pid = child.id();
    let terminal_id = intent.terminal_id.clone();
    let response = json!({
        "terminal_id": terminal_id,
        "instance_generation": intent.instance_generation,
        "pid": pid,
        "spawned": true,
        "attachable": true,
        "cols": cols,
        "rows": rows,
    });
    state.terminals.insert(
        terminal_id.clone(),
        Slot::Live(Box::new(LiveSession::new(
            intent, child, master, cols, rows,
        ))),
    );
    spawn_reader(reader, Arc::clone(arc), terminal_id);
    Ok(response)
}

fn abort_spawn(
    state: &mut DaemonState,
    intent: IntentRecord,
    child: &mut Child,
    err: io::Error,
) -> Result<Value, OpError> {
    let err = match (child.kill(), child.wait()) {
        (Ok(()), Ok(_)) => err,
        (kill, wait) => io::Error::other(format!(
            "{err}; failed to stop child (kill={kill:?}, wait={wait:?})"
        )),
    };
    fail_spawn(state, intent, err)
}

fn replay(state: &DaemonState, terminal_id: &str, payload: &str) -> Result<Value, OpError> {
    let slot = state
        .terminals
        .get(terminal_id)
        .ok_or_else(|| OpError::new("internal", "operation mapped to a missing terminal"))?;
    let intent = match slot {
        Slot::Live(live) => &live.intent,
        Slot::Stored(intent) => intent,
    };
    if intent.payload != payload {
        return Err(OpError::new(
            "operation_conflict",
            "operation_id was already used with a different launch payload",
        ));
    }
    match slot {
        Slot::Live(live) => Ok(json!({
            "terminal_id": live.intent.terminal_id,
            "instance_generation": live.intent.instance_generation,
            "pid": live.child.id(),
            "spawned": false,
            "attachable": true,
            "cols": live.cols,
            "rows": live.rows,
        })),
        Slot::Stored(intent) => match intent.state {
            IntentState::Failed => Err(OpError::new(
                "spawn_failed",
                intent
                    .error
                    .clone()
                    .unwrap_or_else(|| "spawn failed".to_owned()),
            )),
            IntentState::Intent => Err(OpError::new(
                "operation_incomplete",
                "launch intent was recorded but the child was not spawned",
            )),
            IntentState::Running => Ok(json!({
                "terminal_id": intent.terminal_id,
                "instance_generation": intent.instance_generation,
                "pid": intent.pid,
                "spawned": false,
                "attachable": false,
                "cols": intent.cols,
                "rows": intent.rows,
            })),
        },
    }
}

fn attach(state: &mut DaemonState, request: &Request) -> Result<Value, OpError> {
    live_mut(state, request)?
        .snapshot()
        .map_err(|err| OpError::io("pty_io", err))
}

fn pull(state: &mut DaemonState, request: &Request) -> Result<Value, OpError> {
    let after = request
        .after_revision
        .ok_or_else(|| OpError::new("revision_required", "after_revision is required"))?;
    let live = live_mut(state, request)?;
    live.refresh_exit();
    match protocol::classify_pull(live.oldest_retained(), live.revision, after) {
        PullClass::Ahead => Err(OpError::new(
            "revision_ahead",
            format!(
                "after_revision {after} is newer than revision {}",
                live.revision
            ),
        )),
        PullClass::Current => Ok(json!({
            "resync_required": false,
            "revision": live.revision,
            "deltas": [],
        })),
        PullClass::Deltas => {
            let revision = live.revision;
            let deltas: Vec<Value> = live
                .deltas
                .iter()
                .filter(|delta| delta.revision > after)
                .map(|delta| {
                    json!({
                        "revision": delta.revision,
                        "checksum": format!("{:016x}", delta.checksum),
                    })
                })
                .collect();
            Ok(json!({
                "resync_required": false,
                "revision": revision,
                "deltas": deltas,
            }))
        }
        PullClass::Resync => {
            let mut snapshot = live.snapshot().map_err(|err| OpError::io("pty_io", err))?;
            snapshot["resync_required"] = json!(true);
            Ok(snapshot)
        }
    }
}

fn acquire(state: &mut DaemonState, conn_id: u64, request: &Request) -> Result<Value, OpError> {
    let live = live_mut(state, request)?;
    if live.lease.is_some() {
        return Err(OpError::new(
            "lease_held",
            "terminal already has an input owner; takeover is required",
        ));
    }
    grant_lease(live, conn_id)
}

fn takeover(state: &mut DaemonState, conn_id: u64, request: &Request) -> Result<Value, OpError> {
    grant_lease(live_mut(state, request)?, conn_id)
}

fn grant_lease(live: &mut LiveSession, conn_id: u64) -> Result<Value, OpError> {
    live.lease_generation = live.lease_generation.saturating_add(1);
    let token = intent::random_hex(16).map_err(|err| OpError::io("internal", err))?;
    let generation = live.lease_generation;
    live.lease = Some(Lease {
        generation,
        token: token.clone(),
        owner: conn_id,
    });
    Ok(json!({
        "terminal_id": live.intent.terminal_id,
        "generation": generation,
        "token": token,
    }))
}

fn input(state: &mut DaemonState, request: &Request) -> Result<Value, OpError> {
    let data = request
        .data
        .clone()
        .ok_or_else(|| OpError::new("input_rejected", "data is required"))?;
    if data.is_empty() || data.len() > 64 * 1024 {
        return Err(OpError::new(
            "input_rejected",
            "input must be 1 to 65536 bytes",
        ));
    }
    let live = live_mut(state, request)?;
    ensure_lease(live, request)?;
    let accepted = grid::write_pty(&mut live.master, data.as_bytes()).map_err(|err| {
        let code = if err.kind() == ErrorKind::WouldBlock {
            "pty_backpressure"
        } else {
            "pty_io"
        };
        OpError::new(code, err.to_string())
    })?;
    Ok(json!({ "accepted": accepted }))
}

fn resize(state: &mut DaemonState, request: &Request) -> Result<Value, OpError> {
    let cols = request
        .cols
        .ok_or_else(|| OpError::new("geometry_rejected", "cols is required"))?;
    let rows = request
        .rows
        .ok_or_else(|| OpError::new("geometry_rejected", "rows is required"))?;
    intent::validate_geometry(cols, rows).map_err(|err| OpError::new("geometry_rejected", err))?;
    let live = live_mut(state, request)?;
    ensure_lease(live, request)?;
    let previous_cols = live.cols;
    let previous_rows = live.rows;
    grid::set_winsize(&live.master, cols, rows).map_err(|err| OpError::io("pty_io", err))?;
    let (actual_cols, actual_rows) = match grid::read_winsize(&live.master) {
        Ok(size) => size,
        Err(err) => {
            if let Err(rollback) = grid::set_winsize(&live.master, previous_cols, previous_rows) {
                return Err(OpError::new(
                    "pty_io",
                    format!("winsize read failed ({err}) and rollback failed ({rollback})"),
                ));
            }
            return Err(OpError::io("pty_io", err));
        }
    };
    live.cols = actual_cols;
    live.rows = actual_rows;
    live.screen.resize(actual_cols, actual_rows);
    live.push_revision();
    Ok(json!({ "cols": actual_cols, "rows": actual_rows }))
}

fn ensure_lease(live: &LiveSession, request: &Request) -> Result<(), OpError> {
    let lease = live.lease.as_ref().ok_or_else(|| {
        OpError::new(
            "lease_rejected",
            "input and resize require an explicit fencing lease",
        )
    })?;
    if Some(lease.generation) != request.generation
        || Some(lease.token.as_str()) != request.token.as_deref()
    {
        return Err(OpError::new("lease_rejected", "fencing token was rejected"));
    }
    Ok(())
}

fn live_mut<'a>(
    state: &'a mut DaemonState,
    request: &Request,
) -> Result<&'a mut LiveSession, OpError> {
    let terminal_id = required_str(request.terminal_id.as_deref(), "terminal_id")?;
    match state.terminals.get_mut(terminal_id) {
        Some(Slot::Live(live)) => Ok(live),
        Some(Slot::Stored(_)) => Err(OpError::new(
            "not_attachable",
            "this process does not own the recorded pty",
        )),
        None => Err(OpError::new(
            "not_found",
            format!("unknown terminal {terminal_id}"),
        )),
    }
}

fn load_state(runtime_dir: &Path) -> io::Result<DaemonState> {
    let mut state = DaemonState {
        runtime_dir: runtime_dir.to_path_buf(),
        terminals: HashMap::new(),
        operations: HashMap::new(),
    };
    for intent in intent::load_intents(runtime_dir)? {
        state
            .operations
            .insert(intent.operation_id.clone(), intent.terminal_id.clone());
        state
            .terminals
            .insert(intent.terminal_id.clone(), Slot::Stored(intent));
    }
    Ok(state)
}

fn required_str<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, OpError> {
    match value {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(OpError::new("bad_frame", format!("{name} is required"))),
    }
}

fn current_uid() -> u32 {
    // SAFETY: getuid has no inputs and cannot fail.
    unsafe { libc::getuid() }
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut uid = 0u32;
    let mut gid = 0u32;
    // SAFETY: stream is a connected Unix socket and both ids are writable out-params.
    let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if rc != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(uid)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io;
    use std::process::Command;

    use serde_json::Value;

    use super::{abort_spawn, replay, DaemonState};
    use crate::intent::{self, IntentRecord, IntentState};
    use crate::protocol::INTENT_DIR_NAME;
    use crate::session::Slot;

    #[test]
    fn aborted_launch_reaps_the_child_and_replay_is_spawn_failed() {
        let runtime = std::env::temp_dir().join(format!(
            "keeplined-abort-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&runtime);
        fs::create_dir_all(&runtime).expect("runtime");
        let cwd = runtime.to_str().expect("utf-8 runtime path").to_owned();
        let argv = vec!["/bin/sleep".to_owned(), "30".to_owned()];
        let payload = intent::canonical_payload(&argv, &cwd, 80, 24).expect("payload");
        let intent = IntentRecord {
            operation_id: "op-abort".to_owned(),
            terminal_id: "t-abort".to_owned(),
            argv,
            cwd,
            cols: 80,
            rows: 24,
            payload: payload.clone(),
            state: IntentState::Running,
            pid: None,
            instance_generation: 1,
            error: None,
        };
        intent::write_intent(&runtime, &intent).expect("intent");
        let mut state = DaemonState {
            runtime_dir: runtime.clone(),
            terminals: std::collections::HashMap::new(),
            operations: std::collections::HashMap::new(),
        };
        state
            .operations
            .insert(intent.operation_id.clone(), intent.terminal_id.clone());
        state
            .terminals
            .insert(intent.terminal_id.clone(), Slot::Stored(intent.clone()));
        let mut child = Command::new("/bin/sleep").arg("30").spawn().expect("sleep");
        let pid = child.id();
        let err = abort_spawn(
            &mut state,
            intent,
            &mut child,
            io::Error::other("reader clone failed"),
        )
        .expect_err("abort");
        assert_eq!(err.code, "spawn_failed");
        let state_text = process_state(pid);
        assert!(
            state_text.is_none(),
            "pid {pid} still present: {state_text:?}"
        );
        let again = replay(&state, "t-abort", &payload).expect_err("replay");
        assert_eq!(again.code, "spawn_failed");
        let record: Value = serde_json::from_str(
            &fs::read_to_string(runtime.join(INTENT_DIR_NAME).join("op-abort.json")).expect("read"),
        )
        .expect("json");
        assert_eq!(record["state"], "failed");
        let _ = fs::remove_dir_all(&runtime);
    }

    fn process_state(pid: u32) -> Option<String> {
        let output = Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "state="])
            .output()
            .expect("ps");
        let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if state.is_empty() {
            None
        } else {
            Some(state)
        }
    }
}
