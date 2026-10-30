use futures::channel::mpsc::UnboundedSender;
use keepline_core::{protocol::*, terminal::Vt};
use std::io;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{SyncSender, sync_channel},
};

pub struct State {
    pub vt: Option<Vt>,
    pub writable: bool,
    pub error: Option<String>,
    pub cols: u16,
    pub rows: u16,
    pub socket: Option<UnixStream>,
    pub ended: bool,
    /// A running terminal's stream broke after its screen arrived: the host drops a
    /// viewer that falls behind a burst of output. Attaching again restores it.
    pub dropped: bool,
    /// The scheme `vt` was last repainted with; a fresh screen from the host has none.
    pub scheme: Option<keepline_core::terminal::Scheme>,
}
pub struct Connection {
    pub state: Arc<Mutex<State>>,
    pub dirty: Arc<AtomicBool>,
    /// BELs the program rang since this pane connected; counted by the display copy.
    pub bells: Arc<AtomicU64>,
    sender: SyncSender<Request>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(s) = self.state.lock()
            && let Some(socket) = &s.socket
        {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}
impl Connection {
    pub fn new(socket: &Path, id: &str, takeover: bool, wake: UnboundedSender<()>) -> Self {
        let state = Arc::new(Mutex::new(State {
            vt: None,
            writable: false,
            error: None,
            cols: 80,
            rows: 24,
            socket: None,
            ended: false,
            dropped: false,
            scheme: None,
        }));
        let dirty = Arc::new(AtomicBool::new(false));
        let bells = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = sync_channel(128);
        let result = Self {
            state: Arc::clone(&state),
            dirty: Arc::clone(&dirty),
            bells: Arc::clone(&bells),
            sender,
        };
        let socket = socket.to_owned();
        let id = id.to_owned();
        std::thread::spawn(move || {
            let work = (|| -> io::Result<()> {
                let mut stream = UnixStream::connect(socket)?;
                stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
                state.lock().unwrap().socket = Some(stream.try_clone()?);
                send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
                match receive::<Reply>(&mut stream)? {
                    Reply::Hello { major: MAJOR, .. } => (),
                    r => return Err(io::Error::other(format!("handshake: {r:?}"))),
                }
                send(&mut stream, CONTROL, &Request::Attach { id, takeover })?;
                match receive::<Reply>(&mut stream)? {
                    Reply::Attached {
                        writable,
                        cols,
                        rows,
                    } => {
                        let mut state = state.lock().unwrap();
                        state.writable = writable;
                        state.cols = cols;
                        state.rows = rows;
                    }
                    r => return Err(io::Error::other(format!("attach: {r:?}"))),
                }
                let mut writer = stream.try_clone()?;
                let write_state = Arc::clone(&state);
                let write_wake = wake.clone();
                std::thread::spawn(move || {
                    for request in receiver {
                        let sent = match &request {
                            Request::Input { bytes, lossy } => {
                                send_input(&mut writer, bytes, *lossy)
                            }
                            _ => send(&mut writer, CONTROL, &request),
                        };
                        if let Err(error) = sent {
                            let mut state = write_state.lock().unwrap();
                            state.error = Some(format!("{error}；输入结果未知，请勿自动重发"));
                            state.writable = false;
                            let _ = write_wake.unbounded_send(());
                            let _ = writer.shutdown(std::net::Shutdown::Both);
                            break;
                        }
                    }
                });
                let mut snapshot = Vec::new();
                loop {
                    let (kind, bytes) = read_frame(&mut stream)?;
                    let mut state = state.lock().unwrap();
                    match kind {
                        SNAPSHOT => {
                            if snapshot.len() + bytes.len() > 64 * 1024 * 1024 {
                                return Err(io::Error::other("snapshot exceeds memory limit"));
                            }
                            snapshot.extend(bytes);
                            continue;
                        }
                        READY => {
                            let mut vt = Vt::restore(&snapshot)?;
                            let rung = Arc::clone(&bells);
                            vt.on_bell(move || {
                                rung.fetch_add(1, Ordering::Relaxed);
                            })?;
                            state.vt = Some(vt);
                            state.scheme = None;
                            snapshot.clear();
                            snapshot.shrink_to_fit();
                        }
                        OUTPUT => {
                            state
                                .vt
                                .as_mut()
                                .ok_or_else(|| io::Error::other("output before snapshot"))?
                                .feed(&bytes);
                        }
                        RESIZED => {
                            let (cols, rows) =
                                serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                            state
                                .vt
                                .as_mut()
                                .ok_or_else(|| io::Error::other("resize before snapshot"))?
                                .resize(cols, rows)?;
                            state.cols = cols;
                            state.rows = rows;
                        }
                        STATUS => {
                            let session: keepline_core::model::Terminal =
                                serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                            if session.state != keepline_core::model::ProcessState::Running {
                                state.writable = false;
                                state.ended = true;
                            }
                        }
                        WRITABLE => {
                            state.writable = serde_json::from_slice::<bool>(&bytes)
                                .map_err(io::Error::other)?
                                && !state.ended;
                        }
                        ACK => {
                            if let Reply::Error(error) =
                                serde_json::from_slice::<Reply>(&bytes).map_err(io::Error::other)?
                            {
                                state.error = Some(error);
                                state.writable = false;
                            }
                        }
                        _ => return Err(io::Error::other("unknown terminal frame")),
                    }
                    drop(state);
                    if !dirty.swap(true, Ordering::AcqRel) {
                        let _ = wake.unbounded_send(());
                    }
                }
            })();
            if let Err(error) = work {
                let mut state = state.lock().unwrap();
                state.error = Some(format!("连接已断开：{error}"));
                state.dropped = state.vt.is_some() && !state.ended;
                state.writable = false;
                if let Some(socket) = &state.socket {
                    let _ = socket.shutdown(std::net::Shutdown::Both);
                }
                let _ = wake.unbounded_send(());
            }
        });
        result
    }
    pub fn send(&self, request: Request) -> Result<(), String> {
        let state = self.state.lock().map_err(|_| "terminal lock poisoned")?;
        if state.vt.is_none() || !state.writable {
            return Err("终端尚未恢复或当前为只读".into());
        }
        drop(state);
        self.sender
            .try_send(request)
            .map_err(|_| "输入队列已满或连接已断开；未自动重试".into())
    }
}

/// One blocking status subscription, independent of terminal output. No refresh timer.
pub struct RecognitionConnection {
    socket: Arc<Mutex<Option<UnixStream>>>,
    closed: Arc<AtomicBool>,
}
impl RecognitionConnection {
    pub fn new(path: &Path, wake: UnboundedSender<RecognitionStatus>) -> Self {
        let socket = Arc::new(Mutex::new(None));
        let closed = Arc::new(AtomicBool::new(false));
        let result = Self {
            socket: Arc::clone(&socket),
            closed: Arc::clone(&closed),
        };
        let path = path.to_owned();
        std::thread::spawn(move || {
            let work = (|| -> io::Result<()> {
                let mut stream = UnixStream::connect(path)?;
                {
                    let mut slot = socket.lock().unwrap();
                    if closed.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    *slot = Some(stream.try_clone()?);
                }
                stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
                send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
                if !matches!(
                    receive::<Reply>(&mut stream)?,
                    Reply::Hello { major: MAJOR, .. }
                ) {
                    return Err(io::Error::other("识别状态协议不匹配"));
                }
                send(&mut stream, CONTROL, &Request::WatchRecognition)?;
                loop {
                    match receive::<Reply>(&mut stream)? {
                        Reply::Recognition(status) => {
                            if wake.unbounded_send(status).is_err() {
                                return Ok(());
                            }
                        }
                        _ => return Err(io::Error::other("识别状态响应无效")),
                    }
                }
            })();
            if let Err(error) = work {
                let _ = wake.unbounded_send(RecognitionStatus {
                    phase: RecognitionPhase::Interrupted,
                    detail: format!("识别状态连接中断：{error}"),
                    scopes: vec![],
                    ..RecognitionStatus::default()
                });
            }
        });
        result
    }
}
impl Drop for RecognitionConnection {
    fn drop(&mut self) {
        let socket = self.socket.lock().unwrap();
        self.closed.store(true, Ordering::Release);
        if let Some(socket) = &*socket {
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
    }
}
