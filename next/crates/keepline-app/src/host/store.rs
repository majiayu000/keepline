//! The client's link to keeplined: the socket, the in-flight user action, background
//! syncs and the recognition status the host pushes. Views never call `rpc` themselves.
use crate::connection::RecognitionConnection;
use futures::channel::mpsc::UnboundedSender;
use keepline_core::{model::*, protocol::*};
use std::{
    collections::VecDeque,
    future::Future,
    io,
    path::{Path, PathBuf},
};

pub struct Store {
    socket: PathBuf,
    /// A user action is in flight; further actions wait for it.
    busy: bool,
    /// The in-flight user action has taken long enough to show 「正在更新…」.
    slow: bool,
    busy_seq: u64,
    /// A background refresh is in flight; it never blocks input or shows a label.
    syncing: bool,
    sync_again: bool,
    /// Tab and layout edits clicked while another user action was in flight.
    queued: VecDeque<Request>,
    recognition: RecognitionStatus,
    _recognition_connection: RecognitionConnection,
}

impl Store {
    pub fn new(socket: PathBuf, status: UnboundedSender<RecognitionStatus>) -> Self {
        let recognition_connection = RecognitionConnection::new(&socket, status);
        Self {
            socket,
            busy: false,
            slow: false,
            busy_seq: 0,
            syncing: false,
            sync_again: false,
            queued: VecDeque::new(),
            recognition: RecognitionStatus::default(),
            _recognition_connection: recognition_connection,
        }
    }
    pub fn socket(&self) -> &Path {
        &self.socket
    }
    pub fn busy(&self) -> bool {
        self.busy
    }
    pub fn slow(&self) -> bool {
        self.slow
    }
    pub fn recognition(&self) -> &RecognitionStatus {
        &self.recognition
    }
    pub fn set_recognition(&mut self, status: RecognitionStatus) {
        self.recognition = status;
    }
    /// The host has published a newer workspace than the one at `revision`.
    pub fn behind(&self, revision: u64) -> bool {
        self.recognition.workspace_revision > revision
    }
    /// Marks a user action in flight and returns its sequence number.
    pub fn begin_busy(&mut self) -> u64 {
        self.busy = true;
        self.busy_seq += 1;
        self.busy_seq
    }
    /// Shows 「正在更新…」 if action `seq` is still the one in flight.
    pub fn mark_slow(&mut self, seq: u64) -> bool {
        if self.busy && self.busy_seq == seq {
            self.slow = true;
            return true;
        }
        false
    }
    pub fn end_busy(&mut self) {
        self.busy = false;
        self.slow = false;
    }
    /// Revision-free edits are idempotent, so a click during another action
    /// waits its turn; anything else is still dropped to avoid double submits.
    pub fn hold(&mut self, request: Request) {
        if queueable(&request) {
            self.queued.push_back(request);
        }
    }
    pub fn next_queued(&mut self) -> Option<Request> {
        self.queued.pop_front()
    }
    /// Starts a background sync unless one is in flight; that one fetches again after.
    pub fn begin_sync(&mut self) -> bool {
        if self.syncing {
            self.sync_again = true;
            return false;
        }
        self.syncing = true;
        self.sync_again = false;
        true
    }
    /// Ends the sync in flight; true when another was asked for meanwhile.
    pub fn end_sync(&mut self) -> bool {
        self.syncing = false;
        std::mem::take(&mut self.sync_again)
    }
    pub fn send(&self, request: Request) -> impl Future<Output = io::Result<Reply>> + use<> {
        let socket = self.socket.clone();
        async move { rpc(&socket, &request) }
    }
    /// Launches a terminal, then saves the tab that shows it. A split from a running
    /// shell first asks the host for that shell's current directory.
    pub fn launch(
        &self,
        cwd_source: Option<String>,
        mut request: Request,
        tab: Tab,
    ) -> impl Future<Output = io::Result<Reply>> + use<> {
        let socket = self.socket.clone();
        async move {
            // A shell that ended or moved somewhere unreadable keeps the recorded directory.
            if let Some(id) = cwd_source
                && let Ok(Reply::Cwd(current)) = rpc(&socket, &Request::TerminalCwd { id })
                && Path::new(&current).is_dir()
                && let Request::Launch { cwd, .. } = &mut request
            {
                *cwd = current;
            }
            let Reply::Workspace(w) = rpc(&socket, &request)? else {
                return Err(std::io::Error::other("launch response missing"));
            };
            rpc(
                &socket,
                &Request::Edit {
                    revision: w.revision,
                    edit: Edit::PutTab { tab },
                },
            )
        }
    }
}

pub fn record_left(socket: &Path, at: u64) {
    if let Err(e) = rpc(socket, &Request::ClientLeft { at }) {
        eprintln!("记录离开时间失败: {e}");
    }
}
pub fn queueable(request: &Request) -> bool {
    matches!(request, Request::Edit { edit, .. } if !edit.checks_revision())
}
