//! `keeplined remote --relay <url>`: this machine on a relay, so a paired phone reaches
//! it from anywhere. The machine dials out and keeps one socket to the relay; each phone
//! that opens `/c/<token>/keepline` is one remote-protocol session, the same as
//! `remote --stdio`, run inside a Noise channel the relay cannot read.
//!
//! Two secrets, both next to the socket and readable only by this user:
//! - `remote-code`, the machine's: phones find it on the relay by a token derived from it.
//! - one key per phone, in `remote-devices.json`: its handshake proves it holds it, and
//!   the key decides what it may do. Unpairing a phone drops its open sessions too.
//!
//! `keeplined remote --pair --relay <url>` pairs one more phone and prints its link and QR
//! code; `--devices` lists them, `--revoke` unpairs one, and `--new-code` replaces the
//! machine's code and unpairs every phone. Shapes are in `specs/extensions/relay.md`.
use crate::control::Args;
use crate::remote::{self, Access, Activity, Observer};
use futures_util::{SinkExt, StreamExt};
use keepline_core::protocol::{
    rpc, RemoteAct, RemoteChange, RemoteLive, RemotePhone, RemoteState, Reply, Request,
};
use keepline_remote::{
    link::Link,
    secure::{self, Channel, Code, Message as Secure},
    wire::{self, bin, Frame},
};
use keeplined::devices::{self, Device, Role};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{mpsc as std_mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// Phones one machine serves at once.
const MAX_SESSIONS: usize = 64;
/// Silence from the relay after which the socket counts as dead; it pings every 25 s.
const SILENCE: Duration = Duration::from_secs(75);
const PING: Duration = Duration::from_secs(30);
/// Phone input waiting for the session; a phone that floods past it is closed.
const INPUT_QUEUE: usize = 256;

/// How often open sessions are checked against the paired devices, so an unpaired
/// phone is dropped within this long.
const RECHECK: Duration = Duration::from_secs(1);

pub fn run(a: &Args) -> io::Result<()> {
    if a.has("devices") {
        return list_devices(&a.socket);
    }
    let root = a.socket.parent().unwrap_or(Path::new("."));
    if a.has("enable") || a.has("disable") {
        let kept = keeplined::remote_config(root);
        let relay = a.value("relay").map_or(kept.relay, str::to_owned);
        let key = a.value("relay-key").map(str::to_owned).or(kept.key);
        let enabled = a.has("enable");
        if enabled && relay.is_empty() {
            return Err(io::Error::other("--enable 需要 --relay <中继地址>"));
        }
        host(
            a,
            Request::RemoteConfigure {
                enabled,
                relay: relay.clone(),
                key,
            },
        )?;
        println!(
            "{}",
            if enabled {
                format!("已打开：后台会一直连着 {relay}，配对过的手机随时能连过来")
            } else {
                "已关闭：后台断开中继，配对还在".to_owned()
            }
        );
        return Ok(());
    }
    if a.has("status") {
        let Reply::Recognition(status) = host(a, Request::RecognitionStatus)? else {
            return Err(io::Error::other("unexpected reply"));
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&status.remote).map_err(io::Error::other)?
        );
        return Ok(());
    }
    if let Some(needle) = a.value("kick") {
        host(
            a,
            Request::RemoteDevice {
                device: needle.to_owned(),
                change: RemoteChange::Kick,
            },
        )?;
        println!("已断开 {needle} 正连着的会话；它还配对着，可以再连");
        return Ok(());
    }
    if let (Some(needle), Some(role)) = (a.value("device"), a.value("role")) {
        let role = devices::parse_role(role)?;
        let d = devices::set_role(&a.socket, needle, role)?;
        refresh(a);
        println!(
            "{}（{}）的权限改成「{}」，它正连着的会话一秒内断开，重连后按新权限",
            d.name,
            d.id,
            d.role.label()
        );
        return Ok(());
    }
    if let Some(needle) = a.value("revoke") {
        let gone = devices::remove(&a.socket, needle)?;
        refresh(a);
        println!(
            "已取消配对 {}（{}），它正连着的会话一秒内断开",
            gone.name, gone.id
        );
        return Ok(());
    }
    let relay = a
        .value("relay")
        .ok_or_else(|| io::Error::other("--relay 需要中继地址，例如 https://relay.example.com"))?;
    let base = Link::websocket_base(relay).map_err(io::Error::other)?;
    let new = a.has("new-code");
    let code = devices::machine_code(&a.socket, new)?;
    if new {
        devices::clear(&a.socket)?;
    }
    if a.has("pair") {
        let role = a
            .value("role")
            .map_or(Ok(Role::Full), devices::parse_role)?;
        return pair(&a.socket, relay, a.value("name").unwrap_or("手机"), role);
    }
    let key = a
        .value("relay-key")
        .map(str::to_owned)
        .or_else(|| std::env::var("KEEPLINE_RELAY_KEY").ok())
        .filter(|k| !k.is_empty());
    let name = devices::machine(&a.socket).name;
    let live = Live::new(a.has("report").then(|| a.socket.clone()));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let ended = runtime.block_on(stay_connected(&base, &code, key, name, &a.socket, &live));
    if let Err(e) = &ended {
        live.state(RemoteState::Failed, e.to_string());
        live.flush();
    }
    ended
}

/// The connection's state, why it is not connected, and the phones on by session.
type Now = (RemoteState, String, BTreeMap<String, RemotePhone>);

/// Who is on now. With `--report` every change goes to the host, a burst at a time,
/// and the host shows it in the app.
#[derive(Clone)]
struct Live {
    inner: Arc<Mutex<Now>>,
    report: Option<(PathBuf, std_mpsc::Sender<()>)>,
}

impl Live {
    fn new(socket: Option<PathBuf>) -> Self {
        let inner = Arc::new(Mutex::new((
            RemoteState::Connecting,
            String::new(),
            BTreeMap::new(),
        )));
        let report = socket.map(|socket| {
            let (wake, woken) = std_mpsc::channel::<()>();
            let (inner, at) = (inner.clone(), socket.clone());
            thread::spawn(move || {
                while woken.recv().is_ok() {
                    // Changes in the next moment go out together.
                    thread::sleep(Duration::from_millis(100));
                    while woken.try_recv().is_ok() {}
                    let _ = rpc(&at, &Request::RemoteReport(snapshot(&inner)));
                }
            });
            (socket, wake)
        });
        Self { inner, report }
    }
    fn change(&self, f: impl FnOnce(&mut Now)) {
        if let Ok(mut i) = self.inner.lock() {
            f(&mut i);
        }
        if let Some((_, wake)) = &self.report {
            let _ = wake.send(());
        }
    }
    fn state(&self, state: RemoteState, detail: String) {
        self.change(|i| {
            i.0 = state;
            i.1 = detail;
            if state != RemoteState::Connected {
                i.2.clear();
            }
        });
    }
    /// Sends the last state now, before the process ends.
    fn flush(&self) {
        if let Some((socket, _)) = &self.report {
            let _ = rpc(socket, &Request::RemoteReport(snapshot(&self.inner)));
        }
    }
    fn joined(&self, session: &str, device: &str) {
        let phone = RemotePhone {
            device: device.to_owned(),
            since: devices::now_ms(),
            watching: vec![],
            last: None,
        };
        self.change(|i| {
            i.2.insert(session.to_owned(), phone);
        });
    }
    fn left(&self, session: &str) {
        self.change(|i| {
            i.2.remove(session);
        });
    }
    fn observer(&self, session: &str) -> Observer {
        let (live, session) = (self.clone(), session.to_owned());
        Arc::new(move |activity| {
            live.change(|i| {
                let Some(p) = i.2.get_mut(&session) else {
                    return;
                };
                match activity {
                    Activity::Watching(t) => p.watching.push(t),
                    Activity::Unwatched(t) => {
                        if let Some(at) = p.watching.iter().position(|w| *w == t) {
                            p.watching.remove(at);
                        }
                    }
                    Activity::Acted { op, terminal } => {
                        p.last = Some(RemoteAct {
                            op,
                            terminal,
                            at: devices::now_ms(),
                        })
                    }
                }
            })
        })
    }
}

fn snapshot(inner: &Mutex<Now>) -> RemoteLive {
    let i = inner.lock().unwrap_or_else(|e| e.into_inner());
    let mut online: Vec<_> = i.2.values().cloned().collect();
    online.sort_by_key(|p| p.since);
    RemoteLive {
        state: i.0,
        detail: i.1.clone(),
        online,
    }
}

/// Lets a running host show the app the devices as they are now; none running is fine.
fn refresh(a: &Args) {
    let _ = rpc(
        &a.socket,
        &Request::RemoteDevice {
            device: String::new(),
            change: RemoteChange::Refresh,
        },
    );
}

/// Asks the running host.
fn host(a: &Args, request: Request) -> io::Result<Reply> {
    match rpc(&a.socket, &request)? {
        Reply::Error(e) => Err(io::Error::other(e)),
        reply => Ok(reply),
    }
}

fn pair(socket: &Path, relay: &str, name: &str, role: Role) -> io::Result<()> {
    let (device, link) = devices::pair_link(socket, relay, name, role)?;
    let qr = qrcode::QrCode::new(link.as_bytes())
        .map_err(|e| io::Error::other(e.to_string()))?
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    let mut out = io::stdout().lock();
    writeln!(out, "{qr}")?;
    writeln!(
        out,
        "「{name}」（{}，权限：{}）：用手机 App 扫码配对，或者粘贴这一行：",
        device.id,
        role.label()
    )?;
    writeln!(out, "{link}")?;
    writeln!(
        out,
        "这一行只给这一台手机用，别发给别人。取消它：keeplined remote --revoke {}",
        device.id
    )
}

fn list_devices(socket: &Path) -> io::Result<()> {
    let list = devices::load(socket);
    if list.is_empty() {
        println!("还没有配对的设备：keeplined remote --pair --relay <中继地址> --name <名字>");
        return Ok(());
    }
    let seen = devices::last_seen(socket);
    let ago = |at: u64| match devices::now().saturating_sub(at) {
        s if s < 60 => "刚才".to_owned(),
        s if s < 3600 => format!("{} 分钟前", s / 60),
        s if s < 86400 => format!("{} 小时前", s / 3600),
        s => format!("{} 天前", s / 86400),
    };
    for d in list {
        let last = seen.get(&d.id).map_or("没连过".to_owned(), |&at| ago(at));
        println!(
            "{}  {}  {}  配对于 {}  最近连接 {}",
            d.id,
            d.name,
            d.role.label(),
            ago(d.paired),
            last
        );
    }
    Ok(())
}

enum Ended {
    /// The relay refused this machine; redialing would not help.
    Refused(String),
    Lost(String),
}

async fn stay_connected(
    base: &str,
    code: &Code,
    key: Option<String>,
    name: String,
    socket: &Path,
    live: &Live,
) -> io::Result<()> {
    let mut wait = Duration::from_secs(1);
    loop {
        let (ended, was_up) = connect(base, code, key.as_deref(), &name, socket, live).await;
        match ended {
            Ended::Refused(why) => return Err(io::Error::other(why)),
            Ended::Lost(why) => {
                if was_up {
                    wait = Duration::from_secs(1);
                }
                eprintln!(
                    "keeplined remote: 和中继断开（{why}），{} 秒后重连",
                    wait.as_secs()
                );
                live.state(
                    RemoteState::Connecting,
                    format!("{why}，{} 秒后重连", wait.as_secs()),
                );
                tokio::time::sleep(wait).await;
                wait = (wait * 2).min(Duration::from_secs(30));
            }
        }
    }
}

/// One connection to the relay, until it ends; says whether it got registered.
async fn connect(
    base: &str,
    code: &Code,
    key: Option<&str>,
    name: &str,
    socket: &Path,
    live: &Live,
) -> (Ended, bool) {
    let hash = wire::token_hash(&code.token());
    let url = format!("{base}{}?th={hash}", wire::AGENT_PATH);
    let ws = match tokio::time::timeout(
        Duration::from_secs(20),
        tokio_tungstenite::connect_async(&url),
    )
    .await
    {
        Ok(Ok((ws, _))) => ws,
        Ok(Err(e)) => return (Ended::Lost(e.to_string()), false),
        Err(_) => return (Ended::Lost("连接超时".into()), false),
    };
    let (mut sink, mut stream) = ws.split();
    let register = Frame::Register {
        token_hash: hash,
        name: Some(name.into()),
        ver: Some(format!("keeplined/{}", env!("CARGO_PKG_VERSION"))),
        key: key.map(Into::into),
        proto: Some(wire::PROTOCOL),
    };
    if let Err(e) = sink.send(Message::text(register.to_text())).await {
        return (Ended::Lost(e.to_string()), false);
    }
    let mut refusal = None;
    let proto = loop {
        let m = tokio::time::timeout(Duration::from_secs(10), stream.next()).await;
        match m {
            Ok(Some(Ok(Message::Text(t)))) => match serde_json::from_str::<Frame>(&t) {
                Ok(Frame::Registered { proto }) => break proto.unwrap_or(1),
                Ok(Frame::Error { message }) => refusal = Some(message),
                _ => (),
            },
            Ok(Some(Ok(Message::Close(f)))) => {
                let code = f.as_ref().map_or(1005, |f| u16::from(f.code));
                let why = refusal.unwrap_or_else(|| format!("中继关闭了连接（{code}）"));
                // Bad key, bad token, or full: the same again would fail the same way.
                return match code {
                    4002 | 4003 | 4401 => {
                        (Ended::Refused(format!("中继拒绝了这台机器：{why}")), false)
                    }
                    _ => (Ended::Lost(why), false),
                };
            }
            Ok(Some(Ok(_))) => (),
            Ok(Some(Err(e))) => return (Ended::Lost(e.to_string()), false),
            Ok(None) => return (Ended::Lost("中继断开".into()), false),
            Err(_) => return (Ended::Lost("中继没有回应注册".into()), false),
        }
    };
    eprintln!("keeplined remote: 已连上中继 {base}");
    live.state(RemoteState::Connected, String::new());

    let (out, mut outbox) = mpsc::channel::<Message>(64);
    let writer = tokio::spawn(async move {
        let mut tick = tokio::time::interval(PING);
        tick.tick().await;
        loop {
            tokio::select! {
                m = outbox.recv() => match m {
                    Some(m) => if sink.send(m).await.is_err() { break },
                    None => break,
                },
                _ = tick.tick() => if sink.send(Message::Ping(Default::default())).await.is_err() { break },
            }
        }
    });

    let mut sessions: HashMap<String, Session> = HashMap::new();
    let log = devices::log_file(socket);
    let mut recheck = tokio::time::interval(RECHECK);
    let mut paired = Paired::default();
    let ended = loop {
        let deadline = tokio::time::Instant::now() + SILENCE;
        let open = sessions.values().any(|s| matches!(s, Session::Open { .. }));
        let m = tokio::select! {
            m = tokio::time::timeout_at(deadline, stream.next()) => m,
            // Only while a phone is on: an unpaired one is dropped within RECHECK.
            _ = recheck.tick(), if open => {
                let now = paired.load(socket);
                let gone: Vec<String> = sessions
                    .iter()
                    .filter_map(|(id, s)| match s {
                        Session::Open { device, role, since, .. } => (!now
                            .iter()
                            .any(|d| &d.id == device && d.role == *role && d.kicked < *since))
                        .then(|| id.clone()),
                        Session::Pairing(_) => None,
                    })
                    .collect();
                for id in gone {
                    // Tell the phone why before the session ends and says only "done".
                    close(&out, &id, 4401, "unpaired").await;
                    if let Some(Session::Open { device, .. }) = sessions.remove(&id) {
                        live.left(&id);
                        eprintln!("keeplined remote: 设备 {device} 被断开、取消配对或改了权限");
                        devices::log(&log, json!({ "event": "dropped", "device": device }));
                    }
                }
                continue;
            }
        };
        let m = match m {
            Err(_) => break Ended::Lost("中继很久没有消息".into()),
            Ok(None) => break Ended::Lost("中继断开".into()),
            Ok(Some(Err(e))) => break Ended::Lost(e.to_string()),
            Ok(Some(Ok(m))) => m,
        };
        // A phone's message: a binary frame, or JSON with base64 on a protocol-1 relay.
        let phone = match &m {
            Message::Binary(b) => match wire::decode(b) {
                Some((bin::WBIN, id, payload)) => Some((id.to_owned(), payload.to_vec())),
                _ => continue,
            },
            Message::Text(t) => match serde_json::from_str::<Frame>(t) {
                Ok(Frame::Wmsg { id, b: Some(b), .. }) => {
                    use base64::Engine;
                    match base64::engine::general_purpose::STANDARD.decode(b) {
                        Ok(b) => Some((id, b)),
                        Err(_) => continue,
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some((id, payload)) = phone {
            let payload = &payload[..];
            let next = match sessions.remove(&id) {
                Some(Session::Pairing(device)) => {
                    let known = paired.load(socket).iter().find(|d| d.id == device).cloned();
                    let answered = known.as_ref().and_then(|d| {
                        let key = d.key()?;
                        secure::respond(&key, payload).ok().map(|r| (d, r))
                    });
                    match answered {
                        Some((d, (channel, answer))) => {
                            if out.send(to_phone(proto, &id, &answer)).await.is_err() {
                                break Ended::Lost("写不进中继".into());
                            }
                            eprintln!(
                                "keeplined remote: 「{}」已连上（{}）",
                                d.name,
                                d.role.label()
                            );
                            devices::log(
                                &log,
                                json!({ "event": "connected", "device": d.id, "name": d.name, "role": d.role }),
                            );
                            live.joined(&id, &d.id);
                            let access = Access {
                                role: d.role,
                                device: Some((d.id.clone(), d.name.clone())),
                                log: Some(log.clone()),
                                observer: Some(live.observer(&id)),
                            };
                            Some(Session::Open {
                                since: devices::now_ms(),
                                bridge: Bridge::start(
                                    &id,
                                    channel,
                                    out.clone(),
                                    socket,
                                    proto,
                                    access,
                                ),
                                device: d.id.clone(),
                                role: d.role,
                            })
                        }
                        None => {
                            eprintln!("keeplined remote: 设备 {device} 没有配对或密钥不对，已拒绝");
                            devices::log(&log, json!({ "event": "refused", "device": device }));
                            close(&out, &id, 4401, "not paired").await;
                            None
                        }
                    }
                }
                Some(Session::Open {
                    bridge,
                    device,
                    role,
                    since,
                }) => match bridge.receive(payload) {
                    Ok(()) => Some(Session::Open {
                        bridge,
                        device,
                        role,
                        since,
                    }),
                    Err(e) => {
                        eprintln!("keeplined remote: 手机 {id} 断开：{e}");
                        live.left(&id);
                        close(&out, &id, 4400, "bad message").await;
                        None
                    }
                },
                None => None,
            };
            if let Some(s) = next {
                sessions.insert(id, s);
            }
            continue;
        }
        match m {
            Message::Text(t) => match serde_json::from_str::<Frame>(&t) {
                Ok(Frame::Wopen { id, path }) => {
                    let ours = path.split('?').next() == Some(wire::KEEPLINE_PATH);
                    let reply = if !ours {
                        Frame::Werr {
                            id,
                            message: "not found".into(),
                        }
                    } else if sessions.len() >= MAX_SESSIONS {
                        Frame::Werr {
                            id,
                            message: "too many phones".into(),
                        }
                    } else {
                        sessions.insert(id.clone(), Session::Pairing(query(&path, "d")));
                        Frame::Wopened { id }
                    };
                    let _ = out.send(Message::text(reply.to_text())).await;
                }
                Ok(Frame::Wclose { id, .. }) => {
                    if let Some(Session::Open { device, .. }) = sessions.remove(&id) {
                        live.left(&id);
                        eprintln!("keeplined remote: 设备 {device} 已离开");
                        devices::log(&log, json!({ "event": "left", "device": device }));
                    }
                }
                // Text messages from a phone are not part of this protocol.
                Ok(Frame::Wmsg { id, s: Some(_), .. }) => {
                    sessions.remove(&id);
                    close(&out, &id, 4400, "binary only").await;
                }
                Ok(Frame::Hreq { id }) => {
                    let head = Frame::Hres {
                        id: id.clone(),
                        status: 404,
                        headers: Default::default(),
                    };
                    let _ = out.send(Message::text(head.to_text())).await;
                    let _ = out.send(Message::text(Frame::Hend { id }.to_text())).await;
                }
                _ => (),
            },
            Message::Close(f) => {
                let code = f.as_ref().map_or(1005, |f| u16::from(f.code));
                break match code {
                    4000 => Ended::Refused(
                        "另一个 keeplined 用同一个配对码连上了中继，这一个退出".into(),
                    ),
                    _ => Ended::Lost(format!("中继关闭了连接（{code}）")),
                };
            }
            _ => (),
        }
    };
    // Dropping a bridge ends its session.
    drop(sessions);
    drop(out);
    writer.abort();
    (ended, true)
}

/// A message to one phone, in the form the relay speaks.
fn to_phone(proto: u32, id: &str, bytes: &[u8]) -> Message {
    if proto >= 2 {
        return Message::binary(wire::encode(bin::WBIN, id, bytes));
    }
    use base64::Engine;
    let b = base64::engine::general_purpose::STANDARD.encode(bytes);
    Message::text(
        Frame::Wmsg {
            id: id.into(),
            s: None,
            b: Some(b),
        }
        .to_text(),
    )
}

async fn close(out: &mpsc::Sender<Message>, id: &str, code: u16, reason: &str) {
    let frame = Frame::Wclose {
        id: id.into(),
        code: Some(code),
        reason: Some(reason.into()),
    };
    let _ = out.send(Message::text(frame.to_text())).await;
}

enum Session {
    /// Opened by the device the path names; its first message is its half of the handshake.
    Pairing(String),
    Open {
        bridge: Bridge,
        device: String,
        /// The role it connected with; a change drops the session.
        role: Role,
        /// Unix milliseconds; a "disconnect now" after it drops the session.
        since: u64,
    },
}

/// The paired devices, read again only when their file changed.
#[derive(Default)]
struct Paired {
    stamp: Option<(std::time::SystemTime, u64)>,
    devices: Vec<Device>,
}
impl Paired {
    fn load(&mut self, socket: &Path) -> &[Device] {
        let stamp = std::fs::metadata(socket.with_file_name("remote-devices.json"))
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        if stamp.is_none() || stamp != self.stamp {
            self.devices = devices::load(socket);
            self.stamp = stamp;
        }
        &self.devices
    }
}

/// `name`'s value in a path's query, or empty.
fn query(path: &str, name: &str) -> String {
    path.split_once('?')
        .map(|(_, q)| q)
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
        .unwrap_or_default()
        .to_owned()
}

/// One phone's session: `remote::serve` on one end of a socket pair, the Noise channel
/// to the phone on the other.
struct Bridge {
    channel: Arc<Mutex<Channel>>,
    input: std_mpsc::SyncSender<Vec<u8>>,
    credit: Arc<Credit>,
    ours: UnixStream,
}

/// DATA bytes in flight to the phone, unacknowledged.
#[derive(Default)]
struct Credit {
    state: Mutex<(u64, bool)>,
    changed: Condvar,
}

impl Bridge {
    fn start(
        id: &str,
        channel: Channel,
        out: mpsc::Sender<Message>,
        socket: &Path,
        proto: u32,
        access: Access,
    ) -> Self {
        let (ours, theirs) = UnixStream::pair().expect("socket pair");
        let channel = Arc::new(Mutex::new(channel));
        let credit = Arc::new(Credit::default());
        {
            let socket: PathBuf = socket.into();
            let reader = theirs.try_clone().expect("clone");
            thread::spawn(move || {
                let _ = remote::serve(BufReader::new(reader), theirs, &socket, access);
            });
        }
        let (input, inbox) = std_mpsc::sync_channel::<Vec<u8>>(INPUT_QUEUE);
        {
            let mut writer = ours.try_clone().expect("clone");
            thread::spawn(move || {
                for bytes in inbox {
                    if writer.write_all(&bytes).is_err() {
                        break;
                    }
                }
                let _ = writer.shutdown(std::net::Shutdown::Write);
            });
        }
        {
            let (mut reader, channel, credit, id) = (
                ours.try_clone().expect("clone"),
                channel.clone(),
                credit.clone(),
                id.to_owned(),
            );
            thread::spawn(move || pump(&mut reader, &channel, &credit, &out, &id, proto));
        }
        Self {
            channel,
            input,
            credit,
            ours,
        }
    }

    fn receive(&self, message: &[u8]) -> io::Result<()> {
        let opened = self
            .channel
            .lock()
            .map_err(|_| io::Error::other("channel lock poisoned"))?
            .open(message)?;
        match opened {
            Secure::Data(bytes) => self
                .input
                .try_send(bytes)
                .map_err(|_| io::Error::other("phone sends faster than the session reads")),
            Secure::Ack(n) => {
                let mut s = self
                    .credit
                    .state
                    .lock()
                    .map_err(|_| io::Error::other("credit"))?;
                s.0 = s.0.saturating_sub(n.into());
                self.credit.changed.notify_all();
                Ok(())
            }
        }
    }
}

impl Drop for Bridge {
    /// Ends the session; the pump sees it was ended here and says nothing more.
    fn drop(&mut self) {
        if let Ok(mut s) = self.credit.state.lock() {
            s.1 = true;
        }
        self.credit.changed.notify_all();
        let _ = self.ours.shutdown(std::net::Shutdown::Both);
    }
}

/// Sends the session's replies to the phone, at most [`secure::WINDOW`] unacknowledged;
/// past that it stops reading, so the session's own streams slow down and a stalled
/// watch is resynced by the host, while other phones keep flowing.
fn pump(
    reader: &mut UnixStream,
    channel: &Mutex<Channel>,
    credit: &Credit,
    out: &mpsc::Sender<Message>,
    id: &str,
    proto: u32,
) {
    let mut buf = vec![0; secure::CHUNK];
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        {
            let Ok(mut s) = credit.state.lock() else {
                return;
            };
            while !s.1 && s.0 + n as u64 > secure::WINDOW {
                s = match credit.changed.wait(s) {
                    Ok(s) => s,
                    Err(_) => return,
                };
            }
            if s.1 {
                return;
            }
            s.0 += n as u64;
        }
        let sealed = match channel.lock().map(|mut c| c.data(&buf[..n])) {
            Ok(Ok(sealed)) => sealed,
            _ => break,
        };
        if out.blocking_send(to_phone(proto, id, &sealed)).is_err() {
            return;
        }
    }
    // The session ended on its own, not by a dropped bridge: let the phone know.
    if credit.state.lock().map_or(true, |s| s.1) {
        return;
    }
    let frame = Frame::Wclose {
        id: id.into(),
        code: Some(1000),
        reason: None,
    };
    let _ = out.blocking_send(Message::text(frame.to_text()));
}
