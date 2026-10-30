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
/// This host's runtime directory and executable, handed to every terminal for hooks.
static HOST: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();

/// Lets every later terminal tell its Agents' hooks where this host is.
pub fn announce_host(root: &Path) -> io::Result<()> {
    // As given, only made absolute: resolving links can push the socket past SUN_LEN.
    let root = std::path::absolute(root)?;
    let _ = HOST.set((root, std::env::current_exe()?));
    Ok(())
}

/// What a terminal's programs learn about the host: which terminal they run in, the
/// runtime to reach, and the executable whose `hook` subcommand reports to it.
fn host_env(terminal: &str, host: Option<&(PathBuf, PathBuf)>) -> Vec<(OsString, OsString)> {
    use keepline_core::protocol::{HOOK_ENV, RUNTIME_ENV, TERMINAL_ENV};
    let mut env = vec![(TERMINAL_ENV.into(), terminal.into())];
    if let Some((root, exe)) = host {
        env.push((RUNTIME_ENV.into(), root.as_os_str().to_owned()));
        env.push((HOOK_ENV.into(), exe.as_os_str().to_owned()));
    }
    env
}

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
    std::fs::create_dir_all(dir.join("bash"))?;
    std::fs::write(
        dir.join("bash/keepline.bash"),
        include_str!("../shell-integration/bash/keepline.bash"),
    )?;
    std::fs::create_dir_all(dir.join("fish"))?;
    std::fs::write(
        dir.join("fish/keepline.fish"),
        include_str!("../shell-integration/fish/keepline.fish"),
    )?;
    let _ = INTEGRATION.set(dir);
    Ok(())
}

/// How an interactive shell starts with the integration: its argv and environment.
/// zsh goes through the integration's ZDOTDIR, which restores the user's own before any
/// of their startup files run; bash reads the integration as its init file, which loads
/// the user's files itself; fish sources it after the user's config. A shell given
/// anything beyond `-l` / `-i` (a command, a script) starts untouched.
fn integration(argv: &[String], dir: Option<&Path>) -> (Vec<String>, Vec<(OsString, OsString)>) {
    let untouched = (argv.to_vec(), Vec::new());
    let Some(dir) = dir else {
        return untouched;
    };
    let Some(name) = argv
        .first()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|n| n.to_str())
    else {
        return untouched;
    };
    let flags = &argv[1..];
    let interactive = flags
        .iter()
        .all(|f| matches!(f.as_str(), "-l" | "--login" | "-i"));
    let login = flags.iter().any(|f| f == "-l" || f == "--login");
    let shared = (
        OsString::from("KEEPLINE_SHELL_INTEGRATION_DIR"),
        dir.as_os_str().to_owned(),
    );
    match name {
        "zsh" => (
            argv.to_vec(),
            vec![("ZDOTDIR".into(), dir.join("zsh").into_os_string()), shared],
        ),
        // bash ignores --init-file in a login shell, so the file plays the login part.
        "bash" if interactive => {
            let mut env = vec![shared];
            if login {
                env.push(("KEEPLINE_BASH_LOGIN".into(), "1".into()));
            }
            (
                vec![
                    argv[0].clone(),
                    "--init-file".into(),
                    dir.join("bash/keepline.bash")
                        .to_string_lossy()
                        .into_owned(),
                    "-i".into(),
                ],
                env,
            )
        }
        "fish" if interactive => {
            let script = dir
                .join("fish/keepline.fish")
                .to_string_lossy()
                .into_owned();
            let mut argv = argv.to_vec();
            argv.push("--init-command".into());
            // fish's single quotes only treat \\ and \' specially.
            argv.push(format!(
                "source '{}'",
                script.replace('\\', "\\\\").replace('\'', "\\'")
            ));
            (argv, vec![shared])
        }
        _ => untouched,
    }
}

pub fn spawn(
    argv: &[String],
    cwd: &Path,
    cols: u16,
    rows: u16,
    terminal: &str,
    scheme: keepline_core::terminal::Scheme,
) -> io::Result<(Child, File)> {
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
    let (argv, shell_env) = integration(argv, INTEGRATION.get().map(PathBuf::as_path));
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .env_clear()
        .envs(baseline_env(std::env::vars_os()))
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .env("COLORFGBG", scheme.colorfgbg())
        .envs(shell_env)
        .envs(host_env(terminal, HOST.get()))
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
/// The process group the terminal's foreground job runs in, as a job-control shell
/// sets it; the shell's own group otherwise.
pub fn foreground_group(master: &File) -> Option<i32> {
    let group = unsafe { libc::tcgetpgrp(master.as_raw_fd()) };
    (group > 0).then_some(group)
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
                )));
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

/// A shell starts from the same environment however this host was started, the way a
/// terminal opened from the Dock does: the login identity and locale pass, the rest is
/// left to the login shell's profile. The host is long-lived and inherits whatever
/// launched it, so passing its environment through would freeze a launcher's one-off
/// state into every later shell. Started from inside Claude Code, for example,
/// `CLAUDE_CODE_CHILD_SESSION` reached every shell; Claude launched there believed it
/// was a nested child and stopped saving its transcript, so Keepline lost track of it.
/// A launcher's `PATH` is its own state too, so it falls back to the system default.
/// Agent home overrides pass: this host reads transcripts where they point, so the
/// Agents it starts must write there too.
fn baseline_env(vars: impl Iterator<Item = (OsString, OsString)>) -> Vec<(OsString, OsString)> {
    const PASS: [&str; 11] = [
        "HOME",
        "USER",
        "LOGNAME",
        "SHELL",
        "TMPDIR",
        "LANG",
        "SSH_AUTH_SOCK",
        "__CF_USER_TEXT_ENCODING",
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "GROK_HOME",
    ];
    let mut env: Vec<_> = vars
        .filter(|(k, _)| {
            k.to_str()
                .is_some_and(|k| PASS.contains(&k) || k.starts_with("LC_"))
        })
        .collect();
    env.push(("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into()));
    env
}

#[cfg(test)]
mod env_tests {
    #[test]
    fn only_the_login_baseline_passes_whatever_launched_the_host() {
        let launcher = [
            ("HOME", "/Users/u"),
            ("USER", "u"),
            ("LANG", "zh_CN.UTF-8"),
            ("LC_CTYPE", "UTF-8"),
            ("PATH", "/agent/bin:/usr/bin"),
            ("CLAUDE_CODE_CHILD_SESSION", "1"),
            ("CLAUDECODE", "1"),
            ("CODEX_SANDBOX", "seatbelt"),
            ("SOME_FUTURE_AGENT_MARKER", "1"),
            ("COLUMNS", "80"),
            ("GROK_HOME", "/tmp/grok"),
        ]
        .map(|(k, v)| (k.into(), v.into()));
        let env = super::baseline_env(launcher.into_iter());
        let keys: Vec<_> = env.iter().map(|(k, _)| k.to_str().unwrap()).collect();
        assert_eq!(
            keys,
            ["HOME", "USER", "LANG", "LC_CTYPE", "GROK_HOME", "PATH"]
        );
        assert_eq!(env.last().unwrap().1, "/usr/bin:/bin:/usr/sbin:/sbin");
    }
    #[test]
    fn a_spawned_shell_does_not_see_the_hosts_launcher_state() {
        use std::io::Read;
        std::env::set_var("CLAUDE_CODE_CHILD_SESSION", "1");
        let argv = ["/usr/bin/env".to_owned()];
        let scheme = keepline_core::terminal::Scheme::default();
        let (mut child, mut out) = super::spawn(
            &argv,
            std::path::Path::new("/"),
            80,
            24,
            "scheme-test",
            scheme,
        )
        .unwrap();
        // Read while it runs: macOS holds a closing terminal open until its output is read.
        let mut text = String::new();
        let mut buf = [0u8; 4096];
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match out.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => text.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if child.try_wait().unwrap().is_some() || std::time::Instant::now() > deadline {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        assert!(text.contains("TERM=xterm-256color"), "{text}");
        assert!(
            text.contains("PATH=/usr/bin:/bin:/usr/sbin:/sbin"),
            "{text}"
        );
        assert!(!text.contains("CLAUDE_CODE_CHILD_SESSION"), "{text}");
    }
}

#[cfg(test)]
mod integration_tests {
    use std::path::Path;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn interactive_shells_start_through_the_integration_and_commands_do_not() {
        let dir = Path::new("/rt/shell-integration");
        let (zsh, env) = super::integration(&argv(&["/bin/zsh", "-l"]), Some(dir));
        assert_eq!(zsh, ["/bin/zsh", "-l"]);
        assert_eq!(
            env[0],
            ("ZDOTDIR".into(), "/rt/shell-integration/zsh".into())
        );
        assert_eq!(env[1].1, "/rt/shell-integration");

        let (bash, env) = super::integration(&argv(&["/opt/homebrew/bin/bash", "-l"]), Some(dir));
        assert_eq!(
            bash,
            [
                "/opt/homebrew/bin/bash",
                "--init-file",
                "/rt/shell-integration/bash/keepline.bash",
                "-i"
            ]
        );
        assert!(env.contains(&("KEEPLINE_BASH_LOGIN".into(), "1".into())));
        let (_, env) = super::integration(&argv(&["/bin/bash"]), Some(dir));
        assert!(!env.iter().any(|(k, _)| k == "KEEPLINE_BASH_LOGIN"));

        let odd = Path::new("/rt/it's here");
        let (fish, _) = super::integration(&argv(&["/opt/homebrew/bin/fish", "-l"]), Some(odd));
        assert_eq!(
            fish[1..],
            [
                "-l",
                "--init-command",
                "source '/rt/it\\'s here/fish/keepline.fish'"
            ]
        );

        for untouched in [
            argv(&["/bin/bash", "-l", "-i", "-c", "codex"]),
            argv(&["/opt/homebrew/bin/fish", "-c", "ls"]),
            argv(&["/bin/sh", "-l"]),
        ] {
            assert_eq!(
                super::integration(&untouched, Some(dir)),
                (untouched, vec![])
            );
        }
        assert!(super::integration(&argv(&["/bin/zsh"]), None).1.is_empty());
    }

    /// Runs a real shell through its integration with `cwd` as the start, and returns
    /// the directory its first OSC 7 reports, or None when the shell is not installed.
    fn reported(shell: &str, cwd: &Path, home: &Path) -> Option<String> {
        if !Path::new(shell).exists() {
            return None;
        }
        let root = std::env::temp_dir().join(format!("kl-shint-{}", std::process::id()));
        super::install_shell_integration(&root).unwrap();
        let (argv, env) =
            super::integration(&argv(&[shell, "-l"]), Some(&root.join("shell-integration")));
        let out = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(cwd)
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/bin")
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(b"cd .\nexit\n")?;
                c.wait_with_output()
            })
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let start = text.find("\x1b]7;file://")? + "\x1b]7;file://".len();
        let rest = &text[start..];
        let rest = &rest[rest.find('/')?..];
        Some(rest[..rest.find('\x1b')?].to_owned())
    }

    #[test]
    fn bash_and_fish_report_their_directory_and_still_load_the_users_files() {
        let base = std::env::temp_dir().join(format!("kl-shint-home-{}", std::process::id()));
        let home = base.join("home");
        let cwd = base.join("目录 a");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd = cwd.canonicalize().unwrap();
        // A login bash reads the profile; keepline must not skip it.
        std::fs::write(home.join(".bash_profile"), "echo PROFILE_LOADED\n").unwrap();
        let encoded = cwd
            .to_str()
            .unwrap()
            .replace("目录", "%E7%9B%AE%E5%BD%95")
            .replace(' ', "%20");
        for shell in [
            "/bin/bash",
            "/opt/homebrew/bin/bash",
            "/opt/homebrew/bin/fish",
        ] {
            if Path::new(shell).exists() {
                let dir = reported(shell, &cwd, &home);
                assert_eq!(dir.as_deref(), Some(encoded.as_str()), "{shell}");
            }
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod host_env_tests {
    #[test]
    fn every_terminal_names_itself_and_the_host_once_announced() {
        let names = |env: Vec<(std::ffi::OsString, std::ffi::OsString)>| {
            env.into_iter()
                .map(|(k, v)| format!("{}={}", k.to_string_lossy(), v.to_string_lossy()))
                .collect::<Vec<_>>()
        };
        assert_eq!(names(super::host_env("t1", None)), ["KEEPLINE_TERMINAL=t1"]);
        let host = ("/run/k".into(), "/app/keeplined".into());
        assert_eq!(
            names(super::host_env("t2", Some(&host))),
            [
                "KEEPLINE_TERMINAL=t2",
                "KEEPLINE_RUNTIME=/run/k",
                "KEEPLINE_HOOK=/app/keeplined"
            ]
        );
    }
}
