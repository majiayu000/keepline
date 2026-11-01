//! The relay: machines register over `/agent`, phones open `/c/<token>/…`, and each phone
//! socket is bridged over its machine's socket as `wopen`/`wmsg`/`wclose`. It keeps only
//! `sha256(token)` in memory and never reads a message's contents.
//!
//! A slow side never stalls the others:
//! - Phone → machine: a phone's messages wait for room in its machine's send buffer, so
//!   a machine that reads slowly slows phones down instead of filling the relay.
//! - Machine → phone: a phone that falls more than `phone_buffer` behind is cut off; the
//!   machine's socket keeps flowing for its other phones. Keepline machines keep
//!   [`crate::secure::WINDOW`] per phone in flight, so only a phone that stops
//!   acknowledging gets there.
use crate::wire::{self, bin, Frame};
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        OriginalUri, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    io,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch, OwnedSemaphorePermit, Semaphore},
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct Config {
    /// When set, a machine must register with this key.
    pub key: Option<String>,
    pub max_agents: usize,
    pub max_sockets_per_agent: usize,
    /// Bytes a phone may fall behind before it is cut off.
    pub phone_buffer: usize,
    /// Bytes queued toward one machine before its phones wait.
    pub agent_buffer: usize,
    /// Bytes a phone may send before its machine answers `wopened`.
    pub open_queue: usize,
    pub register_timeout: Duration,
    /// How often the relay pings machines; one silent for two of these is dropped.
    pub ping: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            key: None,
            max_agents: 10_000,
            max_sockets_per_agent: 256,
            phone_buffer: 16 << 20,
            agent_buffer: 8 << 20,
            open_queue: 4 << 20,
            register_timeout: Duration::from_secs(10),
            ping: Duration::from_secs(25),
        }
    }
}

const AGENT_MAX_FRAME: usize = 32 << 20;
const PHONE_MAX_MESSAGE: usize = 24 << 20;

pub async fn serve(listener: TcpListener, config: Config) -> io::Result<()> {
    let relay = Arc::new(Relay {
        config,
        agents: Mutex::default(),
    });
    let app = Router::new()
        .route("/healthz", get(health))
        .route(wire::AGENT_PATH, get(agent))
        .route("/c/{*rest}", get(phone))
        .with_state(relay);
    axum::serve(listener, app).await
}

struct Relay {
    config: Config,
    agents: Mutex<HashMap<String, Arc<Agent>>>,
}

struct Agent {
    out: mpsc::UnboundedSender<Out>,
    /// Room in the send buffer, in bytes; phones take some before queueing a message.
    room: Arc<Semaphore>,
    sockets: Mutex<HashMap<String, Bridge>>,
    next: AtomicU64,
    proto: u32,
}

enum Out {
    Msg(Message, Option<OwnedSemaphorePermit>),
    Close(u16, &'static str),
}

struct Bridge {
    to_phone: mpsc::UnboundedSender<Message>,
    behind: Arc<AtomicUsize>,
    /// Ends the phone's socket now, without waiting for what it has not read.
    cut: watch::Sender<Option<(u16, String)>>,
    opened: bool,
    pending: Vec<Message>,
    pending_bytes: usize,
}

impl Agent {
    fn send(&self, frame: &Frame) {
        let _ = self
            .out
            .send(Out::Msg(Message::Text(frame.to_text().into()), None));
    }
    fn sockets(&self) -> std::sync::MutexGuard<'_, HashMap<String, Bridge>> {
        self.sockets.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Passes a machine's message to its phone, cutting a phone that is too far behind.
    fn to_phone(&self, id: &str, message: Message, limit: usize) {
        let mut sockets = self.sockets();
        let Some(b) = sockets.get(id) else { return };
        let n = size(&message);
        if b.behind.fetch_add(n, Ordering::AcqRel) + n > limit {
            let b = sockets.remove(id).expect("present");
            drop(sockets);
            let _ = b.cut.send(Some((1013, "phone too far behind".into())));
            self.send(&Frame::Wclose {
                id: id.into(),
                code: Some(1013),
                reason: Some("phone too far behind".into()),
            });
            return;
        }
        let _ = b.to_phone.send(message);
    }
    fn close_phone(&self, id: &str, code: u16, reason: String) {
        if let Some(b) = self.sockets().remove(id) {
            let _ = b.to_phone.send(Message::Close(Some(CloseFrame {
                code,
                reason: reason.into(),
            })));
        }
    }
}

async fn health(State(relay): State<Arc<Relay>>) -> Json<serde_json::Value> {
    let agents = relay.agents.lock().map(|a| a.len()).unwrap_or(0);
    Json(serde_json::json!({
        "ok": true,
        "edition": "keepline",
        "version": VERSION,
        "protocol": wire::PROTOCOL,
        "agents": agents,
    }))
}

async fn agent(State(relay): State<Arc<Relay>>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(AGENT_MAX_FRAME)
        .max_frame_size(AGENT_MAX_FRAME)
        .on_upgrade(move |socket| run_agent(relay, socket))
}

async fn phone(
    State(relay): State<Arc<Relay>>,
    OriginalUri(uri): OriginalUri,
    ws: WebSocketUpgrade,
) -> Response {
    let target = uri.path_and_query().map_or(uri.path(), |p| p.as_str());
    let Some((token, rest)) = wire::client_path(target) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let found = relay
        .agents
        .lock()
        .ok()
        .and_then(|a| a.get(&wire::token_hash(token)).cloned());
    let Some(agent) = found else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if agent.sockets().len() >= relay.config.max_sockets_per_agent {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let rest = rest.to_owned();
    ws.max_message_size(PHONE_MAX_MESSAGE)
        .on_upgrade(move |socket| run_phone(relay, agent, rest, socket))
}

async fn run_agent(relay: Arc<Relay>, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let config = &relay.config;
    let register = tokio::time::timeout(config.register_timeout, async {
        while let Some(Ok(m)) = stream.next().await {
            if let Message::Text(t) = m {
                return serde_json::from_str::<Frame>(&t).ok();
            }
        }
        None
    });
    let (token_hash, proto) = match register.await {
        Ok(Some(Frame::Register {
            token_hash,
            key,
            proto,
            ..
        })) => {
            let refuse = if config.key.as_ref().is_some_and(|k| key.as_ref() != Some(k)) {
                Some((4003, "A valid relay key is required"))
            } else if !wire::is_token_hash(&token_hash) {
                Some((4002, "Invalid token hash"))
            } else {
                let agents = relay.agents.lock().unwrap_or_else(|e| e.into_inner());
                (!agents.contains_key(&token_hash) && agents.len() >= config.max_agents)
                    .then_some((1013, "This relay is full"))
            };
            if let Some((code, message)) = refuse {
                let error = Frame::Error {
                    message: message.into(),
                };
                let _ = sink.send(Message::Text(error.to_text().into())).await;
                let _ = sink.send(close(code, message)).await;
                return;
            }
            (
                token_hash,
                proto
                    .filter(|&p| p >= 2)
                    .map_or(1, |p| p.min(wire::PROTOCOL)),
            )
        }
        _ => return,
    };

    let (out, mut outbox) = mpsc::unbounded_channel();
    let agent = Arc::new(Agent {
        out,
        room: Arc::new(Semaphore::new(config.agent_buffer)),
        sockets: Mutex::default(),
        next: AtomicU64::new(1),
        proto,
    });
    let replaced = relay
        .agents
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(token_hash.clone(), agent.clone());
    if let Some(old) = replaced {
        let _ = old
            .out
            .send(Out::Close(4000, "Replaced by a newer connection"));
    }
    agent.send(&Frame::Registered { proto: Some(proto) });

    let seen = Arc::new(Mutex::new(Instant::now()));
    let ping = config.ping;
    let writer = {
        let seen = seen.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(ping);
            tick.tick().await;
            loop {
                tokio::select! {
                    next = outbox.recv() => match next {
                        Some(Out::Msg(m, _permit)) => {
                            if sink.send(m).await.is_err() { break }
                        }
                        Some(Out::Close(code, reason)) => {
                            let _ = sink.send(close(code, reason)).await;
                            break;
                        }
                        None => break,
                    },
                    _ = tick.tick() => {
                        let silent = seen.lock().map(|s| s.elapsed()).unwrap_or_default();
                        if silent > ping * 2 || sink.send(Message::Ping(Default::default())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        })
    };

    let limit = config.phone_buffer;
    while let Some(Ok(m)) = tokio::select! {
        m = stream.next() => m,
        // The writer ended: replaced, gone silent, or the socket broke.
        _ = wait_closed(&agent.out) => None,
    } {
        if let Ok(mut s) = seen.lock() {
            *s = Instant::now();
        }
        match m {
            Message::Binary(b) if proto >= 2 => {
                if let Some((kind @ (bin::WBIN | bin::WTEXT), id, payload)) = wire::decode(&b) {
                    let m = if kind == bin::WBIN {
                        Message::Binary(payload.to_vec().into())
                    } else {
                        match std::str::from_utf8(payload) {
                            Ok(s) => Message::Text(s.into()),
                            Err(_) => continue,
                        }
                    };
                    agent.to_phone(id, m, limit);
                }
            }
            Message::Text(t) => match serde_json::from_str::<Frame>(&t) {
                Ok(Frame::Wopened { id }) => {
                    let mut sockets = agent.sockets();
                    match sockets.get_mut(&id) {
                        Some(b) => {
                            b.opened = true;
                            b.pending_bytes = 0;
                            for m in b.pending.drain(..) {
                                let _ = agent.out.send(Out::Msg(bridged(&id, m, proto), None));
                            }
                        }
                        None => {
                            drop(sockets);
                            agent.send(&Frame::Wclose {
                                id,
                                code: Some(1000),
                                reason: None,
                            });
                        }
                    }
                }
                Ok(Frame::Werr { id, .. }) => {
                    agent.close_phone(&id, 1011, "Machine could not open the socket".into())
                }
                Ok(Frame::Wmsg { id, s: Some(s), .. }) => {
                    agent.to_phone(&id, Message::Text(s.into()), limit)
                }
                Ok(Frame::Wmsg { id, b: Some(b), .. }) => {
                    use base64::Engine;
                    if let Ok(b) = base64::engine::general_purpose::STANDARD.decode(b) {
                        agent.to_phone(&id, Message::Binary(b.into()), limit);
                    }
                }
                Ok(Frame::Wclose { id, code, reason }) => agent.close_phone(
                    &id,
                    sanitize(code),
                    reason.unwrap_or_default().chars().take(120).collect(),
                ),
                _ => (),
            },
            Message::Close(_) => break,
            _ => (),
        }
    }

    writer.abort();
    {
        let mut agents = relay.agents.lock().unwrap_or_else(|e| e.into_inner());
        if agents
            .get(&token_hash)
            .is_some_and(|a| Arc::ptr_eq(a, &agent))
        {
            agents.remove(&token_hash);
        }
    }
    // Phones redial instead of talking into a dead tunnel.
    let gone: Vec<_> = agent.sockets().drain().map(|(_, b)| b).collect();
    for b in gone {
        let _ = b.cut.send(Some((1012, "Machine disconnected".into())));
    }
}

async fn run_phone(relay: Arc<Relay>, agent: Arc<Agent>, path: String, socket: WebSocket) {
    let id = format!("w{}", agent.next.fetch_add(1, Ordering::Relaxed));
    let (to_phone, mut inbox) = mpsc::unbounded_channel();
    let behind = Arc::new(AtomicUsize::new(0));
    let (cut, mut cut_rx) = watch::channel(None);
    agent.sockets().insert(
        id.clone(),
        Bridge {
            to_phone,
            behind: behind.clone(),
            cut,
            opened: false,
            pending: vec![],
            pending_bytes: 0,
        },
    );
    agent.send(&Frame::Wopen {
        id: id.clone(),
        path,
    });
    let (mut sink, mut stream) = socket.split();
    let mut writer_cut = cut_rx.clone();
    let writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                // A cut wins over a closed inbox: the bridge is dropped right after it.
                biased;
                (code, reason) = was_cut(&mut writer_cut) => {
                    let close = Message::Close(Some(CloseFrame { code, reason: reason.into() }));
                    let _ = tokio::time::timeout(Duration::from_millis(200), sink.send(close)).await;
                    break;
                }
                m = inbox.recv() => {
                    let Some(m) = m else { break };
                    let n = size(&m);
                    let closing = matches!(m, Message::Close(_));
                    if sink.send(m).await.is_err() || closing { break }
                    behind.fetch_sub(n, Ordering::AcqRel);
                }
            }
        }
    });

    let open_queue = relay.config.open_queue;
    let mut code = 1000;
    loop {
        let m = tokio::select! {
            m = stream.next() => m,
            _ = was_cut(&mut cut_rx) => break,
        };
        let m = match m {
            Some(Ok(m @ (Message::Text(_) | Message::Binary(_)))) => m,
            Some(Ok(Message::Close(f))) => {
                code = f.map_or(1000, |f| f.code);
                break;
            }
            Some(Ok(_)) => continue,
            _ => {
                code = 1006;
                break;
            }
        };
        let n = size(&m);
        {
            let mut sockets = agent.sockets();
            let Some(b) = sockets.get_mut(&id) else { break };
            if !b.opened {
                b.pending_bytes += n;
                if b.pending_bytes > open_queue {
                    let b = sockets.remove(&id).expect("present");
                    drop(sockets);
                    let _ = b.cut.send(Some((
                        1009,
                        "Too much data before the machine answered".into(),
                    )));
                    agent.send(&Frame::Wclose {
                        id: id.clone(),
                        code: Some(1009),
                        reason: None,
                    });
                    break;
                }
                b.pending.push(m);
                continue;
            }
        }
        let want = n.clamp(1, relay.config.agent_buffer) as u32;
        let permit = tokio::select! {
            p = agent.room.clone().acquire_many_owned(want) => p.ok(),
            _ = was_cut(&mut cut_rx) => break,
        };
        let _ = agent
            .out
            .send(Out::Msg(bridged(&id, m, agent.proto), permit));
    }
    // Still registered: the phone left on its own, so tell the machine.
    if agent.sockets().remove(&id).is_some() {
        agent.send(&Frame::Wclose {
            id: id.clone(),
            code: Some(sanitize(Some(code))),
            reason: None,
        });
    }
    let _ = writer.await;
}

/// A phone's message as it travels on its machine's socket.
fn bridged(id: &str, m: Message, proto: u32) -> Message {
    use base64::Engine;
    match (m, proto >= 2) {
        (Message::Binary(b), true) => Message::Binary(wire::encode(bin::WBIN, id, &b).into()),
        (Message::Text(t), true) => {
            Message::Binary(wire::encode(bin::WTEXT, id, t.as_bytes()).into())
        }
        (Message::Binary(b), false) => Message::Text(
            Frame::Wmsg {
                id: id.into(),
                s: None,
                b: Some(base64::engine::general_purpose::STANDARD.encode(b)),
            }
            .to_text()
            .into(),
        ),
        (Message::Text(t), false) => Message::Text(
            Frame::Wmsg {
                id: id.into(),
                s: Some(t.to_string()),
                b: None,
            }
            .to_text()
            .into(),
        ),
        (m, _) => m,
    }
}

fn size(m: &Message) -> usize {
    match m {
        Message::Text(t) => t.len(),
        Message::Binary(b) => b.len(),
        _ => 0,
    }
}

fn close(code: u16, reason: &str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

/// Close codes a peer may set: normal, or the application range.
fn sanitize(code: Option<u16>) -> u16 {
    code.filter(|c| matches!(c, 1000 | 3000..=4999))
        .unwrap_or(1000)
}

/// Resolves once the phone is cut; a bridge dropped without cutting is not a cut.
async fn was_cut(rx: &mut watch::Receiver<Option<(u16, String)>>) -> (u16, String) {
    let why = rx
        .wait_for(Option::is_some)
        .await
        .ok()
        .and_then(|w| w.clone());
    match why {
        Some(why) => why,
        None => std::future::pending().await,
    }
}

async fn wait_closed<T>(tx: &mpsc::UnboundedSender<T>) {
    tx.closed().await
}
