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

/// The few hook payload facts the status rules read. Prompt and reply text stay out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookFacts {
    /// From a Stop or SubagentStop payload that carried the task registry.
    pub background: Option<crate::content::Background>,
    pub notification: Option<String>,
    /// The event came from inside a subagent (`agent_id` present), not the parent turn.
    pub subagent: bool,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Hello {
        major: u32,
    },
    Workspace,
    ClientLeft {
        at: u64,
    },
    ClaimNotification {
        session: String,
        key: String,
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
        #[serde(default)]
        facts: HookFacts,
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
        /// One cell in device pixels, for size reports and the pty's pixel size; 0 if unknown.
        #[serde(default)]
        cell_width: u32,
        #[serde(default)]
        cell_height: u32,
    },
    /// Cmd+K: drop scrollback and the rows above the cursor (no-op in the alternate screen).
    ClearHistory,
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
        /// Absent from hosts built before it existed, which are stale by definition.
        #[serde(default)]
        build: Build,
    },
    Workspace(Workspace),
    Cwd(String),
    NotificationClaimed(bool),
    Recognition(RecognitionStatus),
    Attached {
        writable: bool,
        cols: u16,
        rows: u16,
    },
    Ok,
    Error(String),
}

/// The executable a host runs, as it was when the host started. A client compares it
/// with the file on disk now: a rebuilt binary does not change a running host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Build {
    pub binary: String,
    /// Modification time of `binary` in seconds since the epoch; 0 if unknown.
    pub modified: u64,
    /// Content digest of `binary` when the host started; empty from hosts before it.
    #[serde(default)]
    pub digest: String,
}
impl Build {
    pub fn current() -> Self {
        let binary = std::env::current_exe().unwrap_or_default();
        Self {
            modified: Self::modified(&binary),
            digest: Self::digest(&binary).unwrap_or_default(),
            binary: binary.to_string_lossy().into_owned(),
        }
    }
    /// Identifies a file by its bytes (FNV-1a and length), so a re-signed or copied
    /// binary with the same code is the same build whatever its timestamps say.
    pub fn digest(path: &std::path::Path) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
        });
        Some(format!("{hash:016x}-{}", bytes.len()))
    }
    pub fn modified(path: &std::path::Path) -> u64 {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs())
    }
    /// Why this host runs older code than its own binary, or than `sibling` (the host
    /// this client would start), when it does.
    pub fn stale(&self, sibling: &std::path::Path) -> Option<String> {
        if self.binary.is_empty() || self.modified == 0 {
            return Some("后台进程是旧版本（不报告构建信息）".into());
        }
        let own = std::path::Path::new(&self.binary);
        // The code the host runs: its digest from startup, or for a host before digests,
        // its file as it is now when that file has not been touched since.
        let running = if !self.digest.is_empty() {
            Some(self.digest.clone())
        } else if Self::modified(own) <= self.modified {
            Self::digest(own)
        } else {
            None
        };
        let Some(running) = running else {
            return Some(format!("后台进程启动后 {} 已重新编译", self.binary));
        };
        if Self::digest(own).is_some_and(|d| d != running) {
            return Some(format!("后台进程启动后 {} 已重新编译", self.binary));
        }
        if Self::digest(sibling).is_some_and(|d| d != running) {
            return Some(format!(
                "后台进程（{}）和本 App 自带的 {} 不是同一版本",
                self.binary,
                sibling.display()
            ));
        }
        None
    }
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
    /// Terminal -> the title its program set, spinner dropped. Live state, not stored.
    #[serde(default)]
    pub terminal_titles: std::collections::HashMap<String, String>,
    pub workspace_revision: u64,
    /// CLI -> why its official status query failed last; absent once one succeeds. The
    /// status rules then fall back to logs, hooks and the screen.
    #[serde(default)]
    pub official: std::collections::BTreeMap<String, String>,
}
impl Default for RecognitionStatus {
    fn default() -> Self {
        Self {
            phase: RecognitionPhase::Starting,
            detail: "正在建立身份索引".into(),
            scopes: vec![],
            metrics: RecognitionMetrics::default(),
            terminal_hints: Default::default(),
            terminal_titles: Default::default(),
            workspace_revision: 0,
            official: Default::default(),
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
/// Serialize one queued user input as ordered bounded packets. UTF-8 and bracketed
/// paste delimiters may span packets: the PTY remains a byte stream. Never retry.
pub fn send_input(w: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    for chunk in bytes.chunks(16 * 1024) {
        send(
            w,
            CONTROL,
            &Request::Input {
                bytes: chunk.to_vec(),
            },
        )?;
    }
    Ok(())
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
    // Preserve the error kind and failing stage. Never replay an RPC: a timed-out
    // mutation may already have committed even if its reply was not received.
    let stage = |name, e: io::Error| io::Error::new(e.kind(), format!("RPC {name}: {e}"));
    let mut stream =
        std::os::unix::net::UnixStream::connect(socket).map_err(|e| stage("connect", e))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })
        .map_err(|e| stage("send hello", e))?;
    match receive::<Reply>(&mut stream).map_err(|e| stage("receive hello", e))? {
        Reply::Hello { major: MAJOR, .. } => (),
        reply => return Err(io::Error::other(format!("handshake: {reply:?}"))),
    }
    send(&mut stream, CONTROL, request).map_err(|e| stage("send request", e))?;
    match receive::<Reply>(&mut stream).map_err(|e| stage("receive reply", e))? {
        Reply::Error(message) => Err(io::Error::other(message)),
        reply => Ok(reply),
    }
}

#[cfg(test)]
mod tests {
    use super::Build;

    #[test]
    fn a_host_is_stale_only_when_the_code_differs() {
        let dir = std::env::temp_dir().join(format!("kl-build-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("keeplined");
        std::fs::write(&binary, "v1").unwrap();
        let path = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let started = Build {
            binary: path(&binary),
            modified: Build::modified(&binary),
            digest: Build::digest(&binary).unwrap(),
        };
        let missing = dir.join("absent");
        assert_eq!(started.stale(&missing), None);
        assert!(
            Build::default().stale(&missing).is_some(),
            "pre-build hosts"
        );
        // A copy with other timestamps (re-signed, touched) is the same build.
        let copy = dir.join("bundled");
        std::fs::write(&copy, "v1").unwrap();
        assert_eq!(started.stale(&copy), None);
        let before_digests = Build {
            digest: String::new(),
            modified: started.modified + 60,
            ..started.clone()
        };
        assert_eq!(before_digests.stale(&copy), None);
        // Other bytes next to the app, or in place of the running file, are a new build.
        std::fs::write(&copy, "v2").unwrap();
        assert!(started.stale(&copy).unwrap().contains("不是同一版本"));
        assert!(
            before_digests
                .stale(&copy)
                .unwrap()
                .contains("不是同一版本")
        );
        std::fs::write(&binary, "v2").unwrap();
        assert!(started.stale(&missing).unwrap().contains("重新编译"));
        let touched = Build {
            digest: String::new(),
            modified: 1,
            ..started.clone()
        };
        assert!(touched.stale(&missing).unwrap().contains("重新编译"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
