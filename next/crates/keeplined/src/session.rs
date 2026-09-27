use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};

use crate::grid::{self, Screen};
use crate::intent::IntentRecord;
use crate::protocol::DELTA_LIMIT;

pub(crate) struct Delta {
    pub revision: u64,
    pub checksum: u64,
}

pub(crate) struct Lease {
    pub generation: u64,
    pub token: String,
    pub owner: u64,
}

pub(crate) struct LiveSession {
    pub intent: IntentRecord,
    pub child: Child,
    pub master: File,
    pub screen: Screen,
    pub cols: u16,
    pub rows: u16,
    pub revision: u64,
    pub deltas: VecDeque<Delta>,
    pub lease_generation: u64,
    pub lease: Option<Lease>,
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
        if let Err(err) = self.child.kill() {
            eprintln!(
                "keeplined: failed to stop {}: {err}",
                self.intent.terminal_id
            );
        }
        if let Err(err) = self.child.wait() {
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
            child,
            master,
            screen: Screen::new(cols, rows),
            cols,
            rows,
            revision: 0,
            deltas: VecDeque::new(),
            lease_generation: 0,
            lease: None,
            exited: false,
            exit_code: None,
            wait_error: None,
        }
    }

    pub(crate) fn refresh_exit(&mut self) {
        if self.exited {
            return;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.exited = true;
                self.exit_code = status.code();
            }
            Ok(None) => {}
            Err(err) => {
                if self.wait_error.is_none() {
                    eprintln!(
                        "keeplined: could not check child {}: {err}",
                        self.intent.terminal_id
                    );
                    self.wait_error = Some(err.to_string());
                }
            }
        }
    }

    pub(crate) fn oldest_retained(&self) -> Option<u64> {
        self.deltas.front().map(|delta| delta.revision)
    }

    pub(crate) fn push_revision(&mut self) {
        let checksum = self.screen.view().checksum;
        self.revision = self.revision.saturating_add(1);
        self.deltas.push_back(Delta {
            revision: self.revision,
            checksum,
        });
        while self.deltas.len() > DELTA_LIMIT {
            self.deltas.pop_front();
        }
    }

    pub(crate) fn ingest(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.screen.advance(bytes);
        let newlines = bytes.iter().filter(|byte| **byte == b'\n').count();
        let steps = if newlines == 0 {
            1
        } else if bytes.ends_with(b"\n") {
            newlines
        } else {
            newlines + 1
        };
        for _ in 0..steps {
            self.push_revision();
        }
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
            "pid": self.child.id(),
            "alive": !self.exited,
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

pub(crate) fn spawn_reader(reader: File, state: Arc<Mutex<DaemonState>>, terminal_id: String) {
    std::thread::spawn(move || reader_loop(reader, state, terminal_id));
}

fn reader_loop(mut reader: File, state: Arc<Mutex<DaemonState>>, terminal_id: String) {
    let mut buf = [0u8; 8192];
    loop {
        match grid::wait_readable(reader.as_raw_fd(), 200) {
            Ok(false) => {
                if !reap(&state, &terminal_id) {
                    return;
                }
            }
            Ok(true) => loop {
                match reader.read(&mut buf) {
                    Ok(0) => return,
                    Ok(n) => ingest(&state, &terminal_id, &buf[..n]),
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(err) => {
                        eprintln!("keeplined: pty read failed for {terminal_id}: {err}");
                        return;
                    }
                }
            },
            Err(err) => {
                eprintln!("keeplined: pty poll failed for {terminal_id}: {err}");
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

fn reap(state: &Mutex<DaemonState>, terminal_id: &str) -> bool {
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

pub(crate) fn lock_state(state: &Mutex<DaemonState>) -> Option<MutexGuard<'_, DaemonState>> {
    match state.lock() {
        Ok(guard) => Some(guard),
        Err(_) => {
            eprintln!("keeplined: state lock poisoned");
            None
        }
    }
}
