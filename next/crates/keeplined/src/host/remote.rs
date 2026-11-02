//! Keeps `keepline remote --relay` running while phones are turned on, as a child of the
//! host: the network side stays in its own process, so a fault there ends only it. Its
//! reports of who is on are passed to the app with the rest of the host's status.
//!
//! The setting lives in `remote-relay.json` next to the socket (0600: it may hold the
//! relay's key). A child that exits on its own was refused (a bad key, another machine
//! with this code) and is not restarted until the setting changes; one killed by a
//! signal it did not get from here is restarted after a pause.
use keepline_core::protocol::{RemoteLive, RemoteState, RemoteStatus};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub enabled: bool,
    pub relay: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

fn file(root: &Path) -> PathBuf {
    root.join("remote-relay.json")
}

pub fn load(root: &Path) -> Config {
    std::fs::read(file(root))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(root: &Path, config: &Config) -> io::Result<()> {
    let path = file(root);
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(config)?)?;
    f.sync_all()?;
    std::fs::rename(&tmp, &path)
}

pub(crate) struct Supervisor {
    root: PathBuf,
    inner: Mutex<Inner>,
    changed: Condvar,
    /// Called whenever what the app should show changed.
    on_change: Box<dyn Fn() + Send + Sync>,
}

struct Inner {
    config: Config,
    /// Bumped by every configure, so the loop knows its child is stale.
    generation: u64,
    child: Option<u32>,
    live: RemoteLive,
}

impl Supervisor {
    pub(crate) fn start(root: &Path, on_change: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        let config = load(root);
        let live = RemoteLive {
            state: if config.enabled {
                RemoteState::Connecting
            } else {
                RemoteState::Off
            },
            ..Default::default()
        };
        let s = Arc::new(Self {
            root: root.to_owned(),
            inner: Mutex::new(Inner {
                config,
                generation: 0,
                child: None,
                live,
            }),
            changed: Condvar::new(),
            on_change: Box::new(on_change),
        });
        let run = s.clone();
        std::thread::spawn(move || run.run());
        s
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn configure(&self, config: Config) -> io::Result<()> {
        if config.enabled {
            keepline_remote::link::Link::websocket_base(&config.relay).map_err(io::Error::other)?;
        }
        save(&self.root, &config)?;
        let mut inner = self.lock();
        inner.config = config;
        inner.generation += 1;
        inner.live = RemoteLive {
            state: if inner.config.enabled {
                RemoteState::Connecting
            } else {
                RemoteState::Off
            },
            ..Default::default()
        };
        if let Some(pid) = inner.child {
            // SAFETY: kill(2) on a child this supervisor started and has not reaped.
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        drop(inner);
        self.changed.notify_all();
        (self.on_change)();
        Ok(())
    }

    /// A report from the running child; ignored once the setting moved on.
    pub(crate) fn report(&self, live: RemoteLive) {
        let mut inner = self.lock();
        if inner.config.enabled && inner.child.is_some() && inner.live != live {
            inner.live = live;
            drop(inner);
            (self.on_change)();
        }
    }

    pub(crate) fn status(&self) -> RemoteStatus {
        let inner = self.lock();
        let socket = self.root.join("workbench.sock");
        let seen = crate::devices::last_seen(&socket);
        RemoteStatus {
            enabled: inner.config.enabled,
            relay: inner.config.relay.clone(),
            keyed: inner.config.key.is_some(),
            devices: crate::devices::load(&socket)
                .into_iter()
                .map(|d| keepline_core::protocol::RemoteDevice {
                    last_seen: seen.get(&d.id).copied(),
                    id: d.id,
                    name: d.name,
                    role: serde_json::to_value(d.role)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    paired: d.paired,
                })
                .collect(),
            live: inner.live.clone(),
        }
    }

    pub(crate) fn socket(&self) -> PathBuf {
        self.root.join("workbench.sock")
    }

    /// The paired devices changed: show the app.
    pub(crate) fn refresh(&self) {
        (self.on_change)();
    }

    pub(crate) fn key(&self) -> Option<String> {
        self.lock().config.key.clone()
    }

    pub(crate) fn relay(&self) -> String {
        self.lock().config.relay.clone()
    }

    fn set(&self, generation: u64, live: RemoteLive) {
        let mut inner = self.lock();
        if inner.generation == generation {
            inner.live = live;
            drop(inner);
            (self.on_change)();
        }
    }

    fn run(&self) {
        let mut pause = Duration::from_secs(1);
        loop {
            let (config, generation) = {
                let mut inner = self.lock();
                while !inner.config.enabled {
                    inner = self.changed.wait(inner).unwrap_or_else(|e| e.into_inner());
                }
                (inner.config.clone(), inner.generation)
            };
            // Tests run the host inside their own binary and name keepline here.
            let exe = match std::env::var_os("KEEPLINE_REMOTE_EXE")
                .map(PathBuf::from)
                .map_or_else(std::env::current_exe, Ok)
            {
                Ok(exe) => exe,
                Err(e) => return eprintln!("keepline: remote unavailable: {e}"),
            };
            let mut command = Command::new(exe);
            command
                .args(["remote", "--relay", &config.relay, "--report", "--runtime"])
                .arg(&self.root)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            match &config.key {
                Some(key) => command.env("KEEPLINE_RELAY_KEY", key),
                None => command.env_remove("KEEPLINE_RELAY_KEY"),
            };
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(e) => {
                    self.failed(generation, format!("启动失败：{e}"));
                    continue;
                }
            };
            self.lock().child = Some(child.id());
            // Its last words say why it stopped; earlier lines are passed on to the log.
            let last = Arc::new(Mutex::new(String::new()));
            let reader = child.stderr.take().map(|err| {
                let last = last.clone();
                std::thread::spawn(move || {
                    for line in io::BufReader::new(err).lines().map_while(Result::ok) {
                        eprintln!("{line}");
                        if let Ok(mut l) = last.lock() {
                            *l = line;
                        }
                    }
                })
            });
            let status = child.wait();
            if let Some(r) = reader {
                let _ = r.join();
            }
            let stale = {
                let mut inner = self.lock();
                inner.child = None;
                inner.generation != generation
            };
            if stale {
                pause = Duration::from_secs(1);
                continue;
            }
            let said = last.lock().map(|l| l.clone()).unwrap_or_default();
            match status {
                Ok(s) if s.signal().is_some() => {
                    self.set(
                        generation,
                        RemoteLive {
                            state: RemoteState::Connecting,
                            detail: format!("连接中继的进程意外结束，{} 秒后重启", pause.as_secs()),
                            online: vec![],
                        },
                    );
                    let inner = self.lock();
                    let _ = self.changed.wait_timeout(inner, pause);
                    pause = (pause * 2).min(Duration::from_secs(30));
                }
                _ => {
                    let why = said.strip_prefix("keepline: ").unwrap_or(&said).to_owned();
                    self.failed(generation, why);
                }
            }
        }
    }

    /// Stops trying until the setting changes.
    fn failed(&self, generation: u64, detail: String) {
        self.set(
            generation,
            RemoteLive {
                state: RemoteState::Failed,
                detail,
                online: vec![],
            },
        );
        let mut inner = self.lock();
        while inner.generation == generation {
            inner = self.changed.wait(inner).unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Some(pid) = self.lock().child {
            // SAFETY: as in `configure`.
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
    }
}
