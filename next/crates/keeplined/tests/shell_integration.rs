//! The shell integration in real shells, started by a real host as the app starts them
//! (`<shell> -l`) under a home of their own: each reports its directory with OSC 7 after
//! `cd`, loads the user's startup files exactly once and is otherwise left as the user
//! set it up. Each integrated shell also marks its commands (OSC 133), and a hooks plugin
//! hears each one that finished, with its exit code. macOS's bash 3.2 is integrated as
//! well; fish keeps its own prompt marks.
use keepline_core::{model::*, protocol::*};
use std::{
    fs, io,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Host {
    child: Child,
    root: PathBuf,
}
impl Host {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!("kl-{}", uuid::Uuid::new_v4().simple()));
        let home = root.join("home");
        fs::create_dir_all(home.join(".config/fish")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        // Each startup file counts its loads; bash's also sets a prompt command to keep.
        let count = |name: &str| format!("echo x >> \"$HOME/loaded-{name}\"\n");
        // A hooks plugin that records every finished command, and one that only hears long ones.
        let plugin = home.join(".config/keepline/plugins/record");
        fs::create_dir_all(&plugin).unwrap();
        fs::write(
            plugin.join("plugin.json"),
            r#"{"api":1,"id":"record","name":"记录","provides":["hooks"]}"#,
        )
        .unwrap();
        let record = |file: &str| format!("(cat; echo) >> '{}'", root.join(file).display());
        fs::write(
            plugin.join("hooks.json"),
            serde_json::json!({"api": 1, "hooks": [
                {"id": "all", "on": ["command_done", "command_failed"], "command": record("commands")},
                {"id": "long", "on": ["command_done"], "command": record("long"), "min_ms": 1000},
            ]})
            .to_string(),
        )
        .unwrap();
        fs::write(
            home.join(".bash_profile"),
            count("bash_profile") + "PROMPT_COMMAND='echo -n;'\n",
        )
        .unwrap();
        fs::write(home.join(".bashrc"), count("bashrc")).unwrap();
        fs::write(home.join(".zshrc"), count("zshrc")).unwrap();
        fs::write(
            home.join(".config/fish/config.fish"),
            "echo x >> $HOME/loaded-config_fish\n",
        )
        .unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "daemon_fixture", "--nocapture"])
            .env("KEEPLINE_FIXTURE_RUNTIME", &root)
            .env("HOME", &home)
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("KEEPLINE_NO_SHELL_INTEGRATION")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let h = Self { child, root };
        let until = Instant::now() + Duration::from_secs(30);
        while !h.root.join("workbench.sock").exists() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(20));
        }
        h.call(Request::Edit {
            revision: match h.call(Request::Workspace).unwrap() {
                Reply::Workspace(w) => w.revision,
                x => panic!("{x:?}"),
            },
            edit: Edit::AddSpace(Space {
                id: "s".into(),
                name: "Tests".into(),
                cwd: "/tmp".into(),
            }),
        })
        .unwrap();
        h
    }
    fn home(&self) -> PathBuf {
        self.root.join("home")
    }
    fn call(&self, request: Request) -> io::Result<Reply> {
        rpc(&self.root.join("workbench.sock"), &request)
    }
    /// What a hook recorded into `file`, one event per line.
    fn recorded(&self, file: &str) -> Vec<serde_json::Value> {
        fs::read_to_string(self.root.join(file))
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    fn loads(&self, name: &str) -> usize {
        fs::read_to_string(self.home().join(format!("loaded-{name}")))
            .map_or(0, |s| s.lines().count())
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.call(Request::Stop { id: "sh".into() });
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
#[ignore = "subprocess daemon entry for integration fixtures"]
fn daemon_fixture() {
    let root = PathBuf::from(
        std::env::var_os("KEEPLINE_FIXTURE_RUNTIME").expect("fixture subprocess only"),
    );
    // As keeplined's main does, so the home's plugins are loaded.
    keepline_core::agent::install();
    keepline_core::hook::install();
    let roots = Agent::all()
        .into_iter()
        .map(|a| (a, root.join("logs").join(a.key())))
        .collect();
    keeplined::serve_with_roots(&root, roots).unwrap();
}

/// A shell in a terminal, its raw output kept as it arrives.
struct Session {
    stream: UnixStream,
    output: Vec<u8>,
}
impl Session {
    fn launch(h: &Host, shell: &str) -> Self {
        h.call(Request::Launch {
            id: "sh".into(),
            space: "s".into(),
            argv: vec![shell.into(), "-l".into()],
            cwd: h.root.to_string_lossy().into_owned(),
            cols: 100,
            rows: 30,
            scheme: Default::default(),
        })
        .unwrap();
        let mut stream = UnixStream::connect(h.root.join("workbench.sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        send(&mut stream, CONTROL, &Request::Hello { major: MAJOR }).unwrap();
        receive::<Reply>(&mut stream).unwrap();
        send(
            &mut stream,
            CONTROL,
            &Request::Attach {
                id: "sh".into(),
                takeover: false,
            },
        )
        .unwrap();
        receive::<Reply>(&mut stream).unwrap();
        let mut output = vec![];
        loop {
            let (kind, bytes) = read_frame(&mut stream).unwrap();
            match kind {
                READY => break,
                _ => output.extend(bytes),
            }
        }
        Self { stream, output }
    }
    fn type_line(&mut self, line: &str) {
        send(
            &mut self.stream,
            CONTROL,
            &Request::Input {
                bytes: format!("{line}\n").into_bytes(),
                lossy: false,
            },
        )
        .unwrap();
    }
    /// Reads output until `done` holds or `wait` passes; whether it held.
    fn read_until(
        &mut self,
        wait: Duration,
        done: impl Fn(&[u8], &Path) -> bool,
        home: &Path,
    ) -> bool {
        let until = Instant::now() + wait;
        while Instant::now() < until {
            if done(&self.output, home) {
                return true;
            }
            match read_frame(&mut self.stream) {
                Ok((OUTPUT, bytes)) => self.output.extend(bytes),
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => panic!("{e}"),
            }
        }
        done(&self.output, home)
    }
}

/// OSC 7's path, as every integration encodes it.
fn encoded(path: &str) -> String {
    path.bytes()
        .map(|b| match b {
            b'-' | b'/' | b'.' | b'_' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
fn reported(output: &[u8], path: &str) -> bool {
    let text = String::from_utf8_lossy(output);
    let want = encoded(path);
    text.split("\x1b]7;file://").skip(1).any(|osc| {
        let osc = osc.split(['\x1b', '\x07']).next().unwrap_or("");
        osc.find('/').is_some_and(|i| osc[i..] == want)
    })
}

/// Changes into a directory with a space and an accent; whether OSC 7 reported it.
fn cd_reports(h: &Host, s: &mut Session) -> bool {
    let dir = h.root.join("k é dir");
    fs::create_dir_all(&dir).unwrap();
    let dir = dir.to_string_lossy().into_owned();
    s.type_line(&format!("cd '{dir}'"));
    let home = h.home();
    s.read_until(Duration::from_secs(10), |out, _| reported(out, &dir), &home)
}

/// Runs `command` (which writes `$HOME/state`) and returns what it wrote.
fn state(h: &Host, s: &mut Session, command: &str) -> String {
    let _ = fs::remove_file(h.home().join("state"));
    s.type_line(command);
    let home = h.home();
    assert!(
        s.read_until(
            Duration::from_secs(10),
            |_, home| home.join("state").exists(),
            &home
        ),
        "the shell never ran: {}",
        String::from_utf8_lossy(&s.output)
    );
    std::thread::sleep(Duration::from_millis(100));
    fs::read_to_string(home.join("state")).unwrap()
}

/// A successful, a failing and a long command, and an empty line; each finished command
/// reaches the hooks once, with its exit code, and only the long one passes `min_ms`.
fn commands_reach_hooks(h: &Host, s: &mut Session) {
    // The commands typed so far are heard too; start from a clean record.
    std::thread::sleep(Duration::from_millis(500));
    let _ = fs::remove_file(h.root.join("commands"));
    let _ = fs::remove_file(h.root.join("long"));
    let home = h.home();
    for (line, count) in [
        ("true", 1),
        ("", 1),
        ("sh -c 'exit 3'", 2),
        ("sleep 1.2", 3),
    ] {
        s.type_line(line);
        let wait = |_: &[u8], _: &Path| h.recorded("commands").len() >= count;
        assert!(
            s.read_until(Duration::from_secs(10), wait, &home),
            "{line:?}: {:?}\n{}",
            h.recorded("commands"),
            String::from_utf8_lossy(&s.output)
        );
    }
    std::thread::sleep(Duration::from_millis(300));
    let seen: Vec<_> = h
        .recorded("commands")
        .iter()
        .map(|e| {
            assert_eq!(e["terminal"], "sh");
            assert_eq!(e["space"], "Tests");
            assert_eq!(e["session"], "");
            assert_eq!(
                e["title"], "",
                "fish titles the terminal with the command line"
            );
            (
                e["event"].as_str().unwrap().to_owned(),
                e["exit_code"].as_i64(),
                e["duration_ms"].as_u64().unwrap() >= 1000,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("command_done".into(), Some(0), false),
            ("command_failed".into(), Some(3), false),
            ("command_done".into(), Some(0), true),
        ],
        "the empty line is no command"
    );
    let long = h.recorded("long");
    assert_eq!(
        long.len(),
        1,
        "only the long command passes min_ms: {long:?}"
    );
}

fn installed(candidates: &[&str]) -> Option<String> {
    candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| p.to_string())
}
/// A bash 4 or later; the test is skipped without one, as on a stock Mac.
fn modern_bash() -> Option<String> {
    ["/opt/homebrew/bin/bash", "/usr/local/bin/bash"]
        .into_iter()
        .find(|p| {
            Command::new(p)
                .args(["-c", "echo ${BASH_VERSINFO[0]}"])
                .output()
                .is_ok_and(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<u32>()
                        .is_ok_and(|v| v >= 4)
                })
        })
        .map(String::from)
}

#[test]
fn zsh_reports_its_directory_and_loads_zshrc_once() {
    let h = Host::start();
    let mut s = Session::launch(&h, "/bin/zsh");
    assert!(
        cd_reports(&h, &mut s),
        "{}",
        String::from_utf8_lossy(&s.output)
    );
    let st = state(&h, &mut s, "echo \"zdotdir=[$ZDOTDIR]\" > \"$HOME/state\"");
    assert_eq!(st.trim(), "zdotdir=[]");
    assert_eq!(h.loads("zshrc"), 1);
    commands_reach_hooks(&h, &mut s);
}

#[test]
fn bash_reports_its_directory_and_is_left_as_the_user_set_it_up() {
    let Some(bash) = modern_bash() else {
        eprintln!("skipped: no bash 4+ installed");
        return;
    };
    let h = Host::start();
    let mut s = Session::launch(&h, &bash);
    assert!(
        cd_reports(&h, &mut s),
        "{}",
        String::from_utf8_lossy(&s.output)
    );
    let st = state(
        &h,
        &mut s,
        r#"printf '%s|%s|%s|%s\n' "$(set -o | grep -c 'posix[[:space:]]*on')" "${HISTFILE#$HOME/}" "$PROMPT_COMMAND" "${ENV-unset}" > "$HOME/state""#,
    );
    assert_eq!(
        st.trim(),
        "0|.bash_history|_keepline_prompt;echo -n;|unset",
        "POSIX mode off, history and the user's prompt command kept, ENV gone"
    );
    assert_eq!(
        h.loads("bash_profile"),
        1,
        "a login shell reads its profile once"
    );
    assert_eq!(
        h.loads("bashrc"),
        0,
        "and not .bashrc, as bash -l would not"
    );
    // The user's prompt command still sees the command's own status.
    let st = state(
        &h,
        &mut s,
        r#"PROMPT_COMMAND='_keepline_prompt;echo "status=$?" > "$HOME/state"'; false"#,
    );
    assert_eq!(st.trim(), "status=1");
    commands_reach_hooks(&h, &mut s);
}

#[test]
fn macos_bash_3_reports_its_directory_too() {
    let h = Host::start();
    let mut s = Session::launch(&h, "/bin/bash");
    assert!(
        cd_reports(&h, &mut s),
        "{}",
        String::from_utf8_lossy(&s.output)
    );
    let st = state(
        &h,
        &mut s,
        r#"printf '%s|%s\n' "$(set -o | grep -c 'posix[[:space:]]*on')" "$PROMPT_COMMAND" > "$HOME/state""#,
    );
    assert_eq!(
        st.trim(),
        "0|_keepline_prompt;echo -n;",
        "not in POSIX mode, the user's prompt command kept"
    );
    assert_eq!(h.loads("bash_profile"), 1);
    // bash 3.2 has no PS0 to mark a command's start, so its commands are not heard.
    std::thread::sleep(Duration::from_millis(500));
    let _ = fs::remove_file(h.root.join("commands"));
    s.type_line("sh -c 'exit 3'");
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        h.recorded("commands").is_empty(),
        "no start mark, no events"
    );
}

#[test]
fn fish_reports_its_directory_and_keeps_its_own_prompt_marks() {
    let Some(fish) = installed(&["/opt/homebrew/bin/fish", "/usr/local/bin/fish"]) else {
        eprintln!("skipped: fish is not installed");
        return;
    };
    let h = Host::start();
    let mut s = Session::launch(&h, &fish);
    assert!(
        cd_reports(&h, &mut s),
        "{}",
        String::from_utf8_lossy(&s.output)
    );
    // The prompt marks the terminal events will read come from fish itself too.
    assert!(String::from_utf8_lossy(&s.output).contains("\x1b]133;A"));
    let st = state(&h, &mut s, "echo \"xdg=[$XDG_DATA_DIRS]\" > $HOME/state");
    assert_eq!(st.trim(), "xdg=[]");
    assert_eq!(h.loads("config_fish"), 1);
    commands_reach_hooks(&h, &mut s);
}
