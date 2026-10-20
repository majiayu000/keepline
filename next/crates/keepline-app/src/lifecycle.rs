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
        Reply::Hello { major: MAJOR, .. } => Ok(true),
        other => {
            // A version probe only: no workspace reads/writes or compatibility mode.
            let mut old = UnixStream::connect(socket)?;
            old.set_read_timeout(Some(Duration::from_secs(2)))?;
            old.set_write_timeout(Some(Duration::from_secs(2)))?;
            send(&mut old, CONTROL, &Request::Hello { major: 1 })?;
            if matches!(
                receive::<Reply>(&mut old),
                Ok(Reply::Hello { major: 1, .. })
            ) {
                Err(io::Error::other(format!("{OLD_BACKEND}\n\n{RESET_NOTICE}")))
            } else {
                Err(io::Error::other(format!(
                    "无法连接后台：{other:?}。后台未被重启。"
                )))
            }
        }
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
    fn major_one_host_is_reported_without_mutating_it() {
        use keepline_core::protocol::*;
        let path = std::env::temp_dir().join(format!("kl-old-host-{}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let peer = std::thread::spawn(move || {
            for (i, s) in listener.incoming().take(2).enumerate() {
                let mut s = s.unwrap();
                assert!(
                    matches!(receive::<Request>(&mut s).unwrap(),Request::Hello{major} if major==if i==0 {MAJOR}else{1})
                );
                let reply = if i == 0 {
                    Reply::Error("protocol major 1 required".into())
                } else {
                    Reply::Hello {
                        major: 1,
                        instance: "old".into(),
                    }
                };
                send(&mut s, CONTROL, &reply).unwrap();
            }
        });
        let error = connected_host(&path).unwrap_err().to_string();
        assert!(error.contains(OLD_BACKEND));
        assert!(error.contains("结局不会保留"));
        peer.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
