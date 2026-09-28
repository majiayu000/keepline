use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use crate::grid::{self, Screen};
use crate::intent::IntentRecord;
use crate::protocol::DELTA_LIMIT;

pub(crate) struct Delta {
    pub revision: u64,
    pub checksum: u64,
    pub text: String,
}

pub(crate) struct Lease {
    pub generation: u64,
    pub token: String,
    pub owner: u64,
}

pub(crate) struct LiveSession {
    pub intent: IntentRecord,
    pub child: Option<Child>,
    pub master: File,
    pub screen: Screen,
    pub cols: u16,
    pub rows: u16,
    pub revision: u64,
    pub deltas: VecDeque<Delta>,
    pub lease_generation: u64,
    pub lease: Option<Lease>,
    pending: Vec<u8>,
    exited: bool,
    exit_code: Option<i32>,
    wait_error: Option<String>,
}

pub(crate) enum Slot {
    Live(Box<LiveSession>),
    Stored(IntentRecord),
}

pub(crate) struct DaemonState {
    pub runtime_dir: PathBuf,
    pub terminals: HashMap<String, Slot>,
    pub operations: HashMap<String, String>,
}

impl Drop for LiveSession {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if let Err(err) = child.kill() {
            eprintln!(
                "keeplined: failed to stop {}: {err}",
                self.intent.terminal_id
            );
        }
        if let Err(err) = child.wait() {
            eprintln!(
                "keeplined: failed to reap {}: {err}",
                self.intent.terminal_id
            );
        }
    }
}

impl LiveSession {
    pub(crate) fn new(
        intent: IntentRecord,
        child: Child,
        master: File,
        cols: u16,
        rows: u16,
    ) -> Self {
        Self {
            intent,
            child: Some(child),
            master,
            screen: Screen::new(cols, rows),
            cols,
            rows,
            revision: 0,
            deltas: VecDeque::new(),
            lease_generation: 0,
            lease: None,
            pending: Vec::new(),
            exited: false,
            exit_code: None,
            wait_error: None,
        }
    }

    pub(crate) fn pid(&self) -> u32 {
        self.child
            .as_ref()
            .map(Child::id)
            .or(self.intent.pid)
            .unwrap_or(0)
    }

    pub(crate) fn alive(&self) -> bool {
        !self.exited
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    pub(crate) fn refresh_exit(&mut self) {
        if self.exited {
            return;
        }
        let status = {
            let Some(child) = self.child.as_mut() else {
                return;
            };
            match child.try_wait() {
                Ok(Some(status)) => Some(Ok(status)),
                Ok(None) => None,
                Err(err) => Some(Err(err)),
            }
        };
        match status {
            Some(Ok(status)) => self.note_exit(status),
            Some(Err(err)) => self.note_wait_error(err),
            None => {}
        }
    }

    fn note_exit(&mut self, status: ExitStatus) {
        self.exited = true;
        self.exit_code = status.code();
        self.child = None;
    }

    fn note_wait_error(&mut self, err: io::Error) {
        if self.wait_error.is_none() {
            eprintln!(
                "keeplined: could not check child {}: {err}",
                self.intent.terminal_id
            );
            self.wait_error = Some(err.to_string());
        }
    }

    pub(crate) fn oldest_retained(&self) -> Option<u64> {
        self.deltas.front().map(|delta| delta.revision)
    }

    pub(crate) fn push_revision(&mut self) {
        let view = self.screen.view();
        self.revision = self.revision.saturating_add(1);
        self.deltas.push_back(Delta {
            revision: self.revision,
            checksum: view.checksum,
            text: view.text,
        });
        while self.deltas.len() > DELTA_LIMIT {
            self.deltas.pop_front();
        }
    }

    pub(crate) fn ingest(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let replies = self.screen.advance(bytes);
        self.push_revision();
        if !replies.is_empty() {
            self.enqueue_output(&replies);
        }
    }

    pub(crate) fn accept_input(&mut self, data: &[u8]) -> io::Result<()> {
        self.flush_pending()?;
        if !self.pending.is_empty() {
            return Err(backpressure());
        }
        let written = grid::write_available(&mut self.master, data)?;
        if written == data.len() {
            return Ok(());
        }
        if written == 0 {
            return Err(backpressure());
        }
        self.pending.extend_from_slice(&data[written..]);
        Ok(())
    }

    pub(crate) fn flush_pending(&mut self) -> io::Result<()> {
        while !self.pending.is_empty() {
            let written = grid::write_available(&mut self.master, &self.pending)?;
            if written == 0 {
                return Ok(());
            }
            self.pending.drain(..written);
        }
        Ok(())
    }

    fn enqueue_output(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        if let Err(err) = self.flush_pending() {
            eprintln!(
                "keeplined: failed to write terminal reply for {}: {err}",
                self.intent.terminal_id
            );
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub(crate) fn snapshot(&mut self) -> io::Result<Value> {
        self.refresh_exit();
        let (cols, rows) = grid::read_winsize(&self.master)?;
        self.cols = cols;
        self.rows = rows;
        let view = self.screen.view();
        Ok(json!({
            "terminal_id": self.intent.terminal_id,
            "instance_generation": self.intent.instance_generation,
            "pid": self.pid(),
            "alive": self.alive(),
            "exit_code": self.exit_code,
            "attachable": true,
            "revision": self.revision,
            "oldest_retained_revision": self.oldest_retained().unwrap_or(self.revision),
            "cols": cols,
            "rows": rows,
            "checksum": format!("{:016x}", view.checksum),
            "text_checksum": format!("{:016x}", view.text_checksum),
            "attributed_cells": view.attributed_cells,
            "text": view.text,
            "lease_generation": self.lease_generation,
        }))
    }
}

fn backpressure() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "pty backpressure")
}

pub(crate) fn spawn_reader(reader: File, state: Arc<Mutex<DaemonState>>, terminal_id: String) {
    std::thread::spawn(move || reader_loop(reader, state, terminal_id));
}

fn reader_loop(mut reader: File, state: Arc<Mutex<DaemonState>>, terminal_id: String) {
    let mut buf = [0u8; 8192];
    loop {
        let want_write = has_pending(&state, &terminal_id);
        match grid::wait_pty(reader.as_raw_fd(), want_write, 200) {
            Ok(ready) => {
                if ready.writable {
                    flush_output(&state, &terminal_id);
                }
                if ready.readable {
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => {
                                reap_child(&state, &terminal_id);
                                return;
                            }
                            Ok(n) => ingest(&state, &terminal_id, &buf[..n]),
                            Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                            Err(err) => {
                                eprintln!("keeplined: pty read failed for {terminal_id}: {err}");
                                reap_child(&state, &terminal_id);
                                return;
                            }
                        }
                    }
                } else if !session_open(&state, &terminal_id) {
                    return;
                }
            }
            Err(err) => {
                eprintln!("keeplined: pty poll failed for {terminal_id}: {err}");
                reap_child(&state, &terminal_id);
                return;
            }
        }
    }
}

fn ingest(state: &Mutex<DaemonState>, terminal_id: &str, bytes: &[u8]) {
    let Some(mut guard) = lock_state(state) else {
        return;
    };
    if let Some(Slot::Live(live)) = guard.terminals.get_mut(terminal_id) {
        live.ingest(bytes);
        live.refresh_exit();
    }
}

fn flush_output(state: &Mutex<DaemonState>, terminal_id: &str) {
    let Some(mut guard) = lock_state(state) else {
        return;
    };
    if let Some(Slot::Live(live)) = guard.terminals.get_mut(terminal_id) {
        if let Err(err) = live.flush_pending() {
            eprintln!("keeplined: failed to flush {terminal_id}: {err}");
        }
    }
}

fn has_pending(state: &Mutex<DaemonState>, terminal_id: &str) -> bool {
    let Some(guard) = lock_state(state) else {
        return false;
    };
    match guard.terminals.get(terminal_id) {
        Some(Slot::Live(live)) => live.has_pending(),
        _ => false,
    }
}

fn session_open(state: &Mutex<DaemonState>, terminal_id: &str) -> bool {
    let Some(mut guard) = lock_state(state) else {
        return false;
    };
    if let Some(Slot::Live(live)) = guard.terminals.get_mut(terminal_id) {
        live.refresh_exit();
        true
    } else {
        false
    }
}

/// Waits for the child outside the daemon mutex so other sessions can proceed.
fn reap_child(state: &Mutex<DaemonState>, terminal_id: &str) {
    let child = {
        let Some(mut guard) = lock_state(state) else {
            return;
        };
        let Some(Slot::Live(live)) = guard.terminals.get_mut(terminal_id) else {
            return;
        };
        if live.exited {
            return;
        }
        live.child.take()
    };
    let Some(mut child) = child else {
        return;
    };
    let waited = wait_child(&mut child);
    let Some(mut guard) = lock_state(state) else {
        eprintln!("keeplined: reaped {terminal_id} but could not publish the exit");
        return;
    };
    if let Some(Slot::Live(live)) = guard.terminals.get_mut(terminal_id) {
        match waited {
            Ok(status) => live.note_exit(status),
            Err(err) => {
                eprintln!("keeplined: failed to reap {terminal_id}: {err}");
                live.exited = true;
                live.note_wait_error(err);
            }
        }
    }
}

fn wait_child(child: &mut Child) -> io::Result<ExitStatus> {
    loop {
        match child.wait() {
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}

pub(crate) fn lock_state(state: &Mutex<DaemonState>) -> Option<MutexGuard<'_, DaemonState>> {
    match state.lock() {
        Ok(guard) => Some(guard),
        Err(_) => {
            eprintln!("keeplined: state lock poisoned");
            None
        }
    }
}
