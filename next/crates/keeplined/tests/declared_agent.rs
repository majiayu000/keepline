//! A CLI Keepline was never compiled to know, added by a declaration file alone: the
//! daemon recognises it, binds its transcript to the terminal and can stop it, or, with no
//! transcript declared, says so on the terminal. See docs/architecture/PLUGIN_SYSTEM.md.
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
    /// A daemon whose config holds `declaration` as `agents/fake.json`.
    fn start(declaration: impl FnOnce(&Path) -> String) -> Self {
        let root = std::env::temp_dir().join(format!("kl-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(root.join("agents")).unwrap();
        // The daemon only serves from a private runtime directory.
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir_all(root.join("logs/fake")).unwrap();
        fs::write(root.join("agents/fake.json"), declaration(&root)).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "daemon_fixture", "--nocapture"])
            .env("KEEPLINE_FIXTURE_RUNTIME", &root)
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
            revision: h.workspace().revision,
            edit: Edit::AddSpace(Space {
                id: "s".into(),
                name: "Tests".into(),
                cwd: "/tmp".into(),
            }),
        })
        .unwrap();
        h
    }
    fn call(&self, request: Request) -> io::Result<Reply> {
        rpc(&self.root.join("workbench.sock"), &request)
    }
    fn workspace(&self) -> Workspace {
        match self.call(Request::Workspace).unwrap() {
            Reply::Workspace(w) => w,
            x => panic!("{x:?}"),
        }
    }
    fn recognition(&self) -> RecognitionStatus {
        match self.call(Request::RecognitionStatus).unwrap() {
            Reply::Recognition(s) => s,
            x => panic!("{x:?}"),
        }
    }
    /// Runs `script` in a terminal as the declared CLI: `/bin/sh` under its name.
    fn run_cli(&self, id: &str, script: &str) {
        let cli = self.root.join("fake-cli");
        if !cli.exists() {
            std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
        }
        self.call(Request::Launch {
            id: id.into(),
            space: "s".into(),
            argv: vec![
                "/bin/sh".into(),
                "-c".into(),
                format!(
                    "{} -c {}; read -r line",
                    shell_quote(&cli.to_string_lossy()),
                    shell_quote(script)
                ),
            ],
            cwd: "/tmp".into(),
            cols: 80,
            rows: 24,
            scheme: Default::default(),
        })
        .unwrap();
    }
    /// Presses Enter in a terminal, which is what prompts a process scan.
    fn enter(&self, id: &str) -> UnixStream {
        let mut stream = UnixStream::connect(self.root.join("workbench.sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        send(&mut stream, CONTROL, &Request::Hello { major: MAJOR }).unwrap();
        receive::<Reply>(&mut stream).unwrap();
        send(
            &mut stream,
            CONTROL,
            &Request::Attach {
                id: id.into(),
                takeover: false,
            },
        )
        .unwrap();
        receive::<Reply>(&mut stream).unwrap();
        while read_frame(&mut stream).unwrap().0 != READY {}
        send(
            &mut stream,
            CONTROL,
            &Request::Input {
                bytes: b"\n".to_vec(),
                lossy: false,
            },
        )
        .unwrap();
        stream
    }
    fn wait(&self, what: &str, ok: impl Fn(&Self) -> bool) {
        let until = Instant::now() + Duration::from_secs(30);
        while !ok(self) {
            assert!(Instant::now() < until, "{what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        if let Ok(Reply::Workspace(w)) = self.call(Request::Workspace) {
            for t in w.terminals {
                let _ = self.call(Request::Stop { id: t.id });
            }
        }
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
    assert!(keepline_core::agent::install_from(
        &root.join("agents"),
        &root
    ));
    let fake = Agent::from_key("fake");
    assert!(
        fake.spec().is_some(),
        "{:?}",
        keepline_core::agent::report()
    );
    let mut roots = vec![(fake, root.join("logs/fake"))];
    // The built-ins keep roots of their own here, so nothing of the user's is read.
    for agent in Agent::all().into_iter().filter(|a| *a != fake) {
        roots.push((agent, root.join("logs").join(agent.key())));
    }
    keeplined::serve_with_roots(&root, roots).unwrap();
}

fn declaration(transcripts: bool) -> impl FnOnce(&Path) -> String {
    move |root| {
        let t = format!(
            r#","transcripts":{{"root":{},"session":"/sid","cwd":"/cwd","role":"/role","text":"/content"}}"#,
            serde_json::to_string(&root.join("logs/fake")).unwrap()
        );
        format!(
            r#"{{"api":1,"id":"fake","name":"Fake CLI","executable":"fake-cli","start":"fake-cli","resume":"fake-cli --resume {{id}}"{}}}"#,
            if transcripts { t.as_str() } else { "" }
        )
    }
}

#[test]
fn a_declared_cli_binds_its_transcript_and_stop_agent_ends_only_it() {
    let h = Host::start(declaration(true));
    let log = h.root.join("logs/fake/one.jsonl");
    let row =
        serde_json::json!({"sid":"fake-1","cwd":"/tmp","role":"user","content":"整理一下发布说明"});
    let pid_file = h.root.join("cli-pid");
    // The CLI holds its transcript open, as a real one does while it runs.
    h.run_cli(
        "pane",
        &format!(
            "exec 3>>{}; echo {} >&3; echo $$ > {}; read -r line",
            shell_quote(&log.to_string_lossy()),
            shell_quote(&row.to_string()),
            shell_quote(&pid_file.to_string_lossy())
        ),
    );
    h.wait(
        "the declared CLI's transcript never bound to its terminal",
        |h| {
            h.workspace()
                .current_agent("pane")
                .is_some_and(|s| s.runtime.key() == "fake" && s.cli_session_id == "fake-1")
        },
    );
    let w = h.workspace();
    let session = w.current_agent("pane").unwrap();
    assert_eq!(w.content[&session.id].title, "整理一下发布说明");

    let pid: i32 = fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    h.call(Request::StopAgent { id: "pane".into() }).unwrap();
    h.wait("Stop Agent did not end the declared CLI", |h| {
        h.workspace().current_agent("pane").is_none()
    });
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "the CLI is still alive");
    let w = h.workspace();
    assert_eq!(
        w.terminals[0].state,
        ProcessState::Running,
        "the shell stays"
    );
    // Stored under its declared id, and read back as the same Agent.
    let db = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
    let runtime: String = db
        .query_row("SELECT runtime FROM agent_sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runtime, "fake");
}

#[test]
fn a_declared_cli_without_transcripts_is_named_on_its_terminal_until_it_exits() {
    let h = Host::start(declaration(false));
    // Two lines: the first is the Enter that prompts a scan, the second ends the CLI.
    h.run_cli("pane", "read -r a; read -r b");
    let mut stream = h.enter("pane");
    h.wait("no note for a CLI that keeps no transcript", |h| {
        h.recognition()
            .terminal_notes
            .get("pane")
            .is_some_and(|n| n.note == "fake-cli · 没有对话记录")
    });
    assert!(h.workspace().current_agent("pane").is_none());
    send(
        &mut stream,
        CONTROL,
        &Request::Input {
            bytes: b"\n".to_vec(),
            lossy: false,
        },
    )
    .unwrap();
    h.wait("the note outlives the CLI", |h| {
        let r = h.recognition();
        !r.terminal_notes.contains_key("pane") && !r.terminal_hints.contains_key("pane")
    });
}
