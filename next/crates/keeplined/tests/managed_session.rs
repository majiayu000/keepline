use keepline_core::{model::*, protocol::*, terminal::Vt};
use std::{
    fs, io,
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::{Duration, Instant},
};

// Functional fixtures still run in parallel. Resource measurements exclusively
// own the fixture lane so 40-PTY setup, SQLite fsync and FSEvents recovery from
// sibling tests cannot contaminate their CPU/I/O window. This is test-only:
// the measured PID remains the isolated daemon, never the harness or host CPU.
static FIXTURES: RwLock<()> = RwLock::new(());
enum FixtureLease {
    // Created only while a measured Host already holds the exclusive fixture lane.
    Companion,
    Shared {
        _guard: RwLockReadGuard<'static, ()>,
    },
    Measured {
        _guard: RwLockWriteGuard<'static, ()>,
    },
}
struct Host {
    child: Child,
    root: PathBuf,
    _lease: FixtureLease,
}
impl Host {
    fn start() -> Self {
        Self::with_lease(FixtureLease::Shared {
            _guard: FIXTURES.read().unwrap_or_else(|e| e.into_inner()),
        })
    }
    fn measured() -> Self {
        Self::with_lease(FixtureLease::Measured {
            _guard: FIXTURES.write().unwrap_or_else(|e| e.into_inner()),
        })
    }
    fn with_lease(lease: FixtureLease) -> Self {
        let root = std::env::temp_dir().join(format!("kl-{}", uuid::Uuid::new_v4().simple()));
        let child = fixture_command(&root)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let h = Self {
            child,
            root,
            _lease: lease,
        };
        let until = Instant::now() + Duration::from_secs(30);
        while !h.socket().exists() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(20));
        }
        h
    }
    fn socket(&self) -> PathBuf {
        self.root.join("workbench.sock")
    }
    fn call(&self, request: Request) -> io::Result<Reply> {
        rpc(&self.socket(), &request)
    }
    fn workspace(&self) -> Workspace {
        match self.call(Request::Workspace).unwrap() {
            Reply::Workspace(w) => w,
            x => panic!("{x:?}"),
        }
    }
    fn setup(&self) {
        self.call(Request::Edit {
            revision: self.workspace().revision,
            edit: Edit::AddSpace(Space {
                id: "s".into(),
                name: "Tests".into(),
                cwd: "/tmp".into(),
            }),
        })
        .unwrap();
    }
    fn launch(&self, id: &str, cmd: &str) -> Request {
        Request::Launch {
            id: id.into(),
            space: "s".into(),
            argv: vec!["/bin/sh".into(), "-c".into(), cmd.into()],
            cwd: "/tmp".into(),
            cols: 80,
            rows: 24,
        }
    }
    fn attach(&self, id: &str, takeover: bool) -> (UnixStream, Vt, bool) {
        let mut stream = UnixStream::connect(self.socket()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        send(&mut stream, CONTROL, &Request::Hello { major: MAJOR }).unwrap();
        assert!(matches!(
            receive::<Reply>(&mut stream).unwrap(),
            Reply::Hello { major: MAJOR, .. }
        ));
        send(
            &mut stream,
            CONTROL,
            &Request::Attach {
                id: id.into(),
                takeover,
            },
        )
        .unwrap();
        let writable = match receive::<Reply>(&mut stream).unwrap() {
            Reply::Attached { writable, .. } => writable,
            other => panic!("{other:?}"),
        };
        let mut bytes = Vec::new();
        loop {
            let (kind, chunk) = read_frame(&mut stream).unwrap();
            if kind == READY {
                break;
            }
            assert_eq!(kind, SNAPSHOT);
            bytes.extend(chunk);
        }
        (stream, Vt::restore(&bytes).unwrap(), writable)
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            if let Ok(Reply::Workspace(w)) = self.call(Request::Workspace) {
                for s in w.terminals {
                    let _ = self.call(Request::Stop { id: s.id });
                }
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn ack(stream: &mut UnixStream) -> Reply {
    loop {
        let (kind, bytes) = read_frame(stream).unwrap();
        if kind == ACK {
            return serde_json::from_slice(&bytes).unwrap();
        }
    }
}
// proc_pid_rusage uses Mach absolute time on Apple Silicon. Dividing raw ticks
// by 1e9 undercounts CPU by 125/3 on this host; convert using the OS timebase.
#[allow(deprecated)] // The existing libc binding is sufficient; no extra dependency for this test.
fn cpu_seconds(ticks: u64) -> f64 {
    let mut base = libc::mach_timebase_info { numer: 0, denom: 0 };
    assert_eq!(unsafe { libc::mach_timebase_info(&mut base) }, 0);
    ticks as f64 * base.numer as f64 / base.denom as f64 / 1e9
}

#[test]
fn reconnect_input_resize_and_takeover_are_real_pty_operations() {
    let h = Host::start();
    h.setup();
    h.call(h.launch("one", "stty -echo; cat")).unwrap();
    let (mut first, _, writable) = h.attach("one", false);
    assert!(writable);
    send(
        &mut first,
        CONTROL,
        &Request::Input {
            bytes: b"before-detach\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut first), Reply::Ok));
    let (mut second, _, writable) = h.attach("one", false);
    assert!(!writable);
    send(
        &mut second,
        CONTROL,
        &Request::Input {
            bytes: b"must-not-write\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut second), Reply::Error(_)));
    let (mut third, _, writable) = h.attach("one", true);
    assert!(writable);
    send(&mut first, CONTROL, &Request::Resize { cols: 90, rows: 30 }).unwrap();
    assert!(matches!(ack(&mut first), Reply::Error(_)));
    send(&mut third, CONTROL, &Request::Resize { cols: 90, rows: 30 }).unwrap();
    assert!(matches!(ack(&mut third), Reply::Ok));
    drop(first);
    drop(second);
    drop(third);
    let (mut again, vt, _) = h.attach("one", true);
    assert_eq!(vt.terminal().cols().unwrap(), 90);
    assert_eq!(vt.terminal().rows().unwrap(), 30);
    send(
        &mut again,
        CONTROL,
        &Request::Input {
            bytes: b"after-detach\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut again), Reply::Ok));
    assert_eq!(h.workspace().terminals[0].state, ProcessState::Running);
}
#[test]
fn launch_is_durable_and_deduplicated_and_tab_close_is_not_stop() {
    let h = Host::start();
    h.setup();
    h.call(h.launch("one", "sleep 30")).unwrap();
    let pid = h.workspace().terminals[0].pid;
    h.call(h.launch("one", "sleep 30")).unwrap();
    assert_eq!(h.workspace().terminals.len(), 1);
    assert_eq!(h.workspace().terminals[0].pid, pid);
    assert!(h.call(h.launch("one", "sleep 31")).is_err());
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::PutTab {
            tab: Tab {
                id: "t".into(),
                space: "s".into(),
                default_task: None,
                title: "Test".into(),
                closed: false,
                layout: Some(Layout::Leaf("one".into())),
            },
        },
    })
    .unwrap();
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::CloseTab { id: "t".into() },
    })
    .unwrap();
    assert_eq!(h.workspace().terminals[0].state, ProcessState::Running);
    let connection = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
    let closed: bool = connection
        .query_row("SELECT closed FROM tabs WHERE id='t'", [], |r| r.get(0))
        .unwrap();
    let tab: String = connection
        .query_row(
            "SELECT tab_id FROM tab_layout_nodes WHERE terminal_id='one'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(closed);
    assert_eq!(tab, "t");
    let count: i64 = connection
        .query_row("SELECT count(*) FROM agent_sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "a shell is not an Agent conversation");
}
#[test]
fn process_exit_does_not_complete_user_work() {
    let h = Host::start();
    h.setup();
    h.call(h.launch("one", "printf done; exit 7")).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let w = h.workspace();
        if w.terminals[0].state == ProcessState::Exited {
            assert_eq!(w.terminals[0].exit_code, Some(7));
            assert!(w.agent_sessions.is_empty());
            break;
        }
        assert!(Instant::now() < until, "state {:?}", w.terminals[0].state);
        std::thread::sleep(Duration::from_millis(20));
    }
    let (_, _, writable) = h.attach("one", false);
    assert!(!writable);
}
#[test]
fn a_second_daemon_cannot_replace_the_live_socket() {
    let h = Host::start();
    let result = fixture_command(&h.root).output().unwrap();
    assert!(!result.status.success());
    assert!(h.call(Request::Workspace).is_ok());
}

fn wait_file(path: &std::path::Path) -> String {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(body) = fs::read_to_string(path) {
            if !body.trim().is_empty() {
                return body;
            }
        }
        assert!(Instant::now() < until, "missing {}", path.display());
        std::thread::sleep(Duration::from_millis(25));
    }
}
#[test]
fn two_cli_conversations_in_one_real_pty_keep_separate_ownership_after_restart() {
    let mut h = Host::start();
    h.setup();
    let codex = h.root.join("codex");
    let claude = h.root.join("claude");
    std::os::unix::fs::symlink("/bin/sh", &codex).unwrap();
    std::os::unix::fs::symlink("/bin/sh", &claude).unwrap();
    let first = h.root.join("first.jsonl");
    let second = h.root.join("second.jsonl");
    fs::write(
        &first,
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-one\",\"cwd\":\"/tmp\"}}\n",
    )
    .unwrap();
    fs::write(&second,"{\"type\":\"user\",\"sessionId\":\"claude-two\",\"cwd\":\"/tmp\",\"message\":{\"content\":\"test\"}}\n").unwrap();
    for id in ["first-task", "second-task"] {
        h.call(Request::Edit {
            revision: h.workspace().revision,
            edit: Edit::AddTask(Task {
                id: id.into(),
                space: "s".into(),
                title: id.into(),
                description: "test".into(),
                acceptance: vec![AcceptanceCriterion {
                    id: "criterion-1".into(),
                    description: "关联正确".into(),
                    evidence: None,
                }],
                agent: Agent::Codex,
            }),
        })
        .unwrap();
    }
    fs::write(
        h.root.join("hook.json"),
        serde_json::to_vec(&serde_json::json!({"session_id":"codex-one","transcript_path":first}))
            .unwrap(),
    )
    .unwrap();
    let first_script = format!(
        "echo $$ > {root}/pid1; while [ ! -f {root}/go ]; do sleep 0.05; done; {helper} hook --runtime {root} --agent codex < {root}/hook.json; echo $? > {root}/hook-status; sleep 30",
        root = h.root.display(),
        helper = env!("CARGO_BIN_EXE_keeplined")
    );
    let command = format!(
        "{} -c {}; {} -c {}",
        shell_quote(&codex.to_string_lossy()),
        shell_quote(&first_script),
        shell_quote(&claude.to_string_lossy()),
        shell_quote(&format!("echo $$ > {}/pid2; sleep 30", h.root.display()))
    );
    h.call(h.launch("one", &command)).unwrap();
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::PutTab {
            tab: Tab {
                id: "t".into(),
                space: "s".into(),
                default_task: Some("first-task".into()),
                title: "test".into(),
                closed: false,
                layout: Some(Layout::Leaf("one".into())),
            },
        },
    })
    .unwrap();
    fs::write(h.root.join("go"), "start").unwrap();
    assert_eq!(
        wait_file(&h.root.join("hook-status")).trim(),
        "0",
        "real hook helper failed"
    );
    let pid1: u32 = wait_file(&h.root.join("pid1")).trim().parse().unwrap();
    let hook = || Request::Hook {
        runtime: Agent::Codex,
        pid: pid1,
        cli_session_id: "codex-one".into(),
        transcript: first.to_string_lossy().into_owned(),
        ended: false,
        event: None,
    };
    h.call(hook()).unwrap();
    h.call(hook()).unwrap();
    let first_id = h.workspace().agent_sessions[0].id.clone();
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::SetOutcome {
            id: first_id.clone(),
            outcome: Outcome::Completed,
        },
    })
    .unwrap();
    assert_eq!(unsafe { libc::kill(pid1 as i32, libc::SIGTERM) }, 0);
    let pid2: u32 = wait_file(&h.root.join("pid2")).trim().parse().unwrap();
    h.call(Request::Hook {
        runtime: Agent::Claude,
        pid: pid2,
        cli_session_id: "wrong".into(),
        transcript: second.to_string_lossy().into_owned(),
        ended: false,
        event: None,
    })
    .unwrap_err();
    h.call(Request::Hook {
        runtime: Agent::Claude,
        pid: pid2,
        cli_session_id: "claude-two".into(),
        transcript: second.to_string_lossy().into_owned(),
        ended: false,
        event: None,
    })
    .unwrap();
    let second_id = h
        .workspace()
        .agent_sessions
        .iter()
        .find(|s| s.runtime == Agent::Claude)
        .unwrap()
        .id
        .clone();
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::AssignTask {
            id: second_id,
            task: Some("second-task".into()),
        },
    })
    .unwrap();
    let w = h.workspace();
    assert_eq!(w.terminals.len(), 1);
    assert_eq!(w.agent_sessions.len(), 2);
    assert_eq!(w.current_agent("one").unwrap().cli_session_id, "claude-two");
    assert_eq!(
        w.agent_sessions
            .iter()
            .find(|s| s.id == first_id)
            .unwrap()
            .outcome,
        Outcome::Completed
    );
    let db = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
    let names: Vec<String> = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for name in [
        "spaces",
        "tasks",
        "tabs",
        "terminals",
        "agent_sessions",
        "task_criteria",
    ] {
        assert!(names.iter().any(|n| n == name));
    }
    assert!(!names.iter().any(|n| n == "workspace"));
    let task: String = db
        .query_row(
            "SELECT task_id FROM agent_sessions WHERE cli_session_id='claude-two'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(task, "second-task");
    let evidence: Option<String> = db
        .query_row(
            "SELECT evidence FROM task_criteria WHERE task_id='first-task'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(evidence.is_none());
    drop(db);
    h.call(Request::Stop { id: "one".into() }).unwrap();
    h.child.kill().unwrap();
    h.child.wait().unwrap();
    h.child = fixture_command(&h.root)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if h.call(Request::Workspace).is_ok() {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(25));
    }
    let restored = h.workspace();
    assert_eq!(restored.agent_sessions.len(), 2);
    assert!(restored.agent_sessions.iter().all(|s| !s.current));
    assert_eq!(
        restored
            .agent_sessions
            .iter()
            .find(|s| s.id == first_id)
            .unwrap()
            .outcome,
        Outcome::Completed
    );
    assert_eq!(restored.tab_terminals("t"), ["one"]);
}

fn fixture_command(root: &std::path::Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--ignored", "--exact", "daemon_fixture", "--nocapture"])
        .env("KEEPLINE_FIXTURE_RUNTIME", root);
    command
}
#[test]
#[ignore = "subprocess daemon entry for integration fixtures"]
fn daemon_fixture() {
    let root = PathBuf::from(
        std::env::var_os("KEEPLINE_FIXTURE_RUNTIME").expect("fixture subprocess only"),
    );
    // Roots do not need to exist; production must also handle an uninstalled CLI.
    keeplined::serve_with_roots(
        &root,
        vec![
            (Agent::Codex, root.join("logs/codex")),
            (Agent::Claude, root.join("logs/claude")),
        ],
    )
    .unwrap();
}

#[test]
fn forty_idle_terminals_have_no_periodic_background_io() {
    let h = Host::measured();
    h.setup();
    let logs = h.root.join("logs/codex");
    fs::create_dir_all(&logs).unwrap();
    let cli = h.root.join("codex");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    // Same runtime and watched log directory, but outside every managed PTY.
    struct Outside(Child);
    impl Drop for Outside {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let external=Outside(Command::new(&cli).args(["-c", &format!(
        "exec 3>'{}'; printf '%s\\n' '{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"external\",\"cwd\":\"/tmp\"}}}}' >&3; read -r line",logs.join("external.jsonl").display())])
        .stdin(Stdio::piped()).spawn().unwrap());
    for i in 0..40 {
        let id = format!("idle-{i}");
        let script = format!(
            "exec 3>\"{}\"; printf \"%s\\n\" \"{{\\\"type\\\":\\\"session_meta\\\",\\\"payload\\\":{{\\\"id\\\":\\\"{id}\\\",\\\"cwd\\\":\\\"/tmp\\\"}}}}\" >&3; read -r line",
            logs.join(format!("{id}.jsonl")).display()
        );
        let command = format!("exec '{}' -c '{}'", cli.display(), script);
        h.call(h.launch(&id, &command)).unwrap();
    }
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let workspace = h.workspace();
        if workspace
            .agent_sessions
            .iter()
            .filter(|s| s.current)
            .count()
            == 40
        {
            assert!(workspace
                .agent_sessions
                .iter()
                .filter(|s| s.current)
                .all(|s| s.terminal.is_some() && s.cli_session_id != "external"));
            let mut processes = keeplined::recognition::processes(&workspace).unwrap();
            assert!(!processes.iter().any(|p| p.pid == external.0.id()));
            // Even if passed an unrelated Agent PID, the lsof boundary filters it out.
            processes.push(keeplined::recognition::Process {
                pid: external.0.id(),
                parent: std::process::id(),
                runtime: Some(Agent::Codex),
                cli_session_id: None,
                no_daemon: false,
            });
            assert!(
                !keeplined::recognition::open_transcripts(&workspace, &processes)
                    .unwrap()
                    .contains_key(&external.0.id())
            );
            break;
        }
        assert!(
            Instant::now() < until,
            "automatic subscription associated only {} of 40 sessions",
            workspace.agent_sessions.len()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let metrics_before = recognition(&h).metrics;
    // Let startup FSEvents batches and PTY setup finish before the measurement window.
    std::thread::sleep(Duration::from_secs(2));
    fn usage(pid: u32) -> libc::rusage_info_v2 {
        unsafe {
            let mut r: libc::rusage_info_v2 = std::mem::zeroed();
            assert_eq!(
                libc::proc_pid_rusage(
                    pid as i32,
                    libc::RUSAGE_INFO_V2,
                    (&mut r as *mut libc::rusage_info_v2).cast()
                ),
                0
            );
            r
        }
    }
    fn task(pid: u32) -> libc::proc_taskinfo {
        unsafe {
            let mut t: libc::proc_taskinfo = std::mem::zeroed();
            assert_eq!(
                libc::proc_pidinfo(
                    pid as i32,
                    libc::PROC_PIDTASKINFO,
                    0,
                    (&mut t as *mut libc::proc_taskinfo).cast(),
                    std::mem::size_of_val(&t) as i32
                ),
                std::mem::size_of_val(&t) as i32
            );
            t
        }
    }
    let before = usage(h.child.id());
    let calls = task(h.child.id());
    let start = Instant::now();
    std::thread::sleep(Duration::from_secs(5));
    let after = usage(h.child.id());
    let end_calls = task(h.child.id());
    let cpu_ticks =
        after.ri_user_time + after.ri_system_time - before.ri_user_time - before.ri_system_time;
    let cpu_percent = cpu_seconds(cpu_ticks) / start.elapsed().as_secs_f64() * 100.;
    eprintln!(
        "40 idle PTYs: CPU {cpu_percent:.4}%, disk read {} B, write {} B, Unix syscalls {}",
        after.ri_diskio_bytesread - before.ri_diskio_bytesread,
        after.ri_diskio_byteswritten - before.ri_diskio_byteswritten,
        end_calls.pti_syscalls_unix - calls.pti_syscalls_unix
    );
    assert_eq!(after.ri_diskio_bytesread, before.ri_diskio_bytesread);
    assert_eq!(after.ri_diskio_byteswritten, before.ri_diskio_byteswritten);
    // pti_syscalls_unix includes native FSEvents/libdispatch workqueue housekeeping,
    // not just file I/O. LLDB captured __workq_kernreturn after a filtered native
    // notification; parallel runs reproduced +2 with no application work. Allow
    // at most two such pairs, but keep our log/scan/index/probe counters EXACTLY zero.
    // We cannot retroactively name the user's two calls from an aggregate counter.
    let extra_calls = end_calls.pti_syscalls_unix - calls.pti_syscalls_unix;
    assert!(
        (0..=4).contains(&extra_calls),
        "unexpected idle Unix syscall delta {extra_calls}"
    );
    assert_eq!(
        recognition(&h).metrics,
        metrics_before,
        "idle must not read logs, walk directories, write the index or spawn probes"
    );
    assert!(cpu_percent < 0.5, "idle CPU {cpu_percent}% exceeds 0.5%");
    assert_eq!(h.workspace().terminals.len(), 40);
}

fn recognition(h: &Host) -> RecognitionStatus {
    match h.call(Request::RecognitionStatus).unwrap() {
        Reply::Recognition(s) => s,
        other => panic!("{other:?}"),
    }
}
fn usage(pid: u32) -> libc::rusage_info_v2 {
    unsafe {
        let mut r: libc::rusage_info_v2 = std::mem::zeroed();
        assert_eq!(
            libc::proc_pid_rusage(
                pid as i32,
                libc::RUSAGE_INFO_V2,
                (&mut r as *mut libc::rusage_info_v2).cast()
            ),
            0
        );
        r
    }
}
#[test]
fn four_agents_write_for_thirty_seconds_without_repeated_process_probes() {
    let h = Host::measured();
    h.setup();
    let control = Host::with_lease(FixtureLease::Companion);
    control.setup();
    // Average over a longer, bounded window under scheduler pressure. Writers
    // retain 10 Hz each and the budget remains 2% of one core, never a relaxed rate.
    let mut load = [0.; 1];
    assert_eq!(unsafe { libc::getloadavg(load.as_mut_ptr(), 1) }, 1);
    let cores = std::thread::available_parallelism().unwrap().get() as f64;
    let scale = (load[0] / cores).ceil().clamp(1., 3.) as u64;
    let window = 30 * scale;
    let logs = h.root.join("logs/codex");
    fs::create_dir_all(&logs).unwrap();
    let cli = h.root.join("codex");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    // Relative sleeps lose samples under load; keep a fixed workload on absolute deadlines.
    for i in 0..4 {
        let id = format!("load-{i}");
        let script = h.root.join(format!("{id}.sh"));
        fs::write(
            &script,
            format!(
                r#"exec 3>'{}'
printf '%s\n' '{{"type":"session_meta","payload":{{"id":"{id}","cwd":"/tmp"}}}}' >&3
read -r start
LOAD_LOG='{}' HOLD_OPEN='{}' /usr/bin/python3 - <<'PY'
import os,time
start=time.monotonic()
for sample in range({window}*10):
    data=b'{{"type":"event_msg","payload":{{"type":"agent_message","message":"working"}}}}\n'
    if os.environ['HOLD_OPEN']=='yes':
        os.write(3,data)
    else:
        with open(os.environ['LOAD_LOG'],'ab') as log:
            log.write(data)
    time.sleep(max(0,start+(sample+1)/10-time.monotonic()))
PY
printf done >'{}'
read -r finish
"#,
                logs.join(format!("{id}.jsonl")).display(),
                logs.join(format!("{id}.jsonl")).display(),
                if i < 2 { "yes" } else { "no" },
                h.root.join(format!("{id}.done")).display()
            ),
        )
        .unwrap();
        h.call(h.launch(
            &id,
            &format!("exec '{}' '{}'", cli.display(), script.display()),
        ))
        .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while h
        .workspace()
        .agent_sessions
        .iter()
        .filter(|s| s.current)
        .count()
        != 4
    {
        assert!(
            Instant::now() < deadline,
            "automatic association did not finish"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_secs(1));
    // Connection setup and teardown are not part of incremental log processing.
    // Keep all streams alive until after the sampled interval.
    let mut streams: Vec<_> = (0..4)
        .map(|i| h.attach(&format!("load-{i}"), true).0)
        .collect();
    let before = recognition(&h);
    let cpu = usage(h.child.id());
    let control_cpu = usage(control.child.id());
    let started = Instant::now();
    for stream in &mut streams {
        send(
            stream,
            CONTROL,
            &Request::Input {
                bytes: b"go\n".to_vec(),
            },
        )
        .unwrap();
        assert!(matches!(ack(stream), Reply::Ok));
    }
    // Sleep only the driver; neither daemon has a periodic sampler/timer.
    std::thread::sleep(Duration::from_secs(window + 1));
    let done_deadline = started + Duration::from_secs(window + 30);
    while (0..4).any(|i| !h.root.join(format!("load-{i}.done")).exists()) {
        assert!(
            Instant::now() < done_deadline,
            "writers did not finish under load"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    for i in 0..4 {
        assert!(h.root.join(format!("load-{i}.done")).exists());
        let lines = fs::read_to_string(logs.join(format!("load-{i}.jsonl")))
            .unwrap()
            .lines()
            .count();
        assert_eq!(lines as u64, 1 + window * 10, "writer {i} workload");
    }
    // Writer completion precedes the asynchronous file event. Include its final
    // consumption in the CPU sample, within the existing workload deadline.
    let db = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
    for i in 0..4 {
        let path = logs.join(format!("load-{i}.jsonl")).canonicalize().unwrap();
        let end = fs::metadata(&path).unwrap().len();
        let offset = loop {
            let offset: i64 = db
                .query_row(
                    "SELECT offset FROM log_files WHERE path=?1",
                    [path.to_str().unwrap()],
                    |r| r.get(0),
                )
                .unwrap();
            if offset as u64 == end || Instant::now() >= done_deadline {
                break offset;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(offset as u64, end, "subscriber cursor for writer {i}");
    }
    let after = usage(h.child.id());
    let control_after = usage(control.child.id());
    let stats = recognition(&h);
    let cpu_ticks =
        after.ri_user_time + after.ri_system_time - cpu.ri_user_time - cpu.ri_system_time;
    let seconds = cpu_seconds(cpu_ticks);
    let idle_seconds = cpu_seconds(
        control_after.ri_user_time + control_after.ri_system_time
            - control_cpu.ri_user_time
            - control_cpu.ri_system_time,
    );
    let incremental_seconds = (seconds - idle_seconds).max(0.);
    let percent = incremental_seconds / window as f64 * 100.;
    let ps = stats.metrics.ps_spawns - before.metrics.ps_spawns;
    let lsof = stats.metrics.lsof_spawns - before.metrics.lsof_spawns;
    eprintln!(
        "4 agents / {window}s (load {load:?}, {cores} cores): daemon CPU {seconds:.4}s, idle control {idle_seconds:.4}s, incremental {percent:.3}%, ps spawns {ps}, lsof spawns {lsof}, incremental bytes {}",
        stats.metrics.log_bytes - before.metrics.log_bytes
    );
    assert_eq!(
        (ps, lsof),
        (0, 0),
        "known transcript appends must not spawn probes"
    );
    assert!(
        idle_seconds <= 0.002 * window as f64,
        "idle control is not idle ({idle_seconds}s); do not subtract a contaminated control"
    );
    assert!(
        incremental_seconds <= 0.02 * window as f64,
        "incremental daemon CPU budget <=2% of one core, got {percent:.3}% ({seconds}s, idle {idle_seconds}s)"
    );
    assert!(
        stats.metrics.log_bytes - before.metrics.log_bytes > 80_000 * scale,
        "the subscriber must actually consume the workload"
    );
}
#[test]
fn recognition_status_stream_recovers_after_log_root_is_rebuilt() {
    let mut h = Host::start();
    let root = h.root.join("logs/codex");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("before.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"before\",\"cwd\":\"/tmp\"}}\n",
    )
    .unwrap();
    std::thread::sleep(Duration::from_secs(1));
    // Reopen with existing roots so this exercises an actual WatchRoot stream, not only its ancestor.
    h.child.kill().unwrap();
    h.child.wait().unwrap();
    h.child = fixture_command(&h.root)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if h.call(Request::Workspace).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut stream = UnixStream::connect(h.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR }).unwrap();
    receive::<Reply>(&mut stream).unwrap();
    send(&mut stream, CONTROL, &Request::WatchRecognition).unwrap();
    assert!(matches!(
        receive::<Reply>(&mut stream).unwrap(),
        Reply::Recognition(_)
    ));
    fs::rename(&root, h.root.join("moved-codex")).unwrap();
    std::thread::sleep(Duration::from_millis(400));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("after.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"after\",\"cwd\":\"/tmp\"}}\n",
    )
    .unwrap();
    let mut recovering = false;
    let mut interrupted = false;
    loop {
        let Reply::Recognition(status) = receive::<Reply>(&mut stream).unwrap() else {
            panic!("status frame required")
        };
        interrupted |= status.phase == RecognitionPhase::Interrupted;
        recovering |= status.phase == RecognitionPhase::Recovering;
        if interrupted && recovering && status.phase == RecognitionPhase::Healthy {
            break;
        }
    }
    // A fresh append after recovery is consumed without restarting the host.
    use std::io::Write;
    let path = root.join("after.jsonl");
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{}\n")
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let db = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
    loop {
        let offset: Option<i64> = db
            .query_row(
                "SELECT offset FROM log_files WHERE cli_id='after'",
                [],
                |r| r.get(0),
            )
            .ok();
        if offset == Some(fs::metadata(&path).unwrap().len() as i64) {
            assert_eq!(
                db.query_row(
                    "SELECT count(*) FROM log_files WHERE cli_id='before'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0,
                "missing old paths must be removed from the repaired subtree index"
            );
            break;
        }
        assert!(Instant::now() < deadline, "listener did not resume");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn stop_agent_clears_current_without_stopping_shell_or_changing_outcome() {
    let h = Host::start();
    h.setup();
    let cli = h.root.join("codex");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    let log = h.root.join("exit.jsonl");
    fs::write(
        &log,
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"exit-proof\",\"cwd\":\"/tmp\"}}\n",
    )
    .unwrap();
    let script = format!(
        "echo $$ > {}; read -r line",
        h.root.join("agent-pid").display()
    );
    h.call(h.launch(
        "shell",
        &format!(
            "{} -c {}; read -r line",
            shell_quote(&cli.to_string_lossy()),
            shell_quote(&script)
        ),
    ))
    .unwrap();
    let pid: u32 = wait_file(&h.root.join("agent-pid")).trim().parse().unwrap();
    h.call(Request::Hook {
        runtime: Agent::Codex,
        pid,
        cli_session_id: "exit-proof".into(),
        transcript: log.to_string_lossy().into_owned(),
        ended: false,
        event: None,
    })
    .unwrap();
    let before = h.workspace().current_agent("shell").unwrap().clone();
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::SetOutcome {
            id: before.id.clone(),
            outcome: Outcome::Completed,
        },
    })
    .unwrap();
    h.call(Request::StopAgent { id: "shell".into() }).unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let w = h.workspace();
        if w.current_agent("shell").is_none() {
            assert_eq!(w.terminals[0].state, ProcessState::Running);
            let session = &w.agent_sessions[0];
            assert_eq!(session.id, before.id);
            assert_eq!(session.task, before.task);
            assert_eq!(session.outcome, Outcome::Completed);
            let db = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
            let current: bool = db
                .query_row(
                    "SELECT is_current FROM agent_sessions WHERE id=?1",
                    [&before.id],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(!current);
            break;
        }
        assert!(Instant::now() < until, "Agent exit event was not persisted");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn manually_entered_unbound_codex_gets_a_hint_and_exit_clears_it() {
    let h = Host::start();
    h.setup();
    let cli = h.root.join("codex");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    h.call(h.launch(
        "shell",
        "stty -echo; while IFS= read -r line; do eval \"$line\"; done",
    ))
    .unwrap();
    let (mut stream, _, _) = h.attach("shell", false);
    let script = format!(
        "echo $$ > {}; read -r line",
        h.root.join("hint-pid").display()
    );
    send(
        &mut stream,
        CONTROL,
        &Request::Input {
            bytes: format!(
                "{} -c {}\n",
                shell_quote(&cli.to_string_lossy()),
                shell_quote(&script)
            )
            .into_bytes(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut stream), Reply::Ok));
    let pid: u32 = wait_file(&h.root.join("hint-pid")).trim().parse().unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if recognition(&h)
            .terminal_hints
            .get("shell")
            .is_some_and(|s| s == "未关联：看对话内容请用 codex --no-daemon")
        {
            break;
        }
        assert!(Instant::now() < until, "missing unbound Codex hint");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(h.workspace().agent_sessions.is_empty());
    assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGTERM) }, 0);
    let until = Instant::now() + Duration::from_secs(3);
    while !recognition(&h).terminal_hints.is_empty() {
        assert!(Instant::now() < until, "stale hint after process exit");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn input_control_follows_live_connections_without_takeover_or_polling() {
    let h = Host::start();
    h.setup();
    h.call(h.launch("control", "stty -echo; cat")).unwrap();
    let (first, _, writable) = h.attach("control", false);
    assert!(writable);
    let (mut second, _, writable) = h.attach("control", false);
    assert!(!writable, "a live owner must be respected");
    // Reads block until the disconnect event promotes this already-connected reader.
    drop(first);
    loop {
        let (kind, data) = read_frame(&mut second).unwrap();
        if kind == WRITABLE && serde_json::from_slice::<bool>(&data).unwrap() {
            break;
        }
    }
    send(
        &mut second,
        CONTROL,
        &Request::Input {
            bytes: b"promoted\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut second), Reply::Ok));
    drop(second);
    let (mut reopened, _, writable) = h.attach("control", false);
    if !writable {
        loop {
            let (kind, data) = read_frame(&mut reopened).unwrap();
            if kind == WRITABLE && serde_json::from_slice::<bool>(&data).unwrap() {
                break;
            }
        }
    }
    send(
        &mut reopened,
        CONTROL,
        &Request::Input {
            bytes: b"reopened\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut reopened), Reply::Ok));
}

#[test]
fn nested_split_tree_and_ratios_survive_store_reopen() {
    let mut h = Host::start();
    h.setup();
    for id in ["a", "b", "c", "d"] {
        h.call(h.launch(id, "sleep 30")).unwrap();
    }
    let mut layout = Layout::Leaf("a".into());
    layout.split("a", "b".into(), Axis::Right);
    layout.split("a", "c".into(), Axis::Down);
    layout.split("b", "d".into(), Axis::Down);
    if let Layout::Split { ratio, .. } = &mut layout {
        *ratio = 6300;
    }
    if let Some(Layout::Split { ratio, .. }) = layout.at_mut(&[true]) {
        *ratio = 2700;
    }
    let tab = Tab {
        id: "tree".into(),
        space: "s".into(),
        default_task: None,
        title: "Nested".into(),
        closed: false,
        layout: Some(layout.clone()),
    };
    h.call(Request::Edit {
        revision: h.workspace().revision,
        edit: Edit::PutTab { tab },
    })
    .unwrap();
    // Stop only this isolated fixture's processes before restarting its store.
    for id in ["a", "b", "c", "d"] {
        h.call(Request::Stop { id: id.into() }).unwrap();
    }
    h.child.kill().unwrap();
    h.child.wait().unwrap();
    h.child = fixture_command(&h.root)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    let workspace = loop {
        if let Ok(Reply::Workspace(w)) = h.call(Request::Workspace) {
            break w;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(workspace.tabs[0].layout, Some(layout));
    let db = rusqlite::Connection::open(h.root.join("workbench.sqlite")).unwrap();
    let nodes: u32 = db
        .query_row(
            "SELECT count(*) FROM tab_layout_nodes WHERE tab_id='tree'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(nodes, 7);
}

#[test]
fn split_directory_comes_from_the_live_shell_after_cd() {
    let h = Host::start();
    h.setup();
    let changed = h.root.join("changed-directory");
    fs::create_dir(&changed).unwrap();
    let ready = h.root.join("cwd-ready");
    h.call(h.launch(
        "cwd",
        &format!(
            "cd {}; printf ready > {}; read -r line",
            shell_quote(&changed.to_string_lossy()),
            shell_quote(&ready.to_string_lossy())
        ),
    ))
    .unwrap();
    wait_file(&ready);
    let Reply::Cwd(cwd) = h.call(Request::TerminalCwd { id: "cwd".into() }).unwrap() else {
        panic!("missing cwd")
    };
    assert_eq!(
        PathBuf::from(cwd).canonicalize().unwrap(),
        changed.canonicalize().unwrap()
    );
}

#[test]
fn stop_agent_refuses_an_unidentified_shell() {
    let h = Host::start();
    h.setup();
    h.call(h.launch("plain", "cat")).unwrap();
    assert!(h.call(Request::StopAgent { id: "plain".into() }).is_err());
    let terminal = h.workspace().terminals[0].clone();
    assert_eq!(terminal.state, ProcessState::Running);
    assert_eq!(unsafe { libc::kill(terminal.pid.unwrap() as i32, 0) }, 0);
}

#[test]
fn external_content_updates_without_probes_saves_full_quotes_and_reopens_on_new_turn() {
    let h = Host::start();
    h.setup();
    let logs = h.root.join("logs/codex");
    fs::create_dir_all(&logs).unwrap();
    let path = logs.join("content.jsonl");
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    fs::write(&path,format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"external\",\"cwd\":\"/tmp\"}}}}\n{{\"timestamp\":{at},\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\",\"turn_id\":\"one\"}}}}\n{{\"timestamp\":{at},\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"修复输入法，不要提交\"}}}}\n")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let w = loop {
        let w = h.workspace();
        if !w.content.is_empty() {
            break w;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    };
    let s = w.agent_sessions.first().unwrap();
    assert!(s.terminal.is_none());
    assert!(!s.current);
    assert_eq!(w.content[&s.id].asks[0].text, "修复输入法，不要提交");
    h.call(Request::Edit {
        revision: w.revision,
        edit: Edit::SaveSessionTask {
            session: s.id.clone(),
            task: Task {
                id: "from-session".into(),
                space: "s".into(),
                title: "输入法".into(),
                description: "UI truncated quote is not authoritative".into(),
                acceptance: vec![],
                agent: Agent::Codex,
            },
        },
    })
    .unwrap();
    let w = h.workspace();
    assert_eq!(w.tasks[0].description, "修复输入法，不要提交");
    assert_eq!(w.agent_sessions[0].task.as_deref(), Some("from-session"));
    h.call(Request::Edit {
        revision: w.revision,
        edit: Edit::SetOutcome {
            id: s.id.clone(),
            outcome: Outcome::Completed,
        },
    })
    .unwrap();
    use std::io::Write;
    writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(),"{{\"timestamp\":{},\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\",\"turn_id\":\"two\"}}}}",at+1).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if h.workspace().agent_sessions[0].outcome == Outcome::Open {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    let w = h.workspace();
    assert!(w
        .outcome_history
        .iter()
        .any(|e| e.reason == "新回合" && e.before == Outcome::Completed));
    let due = keepline_core::content::now() + 2;
    h.call(Request::Edit {
        revision: w.revision,
        edit: Edit::SetOutcome {
            id: s.id.clone(),
            outcome: Outcome::Deferred {
                reason: "deadline test".into(),
                until: due,
            },
        },
    })
    .unwrap();
    // Subscribe once: the host deadline must publish without a refresh/poll driving it.
    let mut stream = UnixStream::connect(h.socket()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    send(&mut stream, CONTROL, &Request::Hello { major: MAJOR }).unwrap();
    let _: Reply = receive(&mut stream).unwrap();
    send(&mut stream, CONTROL, &Request::WatchRecognition).unwrap();
    loop {
        let _: Reply = receive(&mut stream).unwrap();
        if keepline_core::content::now() >= due {
            break;
        }
    }
    let w = h.workspace();
    assert_eq!(w.agent_sessions[0].outcome, Outcome::Open);
    assert!(w
        .outcome_history
        .iter()
        .any(|e| e.reason == "暂存到期" && e.at == due));
    let stats = recognition(&h);
    assert_eq!((stats.metrics.ps_spawns, stats.metrics.lsof_spawns), (0, 0));
}

#[test]
fn claude_open_files_bind_same_cwd_without_hooks_and_append_without_probes() {
    use std::io::Write;
    let h = Host::start();
    h.setup();
    let logs = h.root.join("logs/claude");
    fs::create_dir_all(&logs).unwrap();
    let cli = h.root.join("claude");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    for i in 0..2 {
        let path = logs.join(format!("claude-{i}.jsonl"));
        let seed = h.root.join(format!("seed-{i}"));
        let row = serde_json::json!({"type":"user","sessionId":format!("claude-{i}"),"uuid":format!("u-{i}"),"cwd":"/project","entrypoint":"cli","userType":"external","timestamp":keepline_core::content::now(),"message":{"content":format!("修复第 {i} 个项目，不调用模型")}});
        fs::write(&seed, format!("{row}\n")).unwrap();
        let script = format!(
            "exec 3>{}; cat {} >&3; read -r line",
            shell_quote(&path.to_string_lossy()),
            shell_quote(&seed.to_string_lossy())
        );
        h.call(h.launch(
            &format!("claude-pty-{i}"),
            &format!(
                "exec {} -c {}",
                shell_quote(&cli.to_string_lossy()),
                shell_quote(&script)
            ),
        ))
        .unwrap();
    }
    // Wall-clock delivery may wait behind concurrent fixture startup and native
    // FSEvents dispatch. Keep a finite 30s bound; do not relax identity/content checks.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let w = h.workspace();
        if (0..2).all(|i| {
            w.current_agent(&format!("claude-pty-{i}"))
                .is_some_and(|s| s.cli_session_id == format!("claude-{i}"))
        }) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Claude automatic open-file binding timed out"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // Drain setup events before comparing append-only counters.
    std::thread::sleep(Duration::from_millis(500));
    let before = recognition(&h).metrics;
    let path = logs.join("claude-0.jsonl");
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    for row in [
        serde_json::json!({"type":"assistant","sessionId":"claude-0","message":{"content":[{"type":"tool_use","id":"edit","name":"Edit","input":{"file_path":"src/main.rs"}}]}}),
        serde_json::json!({"type":"user","sessionId":"claude-0","message":{"content":[{"type":"tool_result","tool_use_id":"edit","content":"Successfully updated"}]}}),
        serde_json::json!({"type":"assistant","sessionId":"claude-0","message":{"content":[{"type":"tool_use","id":"ask","name":"AskUserQuestion","input":{"questions":[{"question":"选择验证方式？","options":[{"label":"本地测试"},{"label":"人工复核"}]}]}}]}}),
    ] {
        writeln!(file, "{row}").unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let w = h.workspace();
        let agent = w.current_agent("claude-pty-0").unwrap();
        if w.content
            .get(&agent.id)
            .is_some_and(|c| c.turn == keepline_core::content::Turn::Waiting)
        {
            let c = &w.content[&agent.id];
            assert!(c.interactive);
            assert_eq!(c.asks[0].text, "修复第 0 个项目，不调用模型");
            assert!(c.here().done.contains("src/main.rs"));
            assert!(c.here().stopped.contains("选择验证方式"));
            assert!(c.here().stopped.contains("本地测试"));
            let other = w.current_agent("claude-pty-1").unwrap();
            assert!(!w.content[&other.id].here().done.contains("src/main.rs"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Claude append timed out: recognition={:?}, content={:?}, file_bytes={}",
            recognition(&h),
            w.content.get(&agent.id),
            fs::metadata(&path).unwrap().len()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let after = recognition(&h).metrics;
    assert_eq!(
        (before.ps_spawns, before.lsof_spawns),
        (after.ps_spawns, after.lsof_spawns)
    );
}

#[test]
fn large_unicode_input_survives_ack_disconnect_without_replay() {
    let h = Host::start();
    h.setup();
    let output = h.root.join("received");
    let ready = h.root.join("raw-ready");
    h.call(h.launch(
        "paste",
        &format!(
            "stty -echo -icanon; printf ready > {}; cat > {}",
            shell_quote(&ready.to_string_lossy()),
            shell_quote(&output.to_string_lossy())
        ),
    ))
    .unwrap();
    wait_file(&ready);
    let (mut first, _, writable) = h.attach("paste", false);
    assert!(writable);
    let bytes = "多行中文 日本語 한국어 e\u{301}\nsecond line\n"
        .repeat(2048)
        .into_bytes();
    send_input(&mut first, &bytes).unwrap();
    // Observe delivery independently, but deliberately never consume ACKs.
    // Closing before delivery could legitimately leave an unknown partial input.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if fs::metadata(&output).is_ok_and(|m| m.len() == bytes.len() as u64) {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    drop(first);
    let (_again, _, _) = h.attach("paste", true);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(fs::read(output).unwrap(), bytes);
}

#[test]
fn host_death_interrupts_pty_and_restart_preserves_no_fake_running_process() {
    let mut h = Host::start();
    h.setup();
    h.call(h.launch("dies", "exec cat")).unwrap();
    let pid = h.workspace().terminals[0].pid.unwrap();
    h.child.kill().unwrap();
    h.child.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while unsafe { libc::kill(pid as i32, 0) } == 0 {
        assert!(Instant::now() < deadline, "PTY process survived host death");
        std::thread::sleep(Duration::from_millis(25));
    }
    h.child = fixture_command(&h.root)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(Reply::Workspace(w)) = h.call(Request::Workspace) {
            assert_eq!(w.terminals[0].state, ProcessState::Interrupted);
            assert!(h.call(Request::Stop { id: "dies".into() }).is_err());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn claude_explicit_id_binds_closed_log_and_tracks_exit_without_hooks() {
    let h = Host::start();
    h.setup();
    let logs = h.root.join("logs/claude");
    fs::create_dir_all(&logs).unwrap();
    let cli = h.root.join("claude");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    let ids: Vec<_> = (0..2).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    for (i, id) in ids.iter().enumerate() {
        let path = logs.join(format!("{id}.jsonl"));
        let seed = h.root.join(format!("seed-{i}"));
        let row = serde_json::json!({"type":"user","sessionId":id,"uuid":format!("u{i}"),"cwd":"/project","entrypoint":"cli","userType":"external","timestamp":keepline_core::content::now(),"message":{"content":"Explicit identity test"}});
        fs::write(&seed, format!("{row}\n")).unwrap();
        // Close the log before FSEvents delivery: lsof cannot supply this association.
        let script = format!(
            "cat {} > {}; read -r line",
            shell_quote(&seed.to_string_lossy()),
            shell_quote(&path.to_string_lossy())
        );
        h.call(h.launch(
            &format!("id-{i}"),
            &format!(
                "exec {} -c {} fixture --session-id {}",
                shell_quote(&cli.to_string_lossy()),
                shell_quote(&script),
                id
            ),
        ))
        .unwrap();
    }
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let w = h.workspace();
        if ids.iter().enumerate().all(|(i, id)| {
            w.current_agent(&format!("id-{i}"))
                .is_some_and(|s| &s.cli_session_id == id)
        }) {
            break;
        }
        assert!(
            Instant::now() < until,
            "closed logs must bind via explicit argv IDs"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(recognition(&h).metrics.lsof_spawns, 0);
    h.call(Request::Stop { id: "id-0".into() }).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while h.workspace().current_agent("id-0").is_some() {
        assert!(
            Instant::now() < until,
            "exited Claude cannot remain current"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(h.workspace().current_agent("id-1").is_some());
}

#[test]
fn quiet_agent_screen_with_an_approval_waits_on_the_user_until_they_answer() {
    let h = Host::start();
    h.setup();
    let logs = h.root.join("logs/claude");
    fs::create_dir_all(&logs).unwrap();
    let cli = h.root.join("claude");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let path = logs.join(format!("{id}.jsonl"));
    let seed = h.root.join("seed");
    let row = serde_json::json!({"type":"user","sessionId":id,"uuid":"u0","cwd":"/project","entrypoint":"cli","userType":"external","timestamp":keepline_core::content::now(),"message":{"content":"Screen approval test"}});
    fs::write(&seed, format!("{row}\n")).unwrap();
    // The log never records the pending permission; only the screen shows it.
    let script = format!(
        "cat {} > {}; read -r go; printf '%s\\n' 'Bash command' '  git push origin main' 'Do you want to proceed?' '> 1. Yes' '  2. No'; read -r answer; printf '\\033[2J\\033[Hpushed\\n'; read -r end",
        shell_quote(&seed.to_string_lossy()),
        shell_quote(&path.to_string_lossy())
    );
    h.call(h.launch(
        "approval",
        &format!(
            "exec {} -c {} fixture --session-id {}",
            shell_quote(&cli.to_string_lossy()),
            shell_quote(&script),
            id
        ),
    ))
    .unwrap();
    let waiting = |w: &Workspace| {
        w.current_agent("approval")
            .and_then(|s| w.content.get(&s.id))
            .filter(|c| c.turn == keepline_core::content::Turn::Waiting)
            .map(|c| c.waiting.clone())
    };
    let until = Instant::now() + Duration::from_secs(30);
    while h.workspace().current_agent("approval").is_none() {
        assert!(Instant::now() < until, "explicit ID must bind");
        std::thread::sleep(Duration::from_millis(50));
    }
    let (mut stream, _vt, writable) = h.attach("approval", true);
    assert!(writable);
    send(
        &mut stream,
        CONTROL,
        &Request::Input {
            bytes: b"\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut stream), Reply::Ok));
    // The prompt follows the user's own keystroke, so it is only attributed to the agent
    // after the input settles, then once.
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(waiting(&h.workspace()), None);
    let until = Instant::now() + Duration::from_secs(10);
    let question = loop {
        if let Some(q) = waiting(&h.workspace()) {
            break q;
        }
        assert!(
            Instant::now() < until,
            "quiet approval screen must wait on the user"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(question, "Do you want to proceed?（画面上的提示）");
    send(
        &mut stream,
        CONTROL,
        &Request::Input {
            bytes: b"1\n".to_vec(),
        },
    )
    .unwrap();
    assert!(matches!(ack(&mut stream), Reply::Ok));
    assert_eq!(waiting(&h.workspace()), None, "answering clears it at once");
    // The answered prompt is gone from the screen, so the next quiet check stays clear.
    std::thread::sleep(Duration::from_millis(4500));
    assert_eq!(waiting(&h.workspace()), None);
    assert_eq!(recognition(&h).metrics.lsof_spawns, 0);
}

#[test]
fn manual_claude_closed_log_stays_external_and_append_does_not_repeat_probes() {
    use std::io::Write;
    let h = Host::start();
    h.setup();
    let logs = h.root.join("logs/claude");
    fs::create_dir_all(&logs).unwrap();
    let path = logs.join("manual.jsonl");
    let cli = h.root.join("claude");
    std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    let seed = h.root.join("seed");
    let id = uuid::Uuid::new_v4().to_string();
    let row = serde_json::json!({"type":"user","sessionId":id,"uuid":"u1","cwd":"/project","entrypoint":"cli","userType":"external","timestamp":keepline_core::content::now(),"message":{"content":"Manual identity needs open-file proof"}});
    fs::write(&seed, format!("{row}\n")).unwrap();
    let script = format!(
        "cat {} > {}; read -r line",
        shell_quote(&seed.to_string_lossy()),
        shell_quote(&path.to_string_lossy())
    );
    h.call(h.launch(
        "manual",
        &format!(
            "exec {} -c {}",
            shell_quote(&cli.to_string_lossy()),
            shell_quote(&script)
        ),
    ))
    .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if h.workspace()
            .agent_sessions
            .iter()
            .any(|s| s.cli_session_id == id)
            && recognition(&h).metrics.lsof_spawns > 0
        {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(500));
    let before = recognition(&h).metrics;
    for i in 0..5 {
        let row = serde_json::json!({"type":"assistant","sessionId":id,"uuid":format!("a{i}"),"message":{"content":[{"type":"text","text":format!("manual append {i}")}]}});
        // Match Claude's brief-open writes; an unbound file has no vnode watch.
        {
            let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(file, "{row}").unwrap();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let after = recognition(&h).metrics;
        if after.log_bytes > before.log_bytes {
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(50));
    }
    let after = recognition(&h).metrics;
    assert_eq!(
        (after.ps_spawns, after.lsof_spawns),
        (before.ps_spawns, before.lsof_spawns)
    );
    let w = h.workspace();
    assert!(w.current_agent("manual").is_none());
    assert!(w
        .agent_sessions
        .iter()
        .find(|s| s.cli_session_id == id)
        .unwrap()
        .terminal
        .is_none());
}

#[test]
fn socket_publication_waits_for_initialization_then_first_rpc_succeeds() {
    use std::os::unix::fs::DirBuilderExt;
    let lease = FixtureLease::Shared {
        _guard: FIXTURES.read().unwrap_or_else(|e| e.into_inner()),
    };
    let root = std::env::temp_dir().join(format!("kl-{}", uuid::Uuid::new_v4().simple()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let db = rusqlite::Connection::open(root.join("workbench.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE startup_barrier(id INTEGER); BEGIN EXCLUSIVE;")
        .unwrap();
    let child = fixture_command(&root)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let h = Host {
        child,
        root,
        _lease: lease,
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while !h.root.join("host.lock").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    // The daemon has entered initialization but cannot finish the database open.
    // The former code published the socket before this barrier and failed here.
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !h.socket().exists(),
        "a visible socket must mean ready, not merely bind called"
    );
    db.execute_batch("ROLLBACK").unwrap();
    while !h.socket().exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    // Exactly one connect/handshake/request; no retry can hide a readiness race.
    assert!(matches!(
        h.call(Request::Workspace).unwrap(),
        Reply::Workspace(_)
    ));
}

// Shared-daemon Codex stand-in: the TUI paints prompts (and a title) on its own PTY while a
// separate process keeps each thread's log open for appends, as the real daemon does. No argv
// ID and no open log in the Codex process itself.
struct DaemonStep {
    gate: Option<PathBuf>,
    prompt: String,
    thread: String,
}
fn daemon_codex(h: &Host, pane: &str, title: &str, steps: &[DaemonStep], extra: &str) {
    let logs = h.root.join("logs/codex");
    fs::create_dir_all(&logs).unwrap();
    let cli = h.root.join("codex");
    if !cli.exists() {
        std::os::unix::fs::symlink("/bin/sh", &cli).unwrap();
    }
    let q = |p: &std::path::Path| shell_quote(&p.to_string_lossy());
    let mut script = format!("printf '\\033]0;%s\\007' {}; ", shell_quote(title));
    let mut threads: Vec<&str> = vec![];
    for step in steps {
        if !threads.contains(&step.thread.as_str()) {
            threads.push(&step.thread);
        }
    }
    // Like the real daemon: each thread's log is created at TUI start with only its header,
    // held open by a separate non-Agent process, and prompts are appended later.
    for (j, thread) in threads.iter().enumerate() {
        let fifo = h.root.join(format!("{pane}-fifo-{j}"));
        assert!(std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        let meta = h.root.join(format!("{pane}-meta-{j}"));
        fs::write(
            &meta,
            format!(
                "{}\n",
                serde_json::json!({"type":"session_meta","payload":{"id":thread,"cwd":"/tmp"}})
            ),
        )
        .unwrap();
        script.push_str(&format!(
            "/bin/cat < {} >> {} & exec {}> {}; cat {} >&{}; ",
            q(&fifo),
            q(&logs.join(format!("{thread}.jsonl"))),
            3 + j,
            q(&fifo),
            q(&meta),
            3 + j
        ));
    }
    script.push_str("sleep 1.5; ");
    for (i, step) in steps.iter().enumerate() {
        let at = keepline_core::content::now();
        let seed_path = h.root.join(format!("{pane}-seed-{i}"));
        let prompt_path = h.root.join(format!("{pane}-prompt-{i}"));
        fs::write(
            &seed_path,
            format!(
                "{}\n{}\n",
                serde_json::json!({"timestamp":at,"type":"event_msg","payload":{"type":"task_started","turn_id":format!("{pane}-{i}")}}),
                serde_json::json!({"timestamp":at,"type":"event_msg","payload":{"type":"user_message","message":step.prompt}})
            ),
        )
        .unwrap();
        fs::write(&prompt_path, &step.prompt).unwrap();
        if let Some(gate) = &step.gate {
            script.push_str(&format!(
                "while [ ! -e {} ]; do sleep 0.02; done; ",
                q(gate)
            ));
        }
        let fd = 3 + threads.iter().position(|t| *t == step.thread).unwrap();
        script.push_str(&format!(
            "printf '› '; cat {}; printf '\\n'; cat {} >&{fd}; ",
            q(&prompt_path),
            q(&seed_path)
        ));
    }
    script.push_str("while read -r line; do :; done");
    h.call(h.launch(
        pane,
        &format!("exec {} {} -c {}", q(&cli), extra, shell_quote(&script)),
    ))
    .unwrap();
}
fn step(prompt: &str, thread: &str, gate: Option<&PathBuf>) -> DaemonStep {
    DaemonStep {
        gate: gate.cloned(),
        prompt: prompt.into(),
        thread: thread.into(),
    }
}
fn wait_for(h: &Host, what: &str, ok: impl Fn(&Workspace) -> bool) {
    let until = Instant::now() + Duration::from_secs(20);
    while !ok(&h.workspace()) {
        assert!(Instant::now() < until, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn current_is(w: &Workspace, pane: &str, thread: &str) -> bool {
    w.current_agent(pane)
        .is_some_and(|s| s.cli_session_id == thread)
}
fn ids<const N: usize>() -> [String; N] {
    std::array::from_fn(|_| uuid::Uuid::new_v4().to_string())
}

#[test]
fn daemon_codex_same_cwd_panes_bind_to_their_own_screen_and_exit_clears_current() {
    let h = Host::start();
    h.setup();
    let [a, b] = ids();
    daemon_codex(
        &h,
        "p0",
        "proj",
        &[step("请检查登录页跳转逻辑并修复回归问题", &a, None)],
        "",
    );
    daemon_codex(
        &h,
        "p1",
        "proj",
        &[step("给设置页补上深色模式的截图测试用例", &b, None)],
        "",
    );
    wait_for(
        &h,
        "each same-cwd thread binds to the pane showing its prompt",
        |w| current_is(w, "p0", &a) && current_is(w, "p1", &b),
    );
    assert!(recognition(&h).terminal_hints.is_empty());
    h.call(Request::Stop { id: "p0".into() }).unwrap();
    wait_for(&h, "a screen-bound Codex exit clears current", |w| {
        w.current_agent("p0").is_none() && current_is(w, "p1", &b)
    });
}

#[test]
fn daemon_codex_threads_started_in_the_same_millisecond_bind_correctly() {
    let h = Host::start();
    h.setup();
    let gate = h.root.join("go");
    let [a, b] = ids();
    daemon_codex(
        &h,
        "p0",
        "proj",
        &[step(
            "同一时刻启动的第一个线程：整理发布说明",
            &a,
            Some(&gate),
        )],
        "",
    );
    daemon_codex(
        &h,
        "p1",
        "proj",
        &[step(
            "同一时刻启动的第二个线程：修复构建脚本",
            &b,
            Some(&gate),
        )],
        "",
    );
    wait_for(&h, "both panes running before the shared start", |w| {
        w.terminals
            .iter()
            .filter(|t| t.state == ProcessState::Running)
            .count()
            == 2
    });
    fs::write(&gate, "").unwrap();
    wait_for(&h, "simultaneous threads bind to their own panes", |w| {
        current_is(w, "p0", &a) && current_is(w, "p1", &b)
    });
}

#[test]
fn daemon_codex_new_hands_the_pane_to_the_newer_thread_only() {
    let h = Host::start();
    h.setup();
    let gate = h.root.join("new");
    let [old, new] = ids();
    daemon_codex(
        &h,
        "p0",
        "proj",
        &[
            step("先排查通知点击后没有跳转的原因", &old, None),
            step("新会话：把通知投递日志整理成证据表格", &new, Some(&gate)),
        ],
        "",
    );
    // An idle Codex in the same cwd never received a prompt and must not take the /new thread.
    daemon_codex(&h, "idle", "proj", &[], "");
    wait_for(&h, "first thread binds", |w| current_is(w, "p0", &old));
    fs::write(&gate, "").unwrap();
    wait_for(&h, "/new thread takes over its own pane", |w| {
        current_is(w, "p0", &new)
            && w.agent_sessions
                .iter()
                .any(|s| s.cli_session_id == old && !s.current)
    });
    assert!(h.workspace().current_agent("idle").is_none());
}

#[test]
fn daemon_codex_identical_prompts_stay_unbound_until_a_distinct_follow_up() {
    let h = Host::start();
    h.setup();
    let gate = h.root.join("follow");
    let [a, b] = ids();
    let same = "请对比两个模型对这段代码的重构建议";
    daemon_codex(
        &h,
        "p0",
        "proj",
        &[
            step(same, &a, None),
            step("第一个窗格的追问：只保留性能相关建议", &a, Some(&gate)),
        ],
        "",
    );
    daemon_codex(
        &h,
        "p1",
        "proj",
        &[
            step(same, &b, None),
            step("第二个窗格的追问：只保留可读性相关建议", &b, Some(&gate)),
        ],
        "",
    );
    wait_for(&h, "both identical threads are indexed", |w| {
        [&a, &b]
            .iter()
            .all(|id| w.agent_sessions.iter().any(|s| &s.cli_session_id == *id))
    });
    std::thread::sleep(Duration::from_secs(2));
    let w = h.workspace();
    assert!(
        w.current_agent("p0").is_none() && w.current_agent("p1").is_none(),
        "identical prompts must not bind"
    );
    fs::write(&gate, "").unwrap();
    wait_for(
        &h,
        "distinct follow-ups bind each thread to its own pane",
        |w| current_is(w, "p0", &a) && current_is(w, "p1", &b),
    );
}

#[test]
fn unbound_daemon_codex_shows_title_state_and_no_daemon_shows_nothing() {
    let h = Host::start();
    h.setup();
    daemon_codex(&h, "busy", "⠋ proj", &[], "");
    daemon_codex(&h, "plain", "⠋ proj", &[], "--no-daemon");
    // Hints are computed after a submitted command; the fixture has none, so probe via input.
    for pane in ["busy", "plain"] {
        let (mut stream, _, _) = h.attach(pane, false);
        send(
            &mut stream,
            CONTROL,
            &Request::Input {
                bytes: b"\n".to_vec(),
            },
        )
        .unwrap();
        assert!(matches!(ack(&mut stream), Reply::Ok));
    }
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if recognition(&h)
            .terminal_hints
            .get("busy")
            .map(String::as_str)
            == Some("仅显示状态：工作中；看对话内容请用 codex --no-daemon")
        {
            break;
        }
        assert!(
            Instant::now() < until,
            "{:?}",
            recognition(&h).terminal_hints
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(!recognition(&h).terminal_hints.contains_key("plain"));
}
