use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::{ffi::OsStrExt, fs::OpenOptionsExt, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

/// Where this host wrote the shell integration; unset when it is turned off.
static INTEGRATION: OnceLock<PathBuf> = OnceLock::new();

/// Writes the shell integration into the runtime, so the scripts always match this
/// host. `KEEPLINE_NO_SHELL_INTEGRATION` in the host's environment turns it off.
pub fn install_shell_integration(root: &Path) -> io::Result<()> {
    if std::env::var_os("KEEPLINE_NO_SHELL_INTEGRATION").is_some() {
        return Ok(());
    }
    let dir = root.join("shell-integration");
    std::fs::create_dir_all(dir.join("zsh"))?;
    std::fs::write(
        dir.join("zsh/.zshenv"),
        include_str!("../shell-integration/zsh/.zshenv"),
    )?;
    std::fs::write(
        dir.join("zsh/keepline-integration"),
        include_str!("../shell-integration/zsh/keepline-integration"),
    )?;
    let _ = INTEGRATION.set(dir);
    Ok(())
}

/// zsh starts through the integration's ZDOTDIR, which restores the user's own before
/// any of their startup files run. Other shells start untouched for now.
fn integration_env(program: &str, dir: Option<&Path>) -> Vec<(OsString, OsString)> {
    match dir {
        Some(dir) if Path::new(program).file_name().is_some_and(|n| n == "zsh") => vec![
            ("ZDOTDIR".into(), dir.join("zsh").into_os_string()),
            ("KEEPLINE_SHELL_INTEGRATION_DIR".into(), dir.as_os_str().to_owned()),
        ],
        _ => Vec::new(),
    }
}

pub fn spawn(argv: &[String], cwd: &Path, cols: u16, rows: u16) -> io::Result<(Child, File)> {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: initialized out pointers; descriptors immediately become owned.
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    for fd in [&master, &slave] {
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .envs(integration_env(&argv[0], INTEGRATION.get().map(PathBuf::as_path)))
        .env_remove("COLUMNS")
        .env_remove("LINES")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave));
    // SAFETY: only async-signal-safe operations after fork.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            let mut mask: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut mask);
            if libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut()) < 0 {
                return Err(io::Error::last_os_error());
            }
            for signal in [
                libc::SIGINT,
                libc::SIGQUIT,
                libc::SIGTSTP,
                libc::SIGTTIN,
                libc::SIGTTOU,
                libc::SIGWINCH,
                libc::SIGCHLD,
            ] {
                if libc::signal(signal, libc::SIG_DFL) == libc::SIG_ERR {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let child = command.spawn()?;
    Ok((child, File::from(master)))
}

pub fn resize(master: &File, cols: u16, rows: u16, cell: (u32, u32)) -> io::Result<()> {
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: (u32::from(cols) * cell.0).min(u32::from(u16::MAX)) as u16,
        ws_ypixel: (u32::from(rows) * cell.1).min(u32::from(u16::MAX)) as u16,
    };
    if unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &size) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Failed writes report the accepted prefix. Never replay a failed input.
/// Bytes written to the program that it has not read yet, from its side of the
/// terminal; None when that cannot be told.
pub fn backlog(master: &File) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        let mut name = [0 as libc::c_char; 128];
        // SAFETY: TIOCPTYGNAME writes a NUL-terminated path of at most 128 bytes.
        if unsafe {
            libc::ioctl(
                master.as_raw_fd(),
                libc::TIOCPTYGNAME as _,
                name.as_mut_ptr(),
            )
        } < 0
        {
            return None;
        }
        // SAFETY: the buffer is NUL-terminated by the ioctl above.
        let path = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) };
        let path = std::path::Path::new(std::ffi::OsStr::from_bytes(path.to_bytes()));
        let slave = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(path)
            .ok()?;
        let mut pending: libc::c_int = 0;
        // SAFETY: FIONREAD stores the input queue length into `pending`.
        if unsafe { libc::ioctl(slave.as_raw_fd(), libc::FIONREAD, &mut pending) } < 0 {
            return None;
        }
        usize::try_from(pending).ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = master;
        None
    }
}
pub fn write(master: &mut File, bytes: &[u8]) -> io::Result<()> {
    let mut accepted = 0;
    while accepted < bytes.len() {
        let mut poll = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll, 1, 5_000) };
        if ready < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if ready <= 0 {
            return Err(io::Error::other(format!(
                "输入中止，已接受 {accepted} 字节；禁止自动重发"
            )));
        }
        match master.write(&bytes[accepted..bytes.len().min(accepted + 256)]) {
            Ok(0) => {
                return Err(io::Error::other(format!(
                    "PTY 已关闭，已接受 {accepted} 字节"
                )))
            }
            Ok(n) => accepted += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) => {}
            Err(e) => return Err(io::Error::other(format!("{e}; 已接受 {accepted} 字节"))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod integration_tests {
    #[test]
    fn only_zsh_starts_through_the_integration() {
        let dir = std::path::Path::new("/rt/shell-integration");
        let env = super::integration_env("/bin/zsh", Some(dir));
        assert_eq!(env[0], ("ZDOTDIR".into(), "/rt/shell-integration/zsh".into()));
        assert_eq!(env[1].1, "/rt/shell-integration");
        assert!(super::integration_env("/bin/bash", Some(dir)).is_empty());
        assert!(super::integration_env("/bin/zsh", None).is_empty());
    }
}
