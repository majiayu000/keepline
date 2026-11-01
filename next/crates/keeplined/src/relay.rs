//! `keeplined remote --relay <url>`: this machine on a relay, so a paired phone reaches
//! it from anywhere. The machine dials out and keeps one socket to the relay; each phone
//! that opens `/c/<token>/keepline` is one remote-protocol session, the same as
//! `remote --stdio`, run inside a Noise channel the relay cannot read.
//!
//! `keeplined remote --pair --relay <url>` prints the pairing link and its QR code. The
//! code behind it lives in `remote-code` next to the socket; `--new-code` makes a new one,
//! which unpairs every phone.
//! Shapes are in `specs/extensions/relay.md`.
use crate::control::Args;
use crate::remote;
use futures_util::{SinkExt, StreamExt};
use keepline_relay::{
    secure::{self, Channel, Code, Message as Secure},
    wire::{self, bin, Frame},
};
use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
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

pub fn run(a: &Args) -> io::Result<()> {
    let relay = a
        .value("relay")
        .ok_or_else(|| io::Error::other("--relay 需要中继地址，例如 https://relay.example.com"))?;
    let base = ws_base(relay)?;
    let code = code(&a.socket, a.has("new-code"))?;
    if a.has("pair") {
        return pair(&a.socket, relay, &code);
    }
    let key = a
        .value("relay-key")
        .map(str::to_owned)
        .or_else(|| std::env::var("KEEPLINE_RELAY_KEY").ok())
        .filter(|k| !k.is_empty());
    let name = remote::machine(&a.socket)["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(stay_connected(&base, &code, key, name, &a.socket))
}

/// `https://host` → `wss://host`; a bare host is taken as https.
fn ws_base(relay: &str) -> io::Result<String> {
    let relay = relay.trim_end_matches('/');
    let base = if let Some(rest) = relay.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = relay.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if relay.starts_with("wss://") || relay.starts_with("ws://") {
        relay.to_owned()
    } else if !relay.contains("://") && !relay.is_empty() {
        format!("wss://{relay}")
    } else {
        return Err(io::Error::other(format!("看不懂的中继地址 {relay}")));
    };
    Ok(base)
}

/// The machine's pairing code, made on first use, readable only by this user.
fn code(socket: &Path, new: bool) -> io::Result<Code> {
    let file = socket.with_file_name("remote-code");
    if !new {
        if let Some(code) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| Code::parse(&t))
        {
            return Ok(code);
        }
    }
    let code = Code::generate()?;
    let tmp = file.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?
        .write_all(code.text().as_bytes())?;
    std::fs::rename(&tmp, &file)?;
    Ok(code)
}

fn pair(socket: &Path, relay: &str, code: &Code) -> io::Result<()> {
    let machine = remote::machine(socket);
    let link = format!(
        "keepline://pair?v=1&relay={}&code={}&machine={}&name={}",
        escape(relay),
        code.text(),
        escape(machine["id"].as_str().unwrap_or_default()),
        escape(machine["name"].as_str().unwrap_or_default()),
    );
    let qr = qrcode::QrCode::new(link.as_bytes())
        .map_err(|e| io::Error::other(e.to_string()))?
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    let mut out = io::stdout().lock();
    writeln!(out, "{qr}")?;
    writeln!(out, "用手机 App 扫码配对，或者粘贴这一行：")?;
    writeln!(out, "{link}")?;
    writeln!(
        out,
        "这一行等于这台机器的密码，别发给别人。要让已配对的手机全部失效：keeplined remote --pair --relay {relay} --new-code"
    )
}

fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
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
) -> io::Result<()> {
    let mut wait = Duration::from_secs(1);
    loop {
        let (ended, was_up) = connect(base, code, key.as_deref(), &name, socket).await;
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
    let ended = loop {
        let m = match tokio::time::timeout(SILENCE, stream.next()).await {
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
                Some(Session::Pairing) => match secure::respond(code, payload) {
                    Ok((channel, answer)) => {
                        if out.send(to_phone(proto, &id, &answer)).await.is_err() {
                            break Ended::Lost("写不进中继".into());
                        }
                        eprintln!("keeplined remote: 手机 {id} 已连上");
                        Some(Session::Open(Bridge::start(
                            &id,
                            channel,
                            out.clone(),
                            socket,
                            proto,
                        )))
                    }
                    Err(_) => {
                        eprintln!("keeplined remote: 手机 {id} 的配对码不对，已拒绝");
                        close(&out, &id, 4401, "pairing code mismatch").await;
                        None
                    }
                },
                Some(Session::Open(bridge)) => match bridge.receive(payload) {
                    Ok(()) => Some(Session::Open(bridge)),
                    Err(e) => {
                        eprintln!("keeplined remote: 手机 {id} 断开：{e}");
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
                        sessions.insert(id.clone(), Session::Pairing);
                        Frame::Wopened { id }
                    };
                    let _ = out.send(Message::text(reply.to_text())).await;
                }
                Ok(Frame::Wclose { id, .. }) => {
                    if let Some(Session::Open(_)) = sessions.remove(&id) {
                        eprintln!("keeplined remote: 手机 {id} 已离开");
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
    /// Opened; its first message is the phone's half of the handshake.
    Pairing,
    Open(Bridge),
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
    ) -> Self {
        let (ours, theirs) = UnixStream::pair().expect("socket pair");
        let channel = Arc::new(Mutex::new(channel));
        let credit = Arc::new(Credit::default());
        {
            let socket: PathBuf = socket.into();
            let reader = theirs.try_clone().expect("clone");
            thread::spawn(move || {
                let _ = remote::serve(BufReader::new(reader), theirs, &socket);
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
    fn drop(&mut self) {
        let _ = self.ours.shutdown(std::net::Shutdown::Both);
        if let Ok(mut s) = self.credit.state.lock() {
            s.1 = true;
        }
        self.credit.changed.notify_all();
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
    // The session ended on its own: let the phone know.
    let frame = Frame::Wclose {
        id: id.into(),
        code: Some(1000),
        reason: None,
    };
    let _ = out.blocking_send(Message::text(frame.to_text()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_addresses_become_websocket_bases() {
        assert_eq!(
            ws_base("https://r.example.com/").unwrap(),
            "wss://r.example.com"
        );
        assert_eq!(
            ws_base("http://127.0.0.1:8787").unwrap(),
            "ws://127.0.0.1:8787"
        );
        assert_eq!(ws_base("ws://h:1").unwrap(), "ws://h:1");
        assert_eq!(ws_base("r.example.com").unwrap(), "wss://r.example.com");
        assert!(ws_base("ftp://x").is_err());
        assert_eq!(escape("https://a b/"), "https%3A%2F%2Fa%20b%2F");
    }

    #[test]
    fn the_code_is_kept_private_and_only_replaced_on_request() {
        let dir = std::env::temp_dir().join(format!("kl-code-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("workbench.sock");
        let a = code(&socket, false).unwrap();
        assert_eq!(code(&socket, false).unwrap(), a);
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("remote-code"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let b = code(&socket, true).unwrap();
        assert_ne!(a, b);
        assert_eq!(code(&socket, false).unwrap(), b);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
