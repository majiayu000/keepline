//! One UI process per user. The lock lives independently of the daemon/runtime.
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use std::{
    fs::{File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::DirBuilderExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    time::Duration,
};

pub struct Instance {
    _lock: File,
    socket: PathBuf,
}
pub enum Admission {
    Primary(Instance, UnboundedReceiver<()>),
    Forwarded,
}
impl Instance {
    pub fn acquire(directory: &Path) -> io::Result<Admission> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("client.lock"))?;
        let socket = directory.join("client.sock");
        // flock is released by the OS even on a crash; the socket is only a wakeup channel.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(error);
            }
            for _ in 0..40 {
                if UnixStream::connect(&socket).is_ok() {
                    return Ok(Admission::Forwarded);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            return Err(io::Error::other("已有 Keepline 客户端，但无法唤醒其窗口"));
        }
        match std::fs::remove_file(&socket) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        let listener = UnixListener::bind(&socket)?;
        let (tx, rx) = unbounded();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(_) => {
                        if tx.unbounded_send(()).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("client wakeup listener: {error}");
                        break;
                    }
                }
            }
        });
        Ok(Admission::Primary(
            Self {
                _lock: lock,
                socket,
            },
            rx,
        ))
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

pub const OLD_BACKEND: &str = "后台版本过旧，需要重启后台（会中断正在运行的终端）";
pub const RESET_NOTICE: &str = "此版本不迁移旧库。切换到新库后，旧空间、待办、标签布局、终端记录、Agent 会话归属和结局不会保留。原始 CLI 日志文件不会删除。请先处理运行中的终端，再手动备份旧库并切换；本窗口不会重启后台或删除数据。";

/// The running host when it runs other code than this app's own; set on connect.
static STALE_HOST: std::sync::Mutex<Option<keepline_core::protocol::Build>> =
    std::sync::Mutex::new(None);
pub fn stale_build() -> Option<keepline_core::protocol::Build> {
    STALE_HOST.lock().ok()?.clone()
}
pub fn stale_host() -> bool {
    stale_build().is_some()
}
fn set_stale_host(build: Option<keepline_core::protocol::Build>) {
    if let Ok(mut stale) = STALE_HOST.lock() {
        *stale = build;
    }
}
pub fn forget_stale_host() {
    set_stale_host(None);
}
#[cfg(feature = "visual-test")]
pub fn visual_stale_host() {
    set_stale_host(Some(Default::default()));
}

/// Stops the host serving `root` and waits until its socket stops answering; the
/// next start runs this app's host. Its terminals end with it.
pub fn stop_host(root: &Path, host: &keepline_core::protocol::Build) -> io::Result<()> {
    let pid = if host.pid != 0 {
        host.pid
    } else {
        // Hosts before `pid` are found by their exact command line.
        let pattern = format!("{} serve --runtime {}", host.binary, root.display());
        let out = std::process::Command::new("/usr/bin/pgrep")
            .args(["-fx", &pattern])
            .output()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .and_then(|l| l.trim().parse().ok())
            .ok_or_else(|| io::Error::other("找不到正在运行的后台进程"))?
    };
    let socket = root.join("workbench.sock");
    // Gone already: its pid may belong to something else by now.
    if UnixStream::connect(&socket).is_err() {
        return Ok(());
    }
    for (signal, wait) in [(libc::SIGTERM, 50), (libc::SIGKILL, 20)] {
        // SAFETY: kill only sends a signal to the host process found above.
        unsafe { libc::kill(pid as libc::pid_t, signal) };
        for _ in 0..wait {
            if UnixStream::connect(&socket).is_err() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Err(io::Error::other("后台没有在 7 秒内退出"))
}
/// Starts this app's own host for `root` and waits until it serves. With `takeover` it
/// first takes the running terminals over from the host serving there now, which exits.
pub fn spawn_host(root: &Path, takeover: bool) -> io::Result<()> {
    use std::os::unix::{fs::DirBuilderExt, process::CommandExt};
    if !root.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("host.log"))?;
    let daemon = std::env::current_exe()?.with_file_name("keeplined");
    let digest = keepline_core::protocol::Build::digest(&daemon);
    let mut command = std::process::Command::new(daemon);
    command
        .args(["serve", "--runtime"])
        .arg(root)
        .args(takeover.then_some("--takeover"))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log);
    // SAFETY: setsid is async-signal-safe and detaches the host from this UI process.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let socket = root.join("workbench.sock");
    let until = std::time::Instant::now() + Duration::from_secs(if takeover { 20 } else { 5 });
    loop {
        // During a takeover the old host still answers; wait for this build's.
        let serving = if takeover {
            hello_build(&socket).is_ok_and(|b| Some(&b.digest) == digest.as_ref())
        } else {
            keepline_core::protocol::rpc(&socket, &keepline_core::protocol::Request::Workspace)
                .is_ok()
        };
        if serving {
            std::thread::spawn(move || {
                if let Err(e) = child.wait() {
                    eprintln!("host wait: {e}");
                }
            });
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(if takeover {
                format!("新后台没能接手终端（{status}），旧后台照常运行；详情见 host.log")
            } else {
                format!("host exited {status}; see host.log")
            }));
        }
        if std::time::Instant::now() >= until {
            return Err(io::Error::other("后台启动超时，请检查 host.log"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
/// The build a host reports when greeted.
fn hello_build(socket: &Path) -> io::Result<keepline_core::protocol::Build> {
    use keepline_core::protocol::*;
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
    match receive::<Reply>(&mut stream)? {
        Reply::Hello { build, .. } => Ok(build),
        other => Err(io::Error::other(format!("{other:?}"))),
    }
}
/// Replaces the stale host, then opens this app again once this process has quit; the
/// new process connects to the current host. A host that can hand over keeps every
/// terminal running; an older one is stopped and the new process starts a fresh host.
pub fn restart_host(root: &Path) -> io::Result<()> {
    let host = stale_build().ok_or_else(|| io::Error::other("后台已是最新"))?;
    if host.handoff {
        spawn_host(root, true)?;
    } else {
        stop_host(root, &host)?;
    }
    let exe = std::env::current_exe()?;
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.1; done; shift; exec "$@""#)
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(exe)
        .args(std::env::args_os().skip(1))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn connected_host(socket: &Path) -> io::Result<bool> {
    use keepline_core::protocol::*;
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(false);
        }
        Err(e) => return Err(e),
    };
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
    match receive::<Reply>(&mut stream)? {
        Reply::Hello {
            major: MAJOR,
            build,
            ..
        } => {
            // An already running host keeps serving the code it started with, however
            // often its binary is rebuilt; fixes then look like they never landed.
            let sibling = std::env::current_exe()?.with_file_name("keeplined");
            set_stale_host(build.stale(&sibling).then_some(build));
            Ok(true)
        }
        Reply::Error(message) if message.starts_with("protocol major ") => {
            let major = message
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse::<u32>().ok());
            if major.is_some_and(|v| v < MAJOR) {
                let notice = if major == Some(1) {
                    RESET_NOTICE
                } else {
                    "空间、待办和会话结局沿用现有对象表，不作迁移；重启会中断正在运行的终端。本窗口不会重启后台或删除数据。"
                };
                Err(io::Error::other(format!("{OLD_BACKEND}\n\n{notice}")))
            } else {
                Err(io::Error::other(format!(
                    "后台协议不匹配：{message}。后台未被重启。"
                )))
            }
        }
        other => Err(io::Error::other(format!(
            "无法连接后台：{other:?}。后台未被重启。"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_forwards_and_lock_is_reusable() {
        use futures::StreamExt;
        let dir = std::env::temp_dir().join(format!("kl-instance-{}", std::process::id()));
        let Admission::Primary(owner, mut events) = Instance::acquire(&dir).unwrap() else {
            panic!()
        };
        assert!(matches!(
            Instance::acquire(&dir).unwrap(),
            Admission::Forwarded
        ));
        assert_eq!(futures::executor::block_on(events.next()), Some(()));
        drop(owner);
        assert!(matches!(
            Instance::acquire(&dir).unwrap(),
            Admission::Primary(..)
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn old_hosts_are_reported_by_one_handshake_without_mutation() {
        use keepline_core::protocol::*;
        for major in [1, 2] {
            let path = std::env::temp_dir()
                .join(format!("kl-old-host-{}-{major}.sock", std::process::id()));
            let listener = UnixListener::bind(&path).unwrap();
            let peer = std::thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                assert!(matches!(
                    receive::<Request>(&mut s).unwrap(),
                    Request::Hello { major: MAJOR }
                ));
                send(
                    &mut s,
                    CONTROL,
                    &Reply::Error(format!("protocol major {major} required")),
                )
                .unwrap();
            });
            let error = connected_host(&path).unwrap_err().to_string();
            assert!(error.contains(OLD_BACKEND));
            if major == 1 {
                assert!(error.contains("结局不会保留"));
            } else {
                assert!(error.contains("沿用现有对象表"));
            }
            peer.join().unwrap();
            std::fs::remove_file(path).unwrap();
        }
    }
}
