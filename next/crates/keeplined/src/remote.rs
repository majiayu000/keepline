//! `keepline remote --stdio`: this machine's terminals for a client elsewhere, a phone
//! app or another machine's workbench, one JSON object per line each way. Over ssh today
//! (`ssh host keepline remote --stdio`); a later transport carries the same lines.
//! Requests run one after another in the order sent, so text then Enter arrive in that
//! order; `events` and `watch` are streams that run beside them until closed.
//! The message shapes are in `specs/extensions/remote-protocol.md`.
use crate::control::{self, Args, Phase};
use base64::Engine;
use keepline_core::{model::*, protocol::*, terminal::Vt};
use keepline_remote::protocol as rp;
use keeplined::devices::{self, Role};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

const MAX_LINE: u64 = rp::MAX_LINE as u64;
use rp::OPS;
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

/// Who is on the other end and what it may do. Over ssh it is this user, already trusted.
#[derive(Clone)]
pub(crate) struct Access {
    pub(crate) role: Role,
    /// `(id, name)` of a paired device.
    pub(crate) device: Option<(String, String)>,
    /// Where acting ops are logged.
    pub(crate) log: Option<PathBuf>,
    /// Told what the client watches and does, for the app to show who is on.
    pub(crate) observer: Option<Observer>,
}
pub(crate) type Observer = Arc<dyn Fn(Activity) + Send + Sync>;
pub(crate) enum Activity {
    Watching(String),
    Unwatched(String),
    Acted {
        op: String,
        terminal: Option<String>,
    },
}
impl Access {
    fn trusted() -> Self {
        Self {
            role: Role::Full,
            device: None,
            log: None,
            observer: None,
        }
    }
    fn tell(&self, activity: Activity) {
        if let Some(o) = &self.observer {
            o(activity);
        }
    }
    fn record(&self, op: &str, request: &Value, result: &io::Result<Value>) {
        let (Some(log), Some((id, name))) = (&self.log, &self.device) else {
            return;
        };
        devices::log(
            log,
            json!({
                "event": "op",
                "device": id,
                "name": name,
                "op": op,
                "terminal": request["terminal"],
                "error": result.as_ref().err().map(|e| e.to_string()),
            }),
        );
    }
}

pub fn run(args: impl Iterator<Item = String>) -> io::Result<()> {
    let a = Args::parse(
        args,
        &[
            "relay",
            "relay-key",
            "name",
            "role",
            "revoke",
            "device",
            "kick",
        ],
    )?;
    if a.value("relay").is_some()
        || a.has("devices")
        || a.value("revoke").is_some()
        || a.value("device").is_some()
        || a.value("kick").is_some()
        || a.has("enable")
        || a.has("disable")
        || a.has("status")
    {
        return crate::relay::run(&a);
    }
    if !a.has("stdio") {
        return Err(io::Error::other(
            "usage: keepline remote --stdio | --relay <中继地址> [--relay-key <密钥>] | --pair --relay <中继地址> [--name <名字>] [--role view|answer|full] [--new-code] | --devices | --device <设备> --role <权限> | --revoke <设备>",
        ));
    }
    serve(
        io::stdin().lock(),
        io::stdout(),
        &a.socket,
        Access::trusted(),
    )
}

/// Serves `input` until it ends, replying on `output`.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write + Send + 'static,
    socket: &Path,
    access: Access,
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
        let (out, socket, access) = (out.clone(), socket.to_owned(), access.clone());
        thread::spawn(move || {
            for (id, request) in jobs {
                let result = unary(&socket, &request);
                let op = request["op"].as_str().unwrap_or_default();
                if devices::acts(op) {
                    access.record(op, &request, &result);
                    if result.is_ok() {
                        access.tell(Activity::Acted {
                            op: op.to_owned(),
                            terminal: request["terminal"].as_str().map(str::to_owned),
                        });
                    }
                }
                let reply = match result {
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
                let _ = out.send(json!({ "id": id, "ok": hello(socket, access.role) }));
            }
            _ if !greeted => fail("先发 hello".into()),
            _ if !access.role.allows(op) => fail(format!(
                "这台设备的权限是「{}」，不能 {op}",
                access.role.label()
            )),
            "events" | "watch" => {
                let (out, socket, streams) = (out.clone(), socket.to_owned(), streams.clone());
                let access = access.clone();
                thread::spawn(move || stream(id, &request, &socket, &out, &streams, &access));
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

pub(crate) use keeplined::devices::machine;
fn hello(socket: &Path, role: Role) -> Value {
    json!(rp::Hello {
        version: rp::VERSION,
        machine: machine(socket),
        ops: OPS
            .iter()
            .filter(|op| role.allows(op))
            .map(|op| op.to_string())
            .collect(),
        role,
    })
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
        "chat" => chat(socket, request),
        op => Err(io::Error::other(format!("不认识的 op {op:?}"))),
    }
}

/// A conversation as chat, named by its terminal or its session id.
fn chat(socket: &Path, request: &Value) -> io::Result<Value> {
    let session = match (request["session"].as_str(), request["terminal"].as_str()) {
        (Some(s), _) => s.to_owned(),
        (None, Some(t)) => match rpc(socket, &Request::Workspace)? {
            Reply::Workspace(w) => w
                .current_agent(t)
                .map(|s| s.id.clone())
                .ok_or_else(|| io::Error::other("这个终端里没有 Agent 对话"))?,
            other => return Err(io::Error::other(format!("unexpected reply: {other:?}"))),
        },
        (None, None) => return Err(io::Error::other("chat 需要 terminal 或 session")),
    };
    let reply = rpc(
        socket,
        &Request::Chat {
            session: session.clone(),
            after: request["after"].as_i64(),
            before: request["before"].as_i64(),
            limit: request["limit"].as_u64().unwrap_or(50).clamp(1, 500) as usize,
        },
    )?;
    match reply {
        Reply::Chat { messages, more } => Ok(json!(rp::Chat {
            session,
            messages: messages
                .into_iter()
                .map(|m| rp::Message {
                    seq: m.seq,
                    at: m.at,
                    role: if m.user {
                        rp::Speaker::User
                    } else {
                        rp::Speaker::Agent
                    },
                    text: m.text,
                })
                .collect(),
            more,
        })),
        Reply::Error(e) => Err(io::Error::other(e)),
        other => Err(io::Error::other(format!("unexpected reply: {other:?}"))),
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
                Phase::Working => rp::Phase::Working,
                Phase::Waiting => rp::Phase::Waiting,
                Phase::Idle => rp::Phase::Idle,
                Phase::None => rp::Phase::Shell,
            };
            let said = |s: &String| Some(s.clone()).filter(|s| !s.is_empty());
            rp::Card {
                id: t.id.clone(),
                space: t.space.clone(),
                tab: control::tab_title(w, &t.id),
                cwd: t.cwd.clone(),
                phase,
                agent: session.map(|s| control::agent_label(s.runtime).to_owned()),
                session: session.map(|s| s.id.clone()),
                title: content.map(|c| if c.name.is_empty() { &c.title } else { &c.name }.clone()),
                approval: content.and_then(|c| said(&c.approval)),
                waiting: content.and_then(|c| said(&c.waiting)),
            }
        })
        .collect();
    let spaces = w
        .spaces
        .iter()
        .map(|s| rp::Space {
            id: s.id.clone(),
            name: s.name.clone(),
            cwd: s.cwd.clone(),
        })
        .collect();
    json!(rp::List {
        machine: machine(socket),
        spaces,
        terminals,
    })
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
fn stream(id: u64, request: &Value, socket: &Path, out: &Out, streams: &Streams, access: &Access) {
    let mut s = Stream {
        id,
        out,
        streams,
        opened: false,
    };
    let result = match request["op"].as_str() {
        Some("events") => connect(socket, &Request::Subscribe).and_then(|c| events(&mut s, c)),
        _ => text(request, "terminal").map(str::to_owned).and_then(|t| {
            access.tell(Activity::Watching(t.clone()));
            let watched = watch(&mut s, &t, socket);
            access.tell(Activity::Unwatched(t));
            watched
        }),
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
