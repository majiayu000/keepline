//! Read-only status of threads loaded in Codex's shared app-server daemon.
//! One connection per trigger: `initialize`, then `thread/read` per thread, then close.
//! It never subscribes (`thread/start`, `thread/resume`): a subscriber can be handed the
//! thread's approval requests, which belong to the user's own terminal.
use crate::content::Official;
use serde_json::{json, Value};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(5);

/// `$CODEX_HOME/app-server-control/app-server-control.sock`, `~/.codex` by default.
pub fn socket() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("KEEPLINE_CODEX_CONTROL") {
        return Some(path.into());
    }
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".codex")))?;
    Some(home.join("app-server-control/app-server-control.sock"))
}

/// Status per thread id; threads the daemon has not loaded (a `--no-daemon` session, an
/// exited one) and states that say nothing about a dialog are left out.
pub fn query(socket: &std::path::Path, threads: &[String]) -> io::Result<Vec<(String, Official)>> {
    let deadline = Instant::now() + TIMEOUT;
    let mut ws = Ws::connect(socket)?;
    ws.call(
        1,
        "initialize",
        json!({"clientInfo": {"name": "keepline", "version": env!("CARGO_PKG_VERSION")}}),
        deadline,
    )?;
    ws.send(&json!({"method": "initialized"}))?;
    let mut out = Vec::new();
    for (i, thread) in threads.iter().enumerate() {
        let reply = ws.call(
            10 + i as u64,
            "thread/read",
            json!({"threadId": thread, "includeTurns": false}),
            deadline,
        )?;
        if let Some(status) = status(&reply["result"]["thread"]["status"]) {
            out.push((thread.clone(), status));
        }
    }
    Ok(out)
}

pub fn status(v: &Value) -> Option<Official> {
    match v["type"].as_str()? {
        "active" => {
            let flags: Vec<&str> = v["activeFlags"]
                .as_array()
                .map(|f| f.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            Some(if flags.contains(&"waitingOnApproval") {
                Official::Waiting("等待审批".into())
            } else if flags.contains(&"waitingOnUserInput") {
                Official::Waiting("等待你的输入".into())
            } else {
                Official::Busy
            })
        }
        "idle" => Some(Official::Idle),
        _ => None,
    }
}

/// The minimal WebSocket client the control socket needs: one text message per JSON-RPC
/// message, client frames masked, server pings answered.
struct Ws {
    stream: UnixStream,
    buf: Vec<u8>,
}
impl Ws {
    fn connect(path: &std::path::Path) -> io::Result<Self> {
        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        let key = base64(uuid::Uuid::new_v4().as_bytes());
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )?;
        let mut buf = Vec::new();
        let end = loop {
            if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break end;
            }
            if buf.len() > 16 * 1024 {
                return Err(io::Error::other("control socket handshake too long"));
            }
            let mut chunk = [0; 4096];
            let n = stream.read(&mut chunk)?;
            if n == 0 {
                return Err(io::Error::other("control socket closed during handshake"));
            }
            buf.extend_from_slice(&chunk[..n]);
        };
        if !buf.starts_with(b"HTTP/1.1 101") {
            return Err(io::Error::other(
                "control socket refused the WebSocket upgrade",
            ));
        }
        let buf = buf.split_off(end + 4);
        Ok(Self { stream, buf })
    }
    fn frame(&mut self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        let random = uuid::Uuid::new_v4();
        let mask = &random.as_bytes()[..4];
        let mut f = vec![0x80 | opcode];
        match payload.len() {
            n if n < 126 => f.push(0x80 | n as u8),
            n if n < 65536 => {
                f.push(0x80 | 126);
                f.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                f.push(0x80 | 127);
                f.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        f.extend_from_slice(mask);
        f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream.write_all(&f)
    }
    fn send(&mut self, v: &Value) -> io::Result<()> {
        self.frame(0x1, v.to_string().as_bytes())
    }
    fn fill(&mut self, need: usize) -> io::Result<()> {
        while self.buf.len() < need {
            let mut chunk = [0; 16 * 1024];
            let n = self.stream.read(&mut chunk)?;
            if n == 0 {
                return Err(io::Error::other("control socket closed"));
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
        Ok(())
    }
    /// Next text message. Messages are bounded: thread metadata, not transcripts.
    fn message(&mut self) -> io::Result<Value> {
        loop {
            self.fill(2)?;
            let opcode = self.buf[0] & 0x0f;
            let masked = self.buf[1] & 0x80 != 0;
            let (len, at) = match self.buf[1] & 0x7f {
                126 => {
                    self.fill(4)?;
                    (u16::from_be_bytes([self.buf[2], self.buf[3]]) as usize, 4)
                }
                127 => {
                    self.fill(10)?;
                    let mut n = [0; 8];
                    n.copy_from_slice(&self.buf[2..10]);
                    (u64::from_be_bytes(n) as usize, 10)
                }
                n => (n as usize, 2),
            };
            if len > 4 * 1024 * 1024 {
                return Err(io::Error::other("control socket message over 4 MiB"));
            }
            let start = at + if masked { 4 } else { 0 };
            self.fill(start + len)?;
            let mut payload = self.buf[start..start + len].to_vec();
            if masked {
                let mask = self.buf[at..at + 4].to_vec();
                payload
                    .iter_mut()
                    .enumerate()
                    .for_each(|(i, b)| *b ^= mask[i % 4]);
            }
            self.buf.drain(..start + len);
            match opcode {
                0x1 => return serde_json::from_slice(&payload).map_err(io::Error::other),
                0x8 => return Err(io::Error::other("control socket closed the connection")),
                0x9 => self.frame(0xA, &payload)?,
                _ => (),
            }
        }
    }
    /// Sends a request and waits for its reply; notifications in between are skipped.
    fn call(
        &mut self,
        id: u64,
        method: &str,
        params: Value,
        deadline: Instant,
    ) -> io::Result<Value> {
        self.send(&json!({"id": id, "method": method, "params": params}))?;
        loop {
            if Instant::now() > deadline {
                return Err(io::Error::other("Codex app-server status query timed out"));
            }
            let m = self.message()?;
            if m["id"] == id {
                return Ok(m);
            }
        }
    }
}
impl Drop for Ws {
    fn drop(&mut self) {
        let _ = self.frame(0x8, &[]);
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// A fake control socket: answers `initialize` and `thread/read` from `statuses`,
    /// sends a notification and a ping first, and records every method it was asked.
    pub(crate) fn serve(
        path: &std::path::Path,
        statuses: Vec<(String, Value)>,
    ) -> std::thread::JoinHandle<Vec<String>> {
        let listener = UnixListener::bind(path).unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut b = [0; 1];
                s.read_exact(&mut b).unwrap();
                head.push(b[0]);
            }
            s.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n")
                .unwrap();
            let send = |s: &mut UnixStream, opcode: u8, d: &[u8]| {
                let mut f = vec![0x80 | opcode];
                if d.len() < 126 {
                    f.push(d.len() as u8);
                } else {
                    f.push(126);
                    f.extend_from_slice(&(d.len() as u16).to_be_bytes());
                }
                f.extend_from_slice(d);
                s.write_all(&f).unwrap();
            };
            send(&mut s, 0x1, br#"{"method":"account/updated","params":{}}"#);
            send(&mut s, 0x9, b"hi");
            let mut methods = Vec::new();
            loop {
                let mut h = [0; 2];
                if s.read_exact(&mut h).is_err() {
                    break;
                }
                let mut len = (h[1] & 0x7f) as usize;
                if len == 126 {
                    let mut n = [0; 2];
                    s.read_exact(&mut n).unwrap();
                    len = u16::from_be_bytes(n) as usize;
                }
                let mut mask = [0; 4];
                s.read_exact(&mut mask).unwrap();
                let mut d = vec![0; len];
                s.read_exact(&mut d).unwrap();
                d.iter_mut()
                    .enumerate()
                    .for_each(|(i, b)| *b ^= mask[i % 4]);
                match h[0] & 0x0f {
                    0x8 => break,
                    0x1 => (),
                    _ => continue,
                }
                let m: Value = serde_json::from_slice(&d).unwrap();
                let method = m["method"].as_str().unwrap_or("").to_owned();
                methods.push(method.clone());
                let reply = match method.as_str() {
                    "initialize" => json!({"id": m["id"], "result": {}}),
                    "thread/read" => {
                        let id = m["params"]["threadId"].as_str().unwrap();
                        match statuses.iter().find(|(t, _)| t == id) {
                            Some((_, st)) => {
                                json!({"id": m["id"], "result": {"thread": {"id": id, "status": st}}})
                            }
                            None => {
                                json!({"id": m["id"], "error": {"code": -32600, "message": "thread not loaded"}})
                            }
                        }
                    }
                    _ => continue,
                };
                send(&mut s, 0x1, reply.to_string().as_bytes());
            }
            methods
        })
    }

    /// `KL_THREADS=id,id cargo test -- --ignored real_daemon`: one read-only query against
    /// the installed Codex daemon.
    #[test]
    #[ignore = "needs a running Codex app-server daemon"]
    fn real_daemon_read_only() {
        let threads: Vec<String> = std::env::var("KL_THREADS")
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect();
        let at = Instant::now();
        let got = query(&socket().unwrap(), &threads).unwrap();
        eprintln!("{got:?} in {:?}", at.elapsed());
        assert!(!got.is_empty());
    }
    #[test]
    fn reads_status_without_subscribing_and_skips_unloaded_threads() {
        let dir = std::env::temp_dir().join(format!("kl-cx-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("control.sock");
        let server = serve(
            &path,
            vec![
                (
                    "a".into(),
                    json!({"type": "active", "activeFlags": ["waitingOnApproval"]}),
                ),
                ("b".into(), json!({"type": "active", "activeFlags": []})),
                ("c".into(), json!({"type": "idle"})),
                ("d".into(), json!({"type": "systemError"})),
            ],
        );
        let got = query(
            &path,
            &[
                "a".into(),
                "b".into(),
                "c".into(),
                "d".into(),
                "gone".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            got,
            vec![
                ("a".into(), Official::Waiting("等待审批".into())),
                ("b".into(), Official::Busy),
                ("c".into(), Official::Idle),
            ]
        );
        let methods = server.join().unwrap();
        assert_eq!(
            methods,
            [
                "initialize",
                "initialized",
                "thread/read",
                "thread/read",
                "thread/read",
                "thread/read",
                "thread/read"
            ]
        );
        assert_eq!(
            status(&json!({"type": "active", "activeFlags": ["waitingOnUserInput"]})),
            Some(Official::Waiting("等待你的输入".into()))
        );
        assert_eq!(base64(b"any carnal pleas"), "YW55IGNhcm5hbCBwbGVhcw==");
    }
}
