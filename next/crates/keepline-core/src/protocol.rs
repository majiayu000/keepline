use crate::model::{Edit, Workspace};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const MAJOR: u32 = 3;
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
pub const CONTROL: u8 = 1;
pub const SNAPSHOT: u8 = 2;
pub const READY: u8 = 3;
pub const OUTPUT: u8 = 4;
pub const RESIZED: u8 = 5;
pub const STATUS: u8 = 6;
pub const ACK: u8 = 7;
pub const WRITABLE: u8 = 8;

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Hello {
        major: u32,
    },
    Workspace,
    ClientLeft {
        at: u64,
    },
    RefreshAgents,
    RecognitionStatus,
    WatchRecognition,
    Hook {
        runtime: crate::model::Agent,
        pid: u32,
        cli_session_id: String,
        transcript: String,
        ended: bool,
        event: Option<String>,
    },
    Edit {
        revision: u64,
        edit: Edit,
    },
    Launch {
        id: String,
        space: String,
        argv: Vec<String>,
        cwd: String,
        cols: u16,
        rows: u16,
    },
    Attach {
        id: String,
        takeover: bool,
    },
    Input {
        bytes: Vec<u8>,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    TerminalCwd {
        id: String,
    },
    StopAgent {
        id: String,
    },
    Stop {
        id: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Reply {
    Hello {
        major: u32,
        instance: String,
    },
    Workspace(Workspace),
    Cwd(String),
    Recognition(RecognitionStatus),
    Attached {
        writable: bool,
        cols: u16,
        rows: u16,
    },
    Ok,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecognitionPhase {
    Starting,
    Healthy,
    Interrupted,
    Recovering,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognitionMetrics {
    pub ps_spawns: u64,
    pub lsof_spawns: u64,
    pub directories: u64,
    pub log_opens: u64,
    pub log_bytes: u64,
    pub index_writes: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecognitionStatus {
    pub phase: RecognitionPhase,
    pub detail: String,
    pub scopes: Vec<String>,
    pub metrics: RecognitionMetrics,
    pub terminal_hints: std::collections::HashMap<String, String>,
    pub workspace_revision: u64,
}
impl Default for RecognitionStatus {
    fn default() -> Self {
        Self {
            phase: RecognitionPhase::Starting,
            detail: "正在建立身份索引".into(),
            scopes: vec![],
            metrics: RecognitionMetrics::default(),
            terminal_hints: Default::default(),
            workspace_revision: 0,
        }
    }
}

pub fn write_frame(w: &mut impl Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    if payload.len() >= MAX_FRAME {
        return Err(io::Error::other("frame too large"));
    }
    w.write_all(&((payload.len() + 1) as u32).to_be_bytes())?;
    w.write_all(&[kind])?;
    w.write_all(payload)
}
pub fn read_frame(r: &mut impl Read) -> io::Result<(u8, Vec<u8>)> {
    let mut size = [0; 4];
    r.read_exact(&mut size)?;
    let size = u32::from_be_bytes(size) as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(io::Error::other("invalid frame size"));
    }
    let mut bytes = vec![0; size];
    r.read_exact(&mut bytes)?;
    Ok((bytes[0], bytes[1..].to_vec()))
}
pub fn send<T: Serialize>(w: &mut impl Write, kind: u8, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    write_frame(w, kind, &bytes)
}
pub fn receive<T: for<'de> Deserialize<'de>>(r: &mut impl Read) -> io::Result<T> {
    let (kind, bytes) = read_frame(r)?;
    if kind != CONTROL {
        return Err(io::Error::other("expected control frame"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub fn rpc(socket: &std::path::Path, request: &Request) -> io::Result<Reply> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
    match receive::<Reply>(&mut stream)? {
        Reply::Hello { major: MAJOR, .. } => (),
        reply => return Err(io::Error::other(format!("handshake: {reply:?}"))),
    }
    send(&mut stream, CONTROL, request)?;
    match receive::<Reply>(&mut stream)? {
        Reply::Error(message) => Err(io::Error::other(message)),
        reply => Ok(reply),
    }
}
