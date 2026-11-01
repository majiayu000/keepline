//! `keeplined remote --stdio`: this machine's terminals for a client elsewhere, a phone
//! app or another machine's workbench, one JSON object per line each way. Over ssh today
//! (`ssh host keeplined remote --stdio`); a later transport carries the same lines.
//! Requests run one after another in the order sent, so text then Enter arrive in that
//! order; `events` and `watch` are streams that run beside them until closed.
//! The message shapes are in `specs/extensions/remote-protocol.md`.
use crate::control::{self, Args, Phase};
use base64::Engine;
use keepline_core::{model::*, protocol::*, terminal::Vt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

pub const VERSION: u32 = 1;
const MAX_LINE: u64 = 1 << 20;
const OPS: &[&str] = &[
    "hello", "list", "read", "send", "keys", "answer", "events", "watch", "close",
];
/// A client that stops reading holds up its own streams, never the host: the terminal
/// marks a stalled viewer lagged and sends it the screen again once it reads.
const OUT_QUEUE: usize = 256;

type Out = mpsc::SyncSender<Value>;
/// Open streams by the id of the request that opened them, to close one on request.
type Streams = Arc<Mutex<Registry>>;
#[derive(Default)]
struct Registry {
    /// `None`: closed by the client before it had opened.
    open: HashMap<u64, Option<UnixStream>>,
    /// The client has gone; a stream still connecting must not open.
    ended: bool,
}

pub fn run(args: impl Iterator<Item = String>) -> io::Result<()> {
    let a = Args::parse(args, &[])?;
    if !a.has("stdio") {
        return Err(io::Error::other(
            "usage: keeplined remote --stdio [--runtime <目录>]",
        ));
    }
    serve(io::stdin().lock(), io::stdout(), &a.socket)
}

/// Serves `input` until it ends, replying on `output`.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write + Send + 'static,
    socket: &Path,
) -> io::Result<()> {
    let (out, lines) = mpsc::sync_channel::<Value>(OUT_QUEUE);
    let writer = thread::spawn(move || {
        for line in lines {
            if writeln!(output, "{line}")
                .and_then(|_| output.flush())
                .is_err()
            {
                break;
            }
        }
    });
    let (work, jobs) = mpsc::channel::<(u64, Value)>();
    let streams: Streams = Default::default();
    let worker = {
        let (out, socket) = (out.clone(), socket.to_owned());
        thread::spawn(move || {
            for (id, request) in jobs {
                let reply = match unary(&socket, &request) {
                    Ok(ok) => json!({ "id": id, "ok": ok }),
                    Err(e) => json!({ "id": id, "error": e.to_string() }),
                };
                if out.send(reply).is_err() {
                    break;
                }
            }
        })
    };
    let mut greeted = false;
    let mut line = Vec::new();
    loop {
        line.clear();
        if input
            .by_ref()
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut line)?
            == 0
        {
            break;
        }
        if line.len() as u64 > MAX_LINE {
            let _ = out.send(json!({ "error": "一行超过 1 MiB，连接结束" }));
            break;
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let request: Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = out.send(json!({ "error": format!("不是 JSON：{e}") }));
                continue;
            }
        };
        let Some(id) = request["id"].as_u64() else {
            let _ = out.send(json!({ "error": "缺少数字 id" }));
            continue;
        };
        let op = request["op"].as_str().unwrap_or_default();
        let fail = |message: String| {
            let _ = out.send(json!({ "id": id, "error": message }));
        };
        match op {
            "hello" => {
                greeted = true;
                let _ = out.send(json!({ "id": id, "ok": hello(socket) }));
            }
            _ if !greeted => fail("先发 hello".into()),
            "events" | "watch" => {
                let (out, socket, streams) = (out.clone(), socket.to_owned(), streams.clone());
                thread::spawn(move || stream(id, &request, &socket, &out, &streams));
            }
            "close" => match request["stream"].as_u64() {
                Some(s) => {
                    if let Ok(mut r) = streams.lock() {
                        match r.open.remove(&s) {
                            Some(Some(sock)) => {
                                let _ = sock.shutdown(std::net::Shutdown::Both);
                            }
                            // Not open yet: it ends as soon as it would.
                            _ => {
                                r.open.insert(s, None);
                            }
                        }
                    }
                    let _ = out.send(json!({ "id": id, "ok": {} }));
                }
                None => fail("close 需要 stream".into()),
            },
            _ if OPS.contains(&op) => {
                let _ = work.send((id, request));
            }
            _ => fail(format!("不认识的 op {op:?}")),
        }
    }
    // The client is gone: end every stream, finish queued requests, flush the replies.
    if let Ok(mut r) = streams.lock() {
        r.ended = true;
        for sock in r.open.drain().filter_map(|(_, s)| s) {
            let _ = sock.shutdown(std::net::Shutdown::Both);
        }
    }
    drop(work);
    let _ = worker.join();
    drop(out);
    let _ = writer.join();
    Ok(())
}

/// This machine's id stays the same across restarts, so a client that knows several
/// machines can tell their terminals apart.
fn machine(socket: &Path) -> Value {
    let file = socket.with_file_name("machine-id");
    let id = std::fs::read_to_string(&file)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let id = uuid::Uuid::new_v4().to_string();
            let _ = std::fs::write(&file, &id);
            id
        });
    let mut name = [0u8; 256];
    // SAFETY: gethostname writes at most `len` bytes into the buffer it is given.
    let ok = unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } == 0;
    let name = if ok {
        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        String::from_utf8_lossy(&name[..end]).into_owned()
    } else {
        String::new()
    };
    json!({ "id": id, "name": name })
}
fn hello(socket: &Path) -> Value {
    json!({ "version": VERSION, "machine": machine(socket), "ops": OPS })
}

fn text<'a>(request: &'a Value, key: &str) -> io::Result<&'a str> {
    request[key]
        .as_str()
        .ok_or_else(|| io::Error::other(format!("缺少 {key}")))
}
fn ok_reply(reply: Reply) -> io::Result<Value> {
    match reply {
        Reply::Ok => Ok(json!({})),
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    }
}
fn unary(socket: &Path, request: &Value) -> io::Result<Value> {
    let terminal = || text(request, "terminal").map(str::to_owned);
    match request["op"].as_str().unwrap_or_default() {
        "list" => match rpc(socket, &Request::Workspace)? {
            Reply::Workspace(w) => Ok(list(&w, socket)),
            other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
        },
        "read" => {
            let lines = request["lines"].as_u64().unwrap_or(50).clamp(1, 2000) as usize;
            match rpc(
                socket,
                &Request::ReadScreen {
                    id: terminal()?,
                    rows: lines + 200,
                },
            )? {
                Reply::Text(t) => {
                    // The empty rows below the cursor say nothing.
                    let rows: Vec<_> = t.trim_end().lines().collect();
                    Ok(json!({ "text": rows[rows.len().saturating_sub(lines)..].join("\n") }))
                }
                other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
            }
        }
        "send" => ok_reply(rpc(
            socket,
            &Request::SendText {
                id: terminal()?,
                text: text(request, "text")?.to_owned(),
                submit: request["enter"].as_bool().unwrap_or(false),
            },
        )?),
        "keys" => {
            let keys = request["keys"]
                .as_array()
                .ok_or_else(|| io::Error::other("keys 需要字符串数组"))?
                .iter()
                .map(|k| k.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| io::Error::other("keys 需要字符串数组"))?;
            ok_reply(rpc(
                socket,
                &Request::SendKeys {
                    id: terminal()?,
                    keys,
                },
            )?)
        }
        "answer" => ok_reply(rpc(
            socket,
            &Request::AnswerApproval {
                id: terminal()?,
                question: text(request, "question")?.to_owned(),
                allow: request["allow"]
                    .as_bool()
                    .ok_or_else(|| io::Error::other("answer 需要 allow: true 或 false"))?,
            },
        )?),
        op => Err(io::Error::other(format!("不认识的 op {op:?}"))),
    }
}

/// Spaces and running terminals as cards: what each Agent is doing and what it asks.
fn list(w: &Workspace, socket: &Path) -> Value {
    let terminals: Vec<_> = w
        .terminals
        .iter()
        .filter(|t| t.state == ProcessState::Running)
        .map(|t| {
            let session = w.current_agent(&t.id);
            let content = session.and_then(|s| w.content.get(&s.id));
            let phase = match control::phase(w, &t.id) {
                Phase::None => "shell",
                p => p.name(),
            };
            json!({
                "id": t.id,
                "space": t.space,
                "tab": control::tab_title(w, &t.id),
                "cwd": t.cwd,
                "phase": phase,
                "agent": session.map(|s| control::agent_label(s.runtime)),
                "session": session.map(|s| &s.id),
                "title": content.map(|c| if c.name.is_empty() { &c.title } else { &c.name }),
                "approval": content.map(|c| &c.approval).filter(|a| !a.is_empty()),
                "waiting": content.map(|c| &c.waiting).filter(|a| !a.is_empty()),
            })
        })
        .collect();
    let spaces: Vec<_> = w
        .spaces
        .iter()
        .map(|s| json!({ "id": s.id, "name": s.name, "cwd": s.cwd }))
        .collect();
    json!({ "machine": machine(socket), "spaces": spaces, "terminals": terminals })
}

fn connect(socket: &Path, request: &Request) -> io::Result<UnixStream> {
    let mut stream = UnixStream::connect(socket)?;
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
    match receive::<Reply>(&mut stream)? {
        Reply::Hello { major: MAJOR, .. } => (),
        r => return Err(io::Error::other(format!("handshake: {r:?}"))),
    }
    send(&mut stream, CONTROL, request)?;
    Ok(stream)
}
/// One open stream: its request id, where it writes, and the registry `close` uses.
struct Stream<'a> {
    id: u64,
    out: &'a Out,
    streams: &'a Streams,
    opened: bool,
}
impl Stream<'_> {
    /// Registers the stream and tells the client it is open, before its first message.
    fn open(&mut self, socket: &UnixStream, ok: Value) -> io::Result<()> {
        let mut r = self
            .streams
            .lock()
            .map_err(|_| io::Error::other("streams lock poisoned"))?;
        if r.ended || r.open.remove(&self.id).is_some() {
            return Err(io::Error::other("closed"));
        }
        r.open.insert(self.id, Some(socket.try_clone()?));
        drop(r);
        self.opened = true;
        self.emit(json!({ "id": self.id, "ok": ok }))
    }
    fn emit(&self, message: Value) -> io::Result<()> {
        self.out
            .send(message)
            .map_err(|_| io::Error::other("client gone"))
    }
}
/// Runs one stream to its end and says why it ended; the client closing it is "closed".
fn stream(id: u64, request: &Value, socket: &Path, out: &Out, streams: &Streams) {
    let mut s = Stream {
        id,
        out,
        streams,
        opened: false,
    };
    let result = match request["op"].as_str() {
        Some("events") => connect(socket, &Request::Subscribe).and_then(|c| events(&mut s, c)),
        _ => text(request, "terminal")
            .map(str::to_owned)
            .and_then(|t| watch(&mut s, &t, socket)),
    };
    // Still registered means the client did not close it.
    let open = streams
        .lock()
        .map(|mut r| r.open.remove(&id).is_some_and(|s| s.is_some()))
        .unwrap_or(false);
    let _ = match result {
        Err(e) if !s.opened => out.send(json!({ "id": id, "error": e.to_string() })),
        Err(e) if open => out.send(json!({ "stream": id, "type": "end", "reason": e.to_string() })),
        _ => out.send(json!({ "stream": id, "type": "end", "reason": "closed" })),
    };
}
fn events(s: &mut Stream, mut socket: UnixStream) -> io::Result<()> {
    let id = s.id;
    s.open(&socket, json!({ "stream": id }))?;
    loop {
        match receive::<Reply>(&mut socket)? {
            Reply::Events(notices) => {
                for n in notices {
                    s.emit(json!({ "stream": id, "type": "event", "event": n }))?;
                }
            }
            Reply::Error(e) => return Err(io::Error::other(e)),
            other => return Err(io::Error::other(format!("unexpected reply: {other:?}"))),
        }
    }
}

/// The terminal's screen as escape sequences, then its output as it comes, for an
/// emulator on the client. It only looks: no size change, never the writer.
fn watch(st: &mut Stream, terminal: &str, socket: &Path) -> io::Result<()> {
    let (id, b64) = (st.id, base64::engine::general_purpose::STANDARD);
    let mut s = connect(
        socket,
        &Request::Attach {
            id: terminal.to_owned(),
            takeover: false,
            watch: true,
        },
    )?;
    let (mut cols, mut rows) = match receive::<Reply>(&mut s)? {
        Reply::Attached { cols, rows, .. } => (cols, rows),
        Reply::Error(e) => return Err(io::Error::other(e)),
        other => return Err(io::Error::other(format!("unexpected reply: {other:?}"))),
    };
    st.open(&s, json!({ "stream": id, "cols": cols, "rows": rows }))?;
    let mut snapshot = Vec::new();
    let mut exited = false;
    loop {
        let (kind, bytes) = read_frame(&mut s)?;
        match kind {
            SNAPSHOT => snapshot.extend_from_slice(&bytes),
            // The first screen, or the screen again after this client fell behind.
            READY => {
                let vt = Vt::restore(&std::mem::take(&mut snapshot))?;
                st.emit(json!({
                        "stream": id, "type": "screen", "cols": cols, "rows": rows,
                    "vt": b64.encode(vt.replay()?),
                }))?;
            }
            OUTPUT => {
                st.emit(json!({ "stream": id, "type": "output", "vt": b64.encode(&bytes) }))?
            }
            RESIZED => {
                (cols, rows) = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                st.emit(json!({ "stream": id, "type": "resized", "cols": cols, "rows": rows }))?;
            }
            STATUS => {
                let t: Terminal = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                if t.state != ProcessState::Running && !exited {
                    exited = true;
                    st.emit(json!({ "stream": id, "type": "exited", "code": t.exit_code }))?;
                }
            }
            _ => (),
        }
    }
}
