use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

struct Daemon {
    child: Child,
    runtime: PathBuf,
    script: PathBuf,
    stderr: std::sync::Arc<std::sync::Mutex<String>>,
}

impl Daemon {
    fn start(script_body: &str) -> Self {
        Self::start_with(script_body, &[])
    }

    fn start_with(script_body: &str, env: &[(&str, &str)]) -> Self {
        Self::start_inner(script_body, env, None)
    }

    fn start_with_umask(script_body: &str, umask: &str) -> Self {
        assert!(
            !umask.is_empty() && umask.bytes().all(|byte| byte.is_ascii_digit()),
            "umask must be octal digits"
        );
        Self::start_inner(script_body, &[], Some(&format!("umask {umask}; ")))
    }

    fn start_with_ignored_tty_signals(script_body: &str) -> Self {
        Self::start_inner(
            script_body,
            &[],
            Some("trap '' INT QUIT TSTP TTIN TTOU WINCH; "),
        )
    }

    fn start_inner(script_body: &str, env: &[(&str, &str)], shell_prefix: Option<&str>) -> Self {
        static NEXT_RUNTIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT_RUNTIME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let runtime = PathBuf::from(format!("/tmp/kl{}-{id}", std::process::id()));
        fs::create_dir_all(&runtime).expect("runtime dir");
        let script = runtime.join("child.sh");
        fs::write(&script, script_body).expect("script");
        let bin = env!("CARGO_BIN_EXE_keeplined");
        let mut command = if let Some(prefix) = shell_prefix {
            let mut command = Command::new("/bin/sh");
            command
                .arg("-c")
                .arg(format!("{prefix}exec \"$0\" \"$@\""))
                .arg(bin)
                .args(["serve", "--runtime"])
                .arg(&runtime);
            command
        } else {
            let mut command = Command::new(bin);
            command.args(["serve", "--runtime"]).arg(&runtime);
            command
        };
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        for (key, value) in env {
            if *value == "@runtime" {
                command.env(key, &runtime);
            } else {
                command.env(key, value);
            }
        }
        let mut child = command.spawn().expect("spawn keeplined");
        let stderr = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let pipe = child.stderr.take().expect("stderr");
        let captured = std::sync::Arc::clone(&stderr);
        thread::spawn(move || {
            let mut reader = pipe;
            let mut text = String::new();
            let _ = reader.read_to_string(&mut text);
            *captured.lock().expect("stderr lock") = text;
        });
        let socket = runtime.join(keeplined::SOCKET_FILE_NAME);
        let ready = wait_until(Duration::from_secs(5), || socket.exists());
        if !ready {
            let _ = child.kill();
            let _ = child.wait();
            thread::sleep(Duration::from_millis(50));
            panic!(
                "keeplined did not create a socket: {}",
                stderr.lock().expect("stderr lock")
            );
        }
        wait_mode(&runtime, 0o700);
        wait_mode(&socket, 0o600);
        Self {
            child,
            runtime,
            script,
            stderr,
        }
    }

    fn stderr_text(&self) -> String {
        self.stderr.lock().expect("stderr lock").clone()
    }

    fn stop(&mut self) {
        if let Err(err) = self.child.kill() {
            eprintln!("test failed to stop keeplined: {err}");
        }
        if let Err(err) = self.child.wait() {
            eprintln!("test failed to reap keeplined: {err}");
        }
    }

    fn detach(mut self) -> (PathBuf, PathBuf) {
        self.stop();
        let runtime = self.runtime.clone();
        let script = self.script.clone();
        std::mem::forget(self);
        (runtime, script)
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
        let note = self.stderr_text();
        if !note.is_empty() {
            eprintln!("keeplined stderr: {note}");
        }
        for pid in pids_matching(&self.script.display().to_string()) {
            let status = Command::new("/bin/kill")
                .args(["-9", &pid.to_string()])
                .status();
            if let Err(err) = status {
                eprintln!("test failed to signal leftover child {pid}: {err}");
            }
        }
        if let Err(err) = fs::remove_dir_all(&self.runtime) {
            eprintln!("test failed to remove {}: {err}", self.runtime.display());
        }
    }
}

struct Client {
    stream: std::os::unix::net::UnixStream,
    next_id: u64,
}

impl Client {
    fn connect(runtime: &Path) -> Self {
        let socket = runtime.join(keeplined::SOCKET_FILE_NAME);
        let stream = std::os::unix::net::UnixStream::connect(&socket).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("write timeout");
        let mut client = Self { stream, next_id: 1 };
        let hello = client.call(json!({"op": "hello", "major": 0}));
        assert!(hello["ok"].as_bool().unwrap_or(false), "{hello}");
        assert_eq!(hello["major"], 0);
        assert_eq!(hello["result"]["experimental"], true);
        client
    }

    fn call(&mut self, mut body: Value) -> Value {
        let id = self.next_id.to_string();
        self.next_id += 1;
        body["id"] = json!(id);
        let bytes = serde_json::to_vec(&body).expect("encode");
        self.stream
            .write_all(&(bytes.len() as u32).to_be_bytes())
            .expect("len");
        self.stream.write_all(&bytes).expect("body");
        self.stream.flush().expect("flush");
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).expect("response len");
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf).expect("response");
        serde_json::from_slice(&buf).expect("response json")
    }
}

#[test]
fn managed_session_keeps_one_pty_and_one_writer() {
    let ticks = keeplined::DELTA_LIMIT + 4;
    let daemon = Daemon::start(&script(ticks));
    let mut owner = Client::connect(&daemon.runtime);
    let mut reader = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = owner.call(json!({
        "op": "launch",
        "operation_id": "op-main",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let launched = &launch["result"];
    assert_eq!(launched["spawned"], true);
    assert_eq!(launched["attachable"], true);
    let terminal_id = launched["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    let pid = launched["pid"].as_u64().expect("pid") as u32;
    assert!(pid_alive(pid), "child {pid} was not alive after launch");
    assert_eq!(
        pids_matching(&daemon.script.display().to_string()),
        vec![pid]
    );

    let intent_path = intent_path(&daemon.runtime, "op-main");
    let intent: Value = serde_json::from_str(&fs::read_to_string(&intent_path).expect("intent"))
        .expect("intent json");
    assert_eq!(intent["operation_id"], "op-main");
    assert_eq!(intent["state"], "running");
    assert_eq!(intent["pid"], pid);
    assert_eq!(intent["argv"][0], "/bin/sh");

    let left = wait_snapshot(&mut owner, &terminal_id);
    let right = reader.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(right["ok"].as_bool().unwrap_or(false), "{right}");
    assert_eq!(left["revision"], right["result"]["revision"]);
    assert_eq!(left["checksum"], right["result"]["checksum"]);
    assert_eq!(left["text"], right["result"]["text"]);
    assert!(
        left["attributed_cells"].as_u64().unwrap_or(0) >= 3,
        "{left}"
    );
    assert!(left["text"].as_str().unwrap_or("").contains("plain"));
    assert!(left["text"].as_str().unwrap_or("").contains("red"));
    let revision = left["revision"].as_u64().expect("revision");
    assert!(revision > keeplined::DELTA_LIMIT as u64);

    let gap = owner.call(json!({"op": "pull", "terminal_id": terminal_id, "after_revision": 0}));
    assert!(gap["ok"].as_bool().unwrap_or(false), "{gap}");
    assert_eq!(gap["result"]["resync_required"], true);
    assert!(gap["result"].get("deltas").is_none(), "{gap}");
    assert_eq!(gap["result"]["checksum"], left["checksum"]);

    let follow = owner.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": revision - 1,
    }));
    assert!(follow["ok"].as_bool().unwrap_or(false), "{follow}");
    assert_eq!(follow["result"]["resync_required"], false);
    let deltas = follow["result"]["deltas"].as_array().expect("deltas");
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0]["revision"], revision);
    assert_eq!(deltas[0]["checksum"], left["checksum"]);
    assert!(
        deltas[0]["text"].as_str().unwrap_or("").contains("READY"),
        "{follow}"
    );
    assert_eq!(follow["result"]["alive"], true);
    assert!(follow["result"]["exit_code"].is_null(), "{follow}");

    let replay = reader.call(json!({
        "op": "launch",
        "operation_id": "op-main",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(replay["ok"].as_bool().unwrap_or(false), "{replay}");
    assert_eq!(replay["result"]["spawned"], false);
    assert_eq!(replay["result"]["terminal_id"], terminal_id);
    assert_eq!(replay["result"]["pid"], pid);
    assert_eq!(
        pids_matching(&daemon.script.display().to_string()),
        vec![pid]
    );

    let conflict = owner.call(json!({
        "op": "launch",
        "operation_id": "op-main",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 30,
    }));
    assert_eq!(conflict["ok"], false);
    assert_eq!(conflict["error"]["code"], "operation_conflict");
    assert_eq!(
        pids_matching(&daemon.script.display().to_string()),
        vec![pid]
    );

    let rejected = reader.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "data": "REJECTEDTOKEN\n",
    }));
    assert_eq!(rejected["ok"], false);
    assert_eq!(rejected["error"]["code"], "lease_rejected");
    let after_reject = owner.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(!after_reject["result"]["text"]
        .as_str()
        .unwrap_or("")
        .contains("REJECTEDTOKEN"));

    let first = owner.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(first["ok"].as_bool().unwrap_or(false), "{first}");
    let first_generation = first["result"]["generation"].as_u64().expect("generation");
    let first_token = first["result"]["token"].as_str().expect("token").to_owned();

    let second = reader.call(json!({"op": "takeover", "terminal_id": terminal_id}));
    assert!(second["ok"].as_bool().unwrap_or(false), "{second}");
    let generation = second["result"]["generation"].as_u64().expect("generation");
    let token = second["result"]["token"]
        .as_str()
        .expect("token")
        .to_owned();
    assert!(generation > first_generation);

    let stale = owner.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": first_generation,
        "token": first_token,
        "data": "OLDTOKEN\n",
    }));
    assert_eq!(stale["ok"], false);
    assert_eq!(stale["error"]["code"], "lease_rejected");

    let accepted = reader.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": generation,
        "token": token,
        "data": "ping\n",
    }));
    assert!(accepted["ok"].as_bool().unwrap_or(false), "{accepted}");
    let pong = wait_text(&mut owner, &terminal_id, "pong");
    assert!(!pong["text"].as_str().unwrap_or("").contains("OLDTOKEN"));

    let blocked = owner.call(json!({
        "op": "resize",
        "terminal_id": terminal_id,
        "cols": 40,
        "rows": 12,
    }));
    assert_eq!(blocked["ok"], false);
    assert_eq!(blocked["error"]["code"], "lease_rejected");
    let unchanged = owner.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert_eq!(unchanged["result"]["cols"], 80);
    assert_eq!(unchanged["result"]["rows"], 24);

    let resized = reader.call(json!({
        "op": "resize",
        "terminal_id": terminal_id,
        "generation": generation,
        "token": token,
        "cols": 100,
        "rows": 30,
    }));
    assert!(resized["ok"].as_bool().unwrap_or(false), "{resized}");
    assert_eq!(resized["result"]["cols"], 100);
    assert_eq!(resized["result"]["rows"], 30);
    let resize_from = unchanged["result"]["revision"].as_u64().expect("revision");
    let pulled = owner.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": resize_from,
    }));
    assert!(pulled["ok"].as_bool().unwrap_or(false), "{pulled}");
    assert_eq!(pulled["result"]["resync_required"], false, "{pulled}");
    let resize_deltas = pulled["result"]["deltas"].as_array().expect("deltas");
    assert!(
        resize_deltas
            .iter()
            .any(|delta| delta["cols"] == 100 && delta["rows"] == 30),
        "{pulled}"
    );
    let _ = reader.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": generation,
        "token": token,
        "data": "size\n",
    }));
    let sized = wait_text(&mut owner, &terminal_id, "SIZE 30 100");
    assert!(
        sized["text"].as_str().unwrap_or("").contains("SIZE 30 100"),
        "{sized}"
    );

    drop(owner);
    drop(reader);
    thread::sleep(Duration::from_millis(100));
    assert!(pid_alive(pid), "child exited when clients disconnected");
    assert_eq!(
        pids_matching(&daemon.script.display().to_string()),
        vec![pid]
    );
    let mut again = Client::connect(&daemon.runtime);
    let reattached = again.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(reattached["ok"].as_bool().unwrap_or(false), "{reattached}");
    assert_eq!(reattached["result"]["pid"], pid);
    drop(again);

    let before = pids_matching(&daemon.script.display().to_string());
    // Restart is exercised by a second daemon below; this guard keeps the child observable.
    assert!(before.iter().all(|found| *found == pid));
}

#[test]
fn repeating_a_recorded_operation_does_not_spawn_again() {
    let daemon = Daemon::start(&script(1));
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-restart",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let pid = launch["result"]["pid"].as_u64().unwrap();
    drop(client);
    let (runtime, script) = daemon.detach();

    let socket = runtime.join(keeplined::SOCKET_FILE_NAME);
    if socket.exists() {
        fs::remove_file(&socket).expect("remove stale socket");
    }
    let mut restarted = Command::new(env!("CARGO_BIN_EXE_keeplined"))
        .args(["serve", "--runtime"])
        .arg(&runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("restart keeplined");
    let mut stderr_pipe = restarted.stderr.take().expect("restart stderr");
    let restart_log = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let captured = std::sync::Arc::clone(&restart_log);
    thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr_pipe.read_to_string(&mut text);
        *captured.lock().expect("restart log") = text;
    });
    let started = Instant::now();
    while std::os::unix::net::UnixStream::connect(&socket).is_err() {
        if started.elapsed() > Duration::from_secs(5) {
            let status = restarted.try_wait();
            thread::sleep(Duration::from_millis(50));
            panic!(
                "restarted daemon did not accept connections: status={status:?} stderr={}",
                restart_log.lock().expect("restart log")
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    let mut client = Client::connect(&runtime);
    let replay = client.call(json!({
        "op": "launch",
        "operation_id": "op-restart",
        "argv": ["/bin/sh", script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(replay["ok"].as_bool().unwrap_or(false), "{replay}");
    assert_eq!(replay["result"]["spawned"], false);
    assert_eq!(replay["result"]["attachable"], false);
    assert_eq!(replay["result"]["terminal_id"], terminal_id);
    assert_eq!(replay["result"]["pid"], pid);
    let pids = pids_matching(&script.display().to_string());
    assert!(
        pids.iter().all(|found| u64::from(*found) == pid),
        "{pids:?}"
    );
    let attach = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert_eq!(attach["ok"], false);
    assert_eq!(attach["error"]["code"], "not_attachable");
    if let Err(err) = restarted.kill() {
        eprintln!("failed to stop restarted daemon: {err}");
    }
    if let Err(err) = restarted.wait() {
        eprintln!("failed to reap restarted daemon: {err}");
    }
    for found in pids_matching(&script.display().to_string()) {
        let _ = Command::new("/bin/kill")
            .args(["-9", &found.to_string()])
            .status();
    }
    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn failed_spawn_is_recorded_and_not_retried() {
    let daemon = Daemon::start("#!/bin/sh\nexit 0\n");
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let missing = daemon.runtime.join("missing-binary");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-missing",
        "argv": [missing],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(launch["ok"], false, "{launch}");
    assert_eq!(launch["error"]["code"], "spawn_failed");
    let intent: Value = serde_json::from_str(
        &fs::read_to_string(intent_path(&daemon.runtime, "op-missing")).unwrap(),
    )
    .unwrap();
    assert_eq!(intent["state"], "failed");
    let again = client.call(json!({
        "op": "launch",
        "operation_id": "op-missing",
        "argv": [missing],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(again["ok"], false);
    assert_eq!(again["error"]["code"], "spawn_failed");
}

#[test]
fn unsupported_protocol_major_is_rejected() {
    let daemon = Daemon::start("#!/bin/sh\nexit 0\n");
    let socket = daemon.runtime.join(keeplined::SOCKET_FILE_NAME);
    let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let body = serde_json::to_vec(&json!({"id": "bad", "op": "hello", "major": 1})).unwrap();
    stream
        .write_all(&(body.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(&body).unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let mut buf = vec![0u8; u32::from_be_bytes(len_buf) as usize];
    stream.read_exact(&mut buf).unwrap();
    let response: Value = serde_json::from_slice(&buf).unwrap();
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "unsupported_protocol");
    assert_eq!(response["major"], 0);
}

#[test]
fn second_serve_does_not_replace_a_live_socket() {
    let daemon = Daemon::start(&script(1));
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-lock",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;
    let terminal_id = launch["result"]["terminal_id"]
        .as_str()
        .expect("terminal")
        .to_owned();
    let socket = daemon.runtime.join(keeplined::SOCKET_FILE_NAME);
    let inode = fs::metadata(&socket).expect("socket").ino();

    let mut second = Command::new(env!("CARGO_BIN_EXE_keeplined"))
        .args(["serve", "--runtime"])
        .arg(&daemon.runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("second serve");
    let mut stderr_pipe = second.stderr.take().expect("second stderr");
    let started = Instant::now();
    let status = loop {
        match second.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > Duration::from_secs(5) => {
                let _ = second.kill();
                let _ = second.wait();
                panic!("second serve stayed up");
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(err) => panic!("wait second serve: {err}"),
        }
    };
    let mut stderr = String::new();
    stderr_pipe
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert!(!status.success(), "{stderr}");
    assert!(
        stderr.contains("already_running"),
        "second serve stderr was {stderr}"
    );
    assert_eq!(fs::metadata(&socket).expect("socket").ino(), inode);
    assert!(pid_alive(pid), "second serve disturbed the child");
    let attached = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(attached["ok"].as_bool().unwrap_or(false), "{attached}");
    assert_eq!(attached["result"]["pid"], pid);
}

#[test]
fn stale_socket_is_replaced_when_no_peer_accepts() {
    let daemon = Daemon::start("#!/bin/sh\nexit 0\n");
    let (runtime, _script) = daemon.detach();
    let socket = runtime.join(keeplined::SOCKET_FILE_NAME);
    assert!(socket.exists(), "stopped daemon left no socket");
    let mut restarted = Command::new(env!("CARGO_BIN_EXE_keeplined"))
        .args(["serve", "--runtime"])
        .arg(&runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("replace stale socket");
    let mut stderr_pipe = restarted.stderr.take().expect("stderr");
    let log = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let captured = std::sync::Arc::clone(&log);
    thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr_pipe.read_to_string(&mut text);
        *captured.lock().expect("log") = text;
    });
    let started = Instant::now();
    while std::os::unix::net::UnixStream::connect(&socket).is_err() {
        if started.elapsed() > Duration::from_secs(5) {
            let _ = restarted.kill();
            let _ = restarted.wait();
            panic!(
                "stale socket was not replaced: {}",
                log.lock().expect("log")
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
    drop(Client::connect(&runtime));
    if let Err(err) = restarted.kill() {
        eprintln!("failed to stop replacement daemon: {err}");
    }
    if let Err(err) = restarted.wait() {
        eprintln!("failed to reap replacement daemon: {err}");
    }
    let _ = fs::remove_dir_all(&runtime);
}

const TTY_CHILD_SCRIPT: &str = r#"#!/bin/sh
printf 'SID %s\n' "$(/usr/bin/python3 -c 'import os; print(os.getsid(0))')"
printf 'PGID %s\n' "$(/usr/bin/python3 -c 'import os; print(os.getpgrp())')"
trap 'printf WINCH\n' WINCH
trap 'printf INT\n' INT
printf 'READY\n'
i=0
while [ "$i" -lt 40 ]; do
  sleep 0.2
  i=$((i + 1))
done
"#;

#[test]
fn child_session_receives_interrupt_and_sigwinch() {
    assert_child_receives_interrupt_and_sigwinch(Daemon::start(TTY_CHILD_SCRIPT));
}

#[test]
fn ignored_terminal_signals_still_reach_the_child() {
    assert_child_receives_interrupt_and_sigwinch(Daemon::start_with_ignored_tty_signals(
        TTY_CHILD_SCRIPT,
    ));
}

fn assert_child_receives_interrupt_and_sigwinch(daemon: Daemon) {
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-tty",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let snap = wait_text(&mut client, &terminal_id, "READY");
    let text = snap["text"].as_str().unwrap_or("");
    assert!(
        text.contains(&format!("SID {pid}")) && text.contains(&format!("PGID {pid}")),
        "{text}"
    );
    let (pgid, tpgid, tty, flags) = foreground_tty(pid);
    assert_eq!(pgid, pid, "child is not its own process group");
    assert_eq!(tpgid, pid, "child is not the foreground group");
    assert_ne!(tty, "??", "child has no controlling terminal");
    assert_ne!(flags & 0x2, 0, "P_CONTROLT missing from flags {flags:x}");

    let lease = client.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(lease["ok"].as_bool().unwrap_or(false), "{lease}");
    let generation = lease["result"]["generation"].as_u64().unwrap();
    let token = lease["result"]["token"].as_str().unwrap().to_owned();
    let resized = client.call(json!({
        "op": "resize",
        "terminal_id": terminal_id,
        "generation": generation,
        "token": token,
        "cols": 90,
        "rows": 20,
    }));
    assert!(resized["ok"].as_bool().unwrap_or(false), "{resized}");
    let winch = wait_text(&mut client, &terminal_id, "WINCH");
    assert!(winch["text"].as_str().unwrap_or("").contains("WINCH"));
    let interrupted = client.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": generation,
        "token": token,
        "data": "\u{0003}",
    }));
    assert!(
        interrupted["ok"].as_bool().unwrap_or(false),
        "{interrupted}"
    );
    let int_text = wait_text(&mut client, &terminal_id, "INT");
    assert!(int_text["text"].as_str().unwrap_or("").contains("INT"));
}

#[test]
fn contiguous_pull_returns_the_grid_text() {
    let daemon = Daemon::start(
        "#!/bin/sh\nprintf 'alpha\\n'\nsleep 0.1\nprintf 'beta\\n'\nsleep 0.1\nprintf 'READY\\n'\nsleep 30\n",
    );
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-pull",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let snap = wait_text(&mut client, &terminal_id, "READY");
    let revision = snap["revision"].as_u64().expect("revision");
    assert!(revision >= 1, "{snap}");
    let follow = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": revision - 1,
    }));
    assert!(follow["ok"].as_bool().unwrap_or(false), "{follow}");
    assert_eq!(follow["result"]["resync_required"], false);
    assert_eq!(follow["result"]["alive"], true);
    assert!(follow["result"]["exit_code"].is_null(), "{follow}");
    let deltas = follow["result"]["deltas"].as_array().expect("deltas");
    assert_eq!(deltas.len(), 1, "{follow}");
    assert_eq!(deltas[0]["text"], snap["text"]);
    assert_eq!(deltas[0]["checksum"], snap["checksum"]);
    assert_eq!(deltas[0]["cols"], 80);
    assert_eq!(deltas[0]["rows"], 24);
}

#[test]
fn child_winsize_change_keeps_the_requested_payload() {
    let daemon = Daemon::start_with(
        "#!/bin/sh\nsleep 30\n",
        &[("KEEPLINED_TEST_POST_SPAWN_WINSIZE", "100x40")],
    );
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-winsize",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    assert_eq!(launch["result"]["cols"], 100);
    assert_eq!(launch["result"]["rows"], 40);
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let intent: Value = serde_json::from_str(
        &fs::read_to_string(intent_path(&daemon.runtime, "op-winsize")).unwrap(),
    )
    .unwrap();
    assert_eq!(intent["cols"], 80, "{intent}");
    assert_eq!(intent["rows"], 24, "{intent}");
    drop(client);
    let (runtime, script) = daemon.detach();
    let socket = runtime.join(keeplined::SOCKET_FILE_NAME);
    if socket.exists() {
        fs::remove_file(&socket).expect("remove stale socket");
    }
    let mut restarted = Command::new(env!("CARGO_BIN_EXE_keeplined"))
        .args(["serve", "--runtime"])
        .arg(&runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("restart keeplined");
    assert!(
        wait_until(Duration::from_secs(5), || {
            std::os::unix::net::UnixStream::connect(&socket).is_ok()
        }),
        "restarted daemon did not accept connections: {:?}",
        restarted.try_wait()
    );
    let mut client = Client::connect(&runtime);
    let replay = client.call(json!({
        "op": "launch",
        "operation_id": "op-winsize",
        "argv": ["/bin/sh", script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(replay["ok"].as_bool().unwrap_or(false), "{replay}");
    assert_eq!(replay["result"]["spawned"], false);
    assert_eq!(replay["result"]["attachable"], false);
    assert_eq!(replay["result"]["terminal_id"], terminal_id);
    if let Err(err) = restarted.kill() {
        eprintln!("failed to stop restarted daemon: {err}");
    }
    if let Err(err) = restarted.wait() {
        eprintln!("failed to reap restarted daemon: {err}");
    }
    for found in pids_matching(&script.display().to_string()) {
        let _ = Command::new("/bin/kill")
            .args(["-9", &found.to_string()])
            .status();
    }
    let _ = fs::remove_dir_all(&runtime);
}

#[test]
fn out_of_range_winsize_readback_fails_the_spawn() {
    let daemon = Daemon::start_with(
        "#!/bin/sh\nsleep 30\n",
        &[("KEEPLINED_TEST_POST_SPAWN_WINSIZE", "65535x65535")],
    );
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-huge",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(launch["ok"], false, "{launch}");
    assert_eq!(launch["error"]["code"], "spawn_failed");
    assert!(
        launch["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("65535x65535"),
        "{launch}"
    );
    let intent: Value =
        serde_json::from_str(&fs::read_to_string(intent_path(&daemon.runtime, "op-huge")).unwrap())
            .unwrap();
    assert_eq!(intent["state"], "failed", "{intent}");
    assert_eq!(intent["cols"], 80, "{intent}");
    assert_eq!(intent["rows"], 24, "{intent}");
    assert!(
        pids_matching(&daemon.script.display().to_string()).is_empty(),
        "out-of-range winsize left a child"
    );
    let again = client.call(json!({
        "op": "launch",
        "operation_id": "op-huge",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(again["ok"], false, "{again}");
    assert_eq!(again["error"]["code"], "spawn_failed");
}

#[test]
fn out_of_range_resize_readback_keeps_the_previous_screen() {
    let daemon = Daemon::start_with(
        "#!/bin/sh\nsleep 30\n",
        &[("KEEPLINED_TEST_RESIZE_WINSIZE", "65535x65535")],
    );
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-resize-huge",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    assert_eq!(launch["result"]["cols"], 80, "{launch}");
    assert_eq!(launch["result"]["rows"], 24, "{launch}");
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let before = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(before["ok"].as_bool().unwrap_or(false), "{before}");
    assert_eq!(before["result"]["cols"], 80, "{before}");
    assert_eq!(before["result"]["rows"], 24, "{before}");
    let lines_before = before["result"]["text"]
        .as_str()
        .unwrap_or("")
        .matches('\n')
        .count();
    let revision = before["result"]["revision"].as_u64().expect("revision");
    let lease = client.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(lease["ok"].as_bool().unwrap_or(false), "{lease}");
    let resized = client.call(json!({
        "op": "resize",
        "terminal_id": terminal_id,
        "generation": lease["result"]["generation"],
        "token": lease["result"]["token"],
        "cols": 100,
        "rows": 40,
    }));
    assert_eq!(resized["ok"], false, "{resized}");
    assert_eq!(resized["error"]["code"], "geometry_rejected", "{resized}");
    assert!(
        resized["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("65535x65535"),
        "{resized}"
    );
    let after = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(after["ok"].as_bool().unwrap_or(false), "{after}");
    assert_eq!(after["result"]["cols"], 80, "{after}");
    assert_eq!(after["result"]["rows"], 24, "{after}");
    assert_eq!(after["result"]["revision"], revision, "{after}");
    assert_eq!(
        after["result"]["text"]
            .as_str()
            .unwrap_or("")
            .matches('\n')
            .count(),
        lines_before,
        "{after}"
    );
    assert!(pid_alive(pid), "resize rejection stopped the child");
}

#[test]
fn query_replies_do_not_stall_other_sessions() {
    let daemon = Daemon::start("#!/bin/sh\nsleep 30\n");
    let py = daemon.runtime.join("spam.py");
    fs::write(
        &py,
        "import os, time\nos.write(1, b'\\x1b[6n' * 200000)\ntime.sleep(60)\n",
    )
    .unwrap();
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-spam",
        "argv": ["/usr/bin/python3", py],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let started = Instant::now();
    loop {
        let response = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
        assert!(response["ok"].as_bool().unwrap_or(false), "{response}");
        if response["result"]["revision"].as_u64().unwrap_or(0) >= 1 {
            break;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("spam child produced no revision: {response}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(200));
    let still = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(still["ok"].as_bool().unwrap_or(false), "{still}");
    let other = client.call(json!({
        "op": "launch",
        "operation_id": "op-other",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(other["ok"].as_bool().unwrap_or(false), "{other}");
    assert_eq!(other["result"]["spawned"], true);
}

#[test]
fn one_read_with_several_newlines_is_one_revision() {
    let daemon = Daemon::start(
        r#"#!/bin/sh
stty -echo
printf 'READY\n'
while IFS= read -r line; do
  case "$line" in
    burst)
      printf 'L1\nL2\nL3\n'
      ;;
  esac
done
"#,
    );
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-burst",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let before = wait_text(&mut client, &terminal_id, "READY");
    let revision = before["revision"].as_u64().expect("revision");
    let lease = client.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(lease["ok"].as_bool().unwrap_or(false), "{lease}");
    let sent = client.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": lease["result"]["generation"],
        "token": lease["result"]["token"],
        "data": "burst\n",
    }));
    assert!(sent["ok"].as_bool().unwrap_or(false), "{sent}");
    let after = wait_text(&mut client, &terminal_id, "L3");
    assert!(
        after["text"].as_str().unwrap_or("").contains("L1"),
        "{after}"
    );
    assert!(
        after["text"].as_str().unwrap_or("").contains("L2"),
        "{after}"
    );
    assert_eq!(after["revision"].as_u64(), Some(revision + 1), "{after}");
}

#[test]
fn exit_is_visible_without_a_new_revision() {
    let daemon = Daemon::start(
        r#"#!/bin/sh
stty -echo
printf 'READY\n'
while IFS= read -r line; do
  case "$line" in
    quit) exit 4 ;;
  esac
done
"#,
    );
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-exit",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let snap = wait_text(&mut client, &terminal_id, "READY");
    let revision = snap["revision"].as_u64().expect("revision");
    let current = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": revision,
    }));
    assert!(current["ok"].as_bool().unwrap_or(false), "{current}");
    assert_eq!(current["result"]["alive"], true);
    assert!(current["result"]["exit_code"].is_null(), "{current}");
    assert_eq!(current["result"]["deltas"].as_array().unwrap().len(), 0);
    let lease = client.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(lease["ok"].as_bool().unwrap_or(false), "{lease}");
    let quit = client.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": lease["result"]["generation"],
        "token": lease["result"]["token"],
        "data": "quit\n",
    }));
    assert!(quit["ok"].as_bool().unwrap_or(false), "{quit}");
    let started = Instant::now();
    let exited = loop {
        let pull = client.call(json!({
            "op": "pull",
            "terminal_id": terminal_id,
            "after_revision": revision,
        }));
        assert!(pull["ok"].as_bool().unwrap_or(false), "{pull}");
        if pull["result"]["alive"] == false {
            break pull;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("child stayed alive at revision {revision}: {pull}");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(exited["result"]["revision"], revision);
    assert_eq!(exited["result"]["exit_code"], 4);
    assert_eq!(exited["result"]["resync_required"], false);
    assert_eq!(exited["result"]["deltas"].as_array().unwrap().len(), 0);
}

#[test]
fn exit_is_published_only_after_the_pty_is_drained() {
    let hold = std::env::temp_dir().join(format!(
        "keeplined-hold-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _ = fs::remove_file(&hold);
    let release = hold.with_extension("release");
    let _ = fs::remove_file(&release);
    let daemon = Daemon::start_with(
        "#!/bin/sh\nprintf 'END\\n'\n",
        &[(
            "KEEPLINED_TEST_HOLD_READER",
            hold.to_str().expect("hold path"),
        )],
    );
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-drain",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    assert!(
        wait_until(Duration::from_secs(5), || hold.exists()),
        "reader did not pause: {}",
        daemon.stderr_text()
    );
    let mid = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(mid["ok"].as_bool().unwrap_or(false), "{mid}");
    assert_eq!(mid["result"]["alive"], true, "{mid}");
    assert!(
        !mid["result"]["text"].as_str().unwrap_or("").contains("END"),
        "{mid}"
    );
    fs::write(&release, b"go").expect("release");
    let started = Instant::now();
    let done = loop {
        let snap = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
        assert!(snap["ok"].as_bool().unwrap_or(false), "{snap}");
        if snap["result"]["alive"] == false {
            break snap;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("exit was not published after the reader resumed: {snap}");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(
        done["result"]["text"]
            .as_str()
            .unwrap_or("")
            .contains("END"),
        "{done}"
    );
    let _ = fs::remove_file(&hold);
    let _ = fs::remove_file(&release);
}

#[test]
fn exit_is_not_visible_until_the_final_chunk_is_ingested() {
    let daemon = Daemon::start_with(
        "#!/bin/sh\nsleep 30\n",
        &[("KEEPLINED_TEST_PAUSE_AFTER_READ", "@runtime")],
    );
    let py = daemon.runtime.join("burst.py");
    fs::write(
        &py,
        r#"import os, sys, time
runtime = sys.argv[1]
os.write(1, b"Q" * 8192 + b"\nREADY\n")
arm = os.path.join(runtime, "arm")
while not os.path.exists(arm):
    time.sleep(0.02)
os.write(1, b"TAIL\n")
paused = os.path.join(runtime, "paused")
while not os.path.exists(paused):
    time.sleep(0.02)
os._exit(6)
"#,
    )
    .expect("burst script");
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-final-chunk",
        "argv": ["/usr/bin/python3", py, daemon.runtime],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;
    let ready = wait_text(&mut client, &terminal_id, "READY");
    let revision = ready["revision"].as_u64().expect("revision");
    assert!(
        revision >= 2,
        "a write larger than the read buffer collapsed into revision {revision}"
    );
    assert!(
        !ready["text"].as_str().unwrap_or("").contains("TAIL"),
        "{ready}"
    );
    fs::write(daemon.runtime.join("arm"), "1").expect("arm");
    assert!(
        wait_until(Duration::from_secs(5), || {
            daemon.runtime.join("paused").exists()
        }),
        "reader did not pause on the final chunk: {}",
        daemon.stderr_text()
    );
    assert!(
        wait_until(Duration::from_secs(5), || child_has_exited(pid)),
        "child was still running after the final read was held, stat={:?}",
        process_stat(pid)
    );
    let held = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": revision,
    }));
    assert!(held["ok"].as_bool().unwrap_or(false), "{held}");
    assert_eq!(held["result"]["alive"], true, "{held}");
    assert!(held["result"]["exit_code"].is_null(), "{held}");
    assert_eq!(held["result"]["revision"], revision, "{held}");
    assert_eq!(
        held["result"]["deltas"].as_array().unwrap().len(),
        0,
        "{held}"
    );
    let snap = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(snap["ok"].as_bool().unwrap_or(false), "{snap}");
    let held_text = snap["result"]["text"].as_str().unwrap_or("");
    assert!(held_text.contains("READY"), "{snap}");
    assert!(!held_text.contains("TAIL"), "{snap}");
    assert_eq!(snap["result"]["alive"], true, "{snap}");

    fs::write(daemon.runtime.join("resume"), "1").expect("resume");
    let started = Instant::now();
    let exited = loop {
        let pull = client.call(json!({
            "op": "pull",
            "terminal_id": terminal_id,
            "after_revision": revision,
        }));
        assert!(pull["ok"].as_bool().unwrap_or(false), "{pull}");
        if pull["result"]["alive"] == false {
            break pull;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("child exit was not published: {pull}");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let deltas = exited["result"]["deltas"].as_array().expect("deltas");
    assert!(
        deltas
            .iter()
            .any(|delta| delta["text"].as_str().unwrap_or("").contains("TAIL")),
        "first exited pull missed the final grid: {exited}"
    );
    assert_eq!(exited["result"]["exit_code"], 6, "{exited}");
    assert_eq!(exited["result"]["resync_required"], false, "{exited}");
    let final_revision = exited["result"]["revision"].as_u64().expect("revision");
    assert!(final_revision > revision, "{exited}");
    let again = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": final_revision,
    }));
    assert!(again["ok"].as_bool().unwrap_or(false), "{again}");
    assert_eq!(again["result"]["alive"], false, "{again}");
    assert_eq!(again["result"]["exit_code"], 6, "{again}");
    assert_eq!(again["result"]["revision"], final_revision, "{again}");
    assert_eq!(again["result"]["resync_required"], false, "{again}");
    assert_eq!(
        again["result"]["deltas"].as_array().unwrap().len(),
        0,
        "{again}"
    );
}

#[test]
fn resize_after_exit_leaves_the_grid_unchanged() {
    let daemon = Daemon::start(
        r#"#!/bin/sh
stty -echo
printf 'READY\n'
while IFS= read -r line; do
  case "$line" in
    quit) exit 4 ;;
  esac
done
"#,
    );
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-resize-exit",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    wait_text(&mut client, &terminal_id, "READY");
    let lease = client.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(lease["ok"].as_bool().unwrap_or(false), "{lease}");
    let quit = client.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": lease["result"]["generation"],
        "token": lease["result"]["token"],
        "data": "quit\n",
    }));
    assert!(quit["ok"].as_bool().unwrap_or(false), "{quit}");
    let started = Instant::now();
    let exited = loop {
        let snap = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
        assert!(snap["ok"].as_bool().unwrap_or(false), "{snap}");
        if snap["result"]["alive"] == false {
            break snap;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("child stayed alive: {snap}");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let revision = exited["result"]["revision"].clone();
    let text = exited["result"]["text"].clone();
    let resized = client.call(json!({
        "op": "resize",
        "terminal_id": terminal_id,
        "generation": lease["result"]["generation"],
        "token": lease["result"]["token"],
        "cols": 100,
        "rows": 30,
    }));
    assert_eq!(resized["ok"], false, "{resized}");
    assert_eq!(resized["error"]["code"], "terminal_exited", "{resized}");
    let after = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(after["ok"].as_bool().unwrap_or(false), "{after}");
    assert_eq!(after["result"]["revision"], revision, "{after}");
    assert_eq!(after["result"]["text"], text, "{after}");
    assert_eq!(after["result"]["cols"], 80, "{after}");
    assert_eq!(after["result"]["rows"], 24, "{after}");
    assert_eq!(after["result"]["alive"], false, "{after}");
}

#[test]
fn partial_write_resumes_without_duplicating_a_prefix() {
    let daemon = Daemon::start("#!/bin/sh\nsleep 30\n");
    let py = daemon.runtime.join("capture.py");
    fs::write(&py, include_capture_script()).expect("capture script");
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let runtime = cwd.clone();
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-partial",
        "argv": ["/usr/bin/python3", py, runtime],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let _ = wait_text(&mut client, &terminal_id, "READY");
    let lease = client.call(json!({"op": "acquire", "terminal_id": terminal_id}));
    assert!(lease["ok"].as_bool().unwrap_or(false), "{lease}");
    let generation = lease["result"]["generation"].as_u64().unwrap();
    let token = lease["result"]["token"].as_str().unwrap().to_owned();

    let mut accepted = Vec::<u8>::new();
    let mut rejected = None;
    for index in 0..400 {
        let chunk = format!("ROW{index:04}{:.<64}\n", "X");
        let response = client.call(json!({
            "op": "input",
            "terminal_id": terminal_id,
            "generation": generation,
            "token": token,
            "data": chunk,
        }));
        if response["ok"].as_bool().unwrap_or(false) {
            accepted.extend(chunk.as_bytes());
        } else {
            assert_eq!(response["error"]["code"], "pty_backpressure", "{response}");
            rejected = Some(chunk);
            break;
        }
    }
    let rejected = rejected.expect("pty did not report backpressure");
    fs::write(daemon.runtime.join("expect"), accepted.len().to_string()).unwrap();
    fs::write(daemon.runtime.join("go"), "1").unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || {
            daemon.runtime.join("captured").exists()
        }),
        "child did not capture the queued input"
    );
    let captured = fs::read(daemon.runtime.join("captured")).unwrap();
    let extra = fs::read(daemon.runtime.join("captured.extra")).unwrap_or_default();
    assert_eq!(
        captured, accepted,
        "queued tail was truncated or duplicated"
    );
    assert!(
        extra.is_empty(),
        "rejected prefix leaked into the first read: {extra:?}"
    );
    assert!(!captured
        .windows(rejected.len())
        .any(|window| window == rejected.as_bytes()));

    let again = client.call(json!({
        "op": "input",
        "terminal_id": terminal_id,
        "generation": generation,
        "token": token,
        "data": rejected,
    }));
    assert!(again["ok"].as_bool().unwrap_or(false), "{again}");
    fs::write(daemon.runtime.join("expect2"), rejected.len().to_string()).unwrap();
    fs::write(daemon.runtime.join("go2"), "1").unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || {
            daemon.runtime.join("captured2").exists()
        }),
        "retry was not captured"
    );
    let captured2 = fs::read(daemon.runtime.join("captured2")).unwrap();
    let extra2 = fs::read(daemon.runtime.join("captured2.extra")).unwrap_or_default();
    assert_eq!(captured2, rejected.as_bytes());
    assert!(extra2.is_empty(), "retry duplicated bytes: {extra2:?}");
}

#[test]
fn deleted_cwd_still_replays_the_same_operation() {
    let daemon = Daemon::start("#!/bin/sh\nsleep 30\n");
    let work = daemon.runtime.join("work");
    fs::create_dir(&work).expect("work");
    let cwd = work.canonicalize().expect("work cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-cwd",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let pid = launch["result"]["pid"].as_u64().unwrap();
    fs::remove_dir(&work).expect("remove cwd");
    let replay = client.call(json!({
        "op": "launch",
        "operation_id": "op-cwd",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(replay["ok"].as_bool().unwrap_or(false), "{replay}");
    assert_eq!(replay["result"]["spawned"], false);
    assert_eq!(replay["result"]["terminal_id"], terminal_id);
    assert_eq!(replay["result"]["pid"], pid);
    let conflict = client.call(json!({
        "op": "launch",
        "operation_id": "op-cwd",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 30,
    }));
    assert_eq!(conflict["ok"], false, "{conflict}");
    assert_eq!(conflict["error"]["code"], "operation_conflict");
    let missing = client.call(json!({
        "op": "launch",
        "operation_id": "op-new-cwd",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(missing["ok"], false, "{missing}");
    assert_eq!(missing["error"]["code"], "cwd_rejected");
    assert!(pid_alive(pid as u32));
}

#[test]
fn device_status_reply_is_written_to_the_master() {
    let daemon = Daemon::start("#!/bin/sh\nsleep 30\n");
    let py = daemon.runtime.join("dsr.py");
    fs::write(
        &py,
        r#"import os, termios, time
attr = termios.tcgetattr(0)
attr[3] = attr[3] & ~(termios.ECHO | termios.ICANON)
attr[6][termios.VMIN] = 1
attr[6][termios.VTIME] = 0
termios.tcsetattr(0, termios.TCSANOW, attr)
os.write(1, b"\x1b[6n")
buf = b""
while b"R" not in buf and len(buf) < 32:
    chunk = os.read(0, 16)
    if not chunk:
        break
    buf += chunk
os.write(1, b"CPR " + buf.hex().encode() + b"\n")
time.sleep(30)
"#,
    )
    .unwrap();
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-dsr",
        "argv": ["/usr/bin/python3", py],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let snap = wait_text(&mut client, &terminal_id, "CPR");
    let text = snap["text"].as_str().unwrap_or("");
    assert!(
        text.contains("1b5b") && text.contains("52"),
        "cursor reply was {text}"
    );
}

#[test]
fn failed_post_spawn_setup_is_marked_failed() {
    let daemon = Daemon::start_with(
        "#!/bin/sh\nsleep 30\n",
        &[("KEEPLINED_TEST_FAIL_POST_SPAWN", "winsize")],
    );
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-post",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(launch["ok"], false, "{launch}");
    assert_eq!(launch["error"]["code"], "spawn_failed");
    let intent: Value =
        serde_json::from_str(&fs::read_to_string(intent_path(&daemon.runtime, "op-post")).unwrap())
            .unwrap();
    assert_eq!(intent["state"], "failed", "{intent}");
    assert!(
        pids_matching(&daemon.script.display().to_string()).is_empty(),
        "failed setup left a child"
    );
    let again = client.call(json!({
        "op": "launch",
        "operation_id": "op-post",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(again["ok"], false, "{again}");
    assert_eq!(again["error"]["code"], "spawn_failed");
    assert!(again.get("result").is_none() || again["result"].is_null());
}

#[test]
fn eof_reaps_the_child_with_no_client_attached() {
    let daemon = Daemon::start("#!/bin/sh\nprintf 'gone\\n'\nexit 9\n");
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-eof",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    drop(client);
    let started = Instant::now();
    while pid_alive(pid) || process_stat(pid).is_some() {
        if started.elapsed() > Duration::from_secs(5) {
            panic!("child {pid} was not reaped, stat={:?}", process_stat(pid));
        }
        thread::sleep(Duration::from_millis(20));
    }
    let mut client = Client::connect(&daemon.runtime);
    let pull = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": 0,
    }));
    assert!(pull["ok"].as_bool().unwrap_or(false), "{pull}");
    assert_eq!(pull["result"]["alive"], false, "{pull}");
    assert_eq!(pull["result"]["exit_code"], 9, "{pull}");
    let revision = pull["result"]["revision"].as_u64().expect("revision");
    let current = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": revision,
    }));
    assert!(current["ok"].as_bool().unwrap_or(false), "{current}");
    assert_eq!(current["result"]["alive"], false);
    assert_eq!(current["result"]["exit_code"], 9);
    assert_eq!(current["result"]["resync_required"], false);
    assert!(current["result"].get("deltas").is_some());
    assert_eq!(current["result"]["deltas"].as_array().unwrap().len(), 0);
}

#[test]
fn launched_child_keeps_the_daemon_umask() {
    let daemon = Daemon::start_with_umask(
        "#!/bin/sh\numask > child-umask\n: > child-created\nmkdir child-dir\nprintf 'READY\\n'\nwhile true; do sleep 30; done\n",
        "022",
    );
    let mut client = Client::connect(&daemon.runtime);
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-umask",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let _snap = wait_text(&mut client, &terminal_id, "READY");
    let printed = fs::read_to_string(cwd.join("child-umask")).expect("child umask");
    let mask = u32::from_str_radix(printed.trim(), 8).expect(&printed);
    assert_eq!(mask, 0o022, "{printed}");
    assert_eq!(mode_bits(&cwd.join("child-created")), 0o644);
    assert_eq!(mode_bits(&cwd.join("child-dir")), 0o755);
    assert_eq!(mode_bits(&intent_path(&cwd, "op-umask")), 0o600);
}

#[test]
fn oversized_pull_returns_a_resync_snapshot() {
    let daemon = Daemon::start("#!/bin/sh\nsleep 30\n");
    let runtime = daemon.runtime.canonicalize().expect("runtime");
    let py = runtime.join("fill.py");
    fs::write(&py, quote_fill_script()).expect("fill script");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-frame",
        "argv": ["/usr/bin/python3", py, runtime],
        "cwd": runtime,
        "cols": 400,
        "rows": 200,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"].as_str().unwrap().to_owned();
    let settled = wait_quote_screen(&mut client, &terminal_id, &runtime);
    let mut revision = settled["revision"].as_u64().expect("revision");
    for index in 0..keeplined::DELTA_LIMIT {
        fs::write(runtime.join(format!("go-{index}")), "1").expect("go");
        let advanced = wait_revision_above(&mut client, &terminal_id, revision);
        revision = advanced["revision"].as_u64().expect("revision");
        assert!(
            wait_until(Duration::from_secs(5), || {
                runtime.join(format!("done-{index}")).exists()
            }),
            "child did not record update {index}"
        );
    }
    let snap = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(snap["ok"].as_bool().unwrap_or(false), "{snap}");
    let current = snap["result"]["revision"].as_u64().expect("revision");
    let oldest = snap["result"]["oldest_retained_revision"]
        .as_u64()
        .expect("oldest");
    assert_eq!(
        current - oldest,
        (keeplined::DELTA_LIMIT - 1) as u64,
        "{snap}"
    );
    let pull = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": oldest - 1,
    }));
    assert!(pull["ok"].as_bool().unwrap_or(false), "{pull}");
    assert_eq!(pull["result"]["resync_required"], true, "{pull}");
    assert!(pull["result"].get("deltas").is_none(), "{pull}");
    assert_eq!(pull["result"]["alive"], true, "{pull}");
    assert_eq!(pull["result"]["revision"], current);
    assert_eq!(pull["result"]["checksum"], snap["result"]["checksum"]);
    assert!(
        pull["result"]["text"].as_str().unwrap_or("").contains('"'),
        "{pull}"
    );
    let one = client.call(json!({
        "op": "pull",
        "terminal_id": terminal_id,
        "after_revision": current - 1,
    }));
    assert!(one["ok"].as_bool().unwrap_or(false), "{one}");
    assert_eq!(one["result"]["resync_required"], false, "{one}");
    let deltas = one["result"]["deltas"].as_array().expect("deltas");
    assert_eq!(deltas.len(), 1, "{one}");
    assert!(
        deltas[0]["text"].as_str().unwrap_or("").len() > 70_000,
        "one grid was too small to prove the frame limit"
    );
}

#[test]
fn default_keepline_entry_does_not_name_the_daemon() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for relative in [
        "package.json",
        "src/index.ts",
        "src/cli/index.ts",
        "src/cli/daemon.ts",
    ] {
        let text = fs::read_to_string(root.join(relative)).expect(relative);
        assert!(!text.contains("keeplined"), "{relative} starts keeplined");
    }
}

fn script(ticks: usize) -> String {
    format!(
        r#"#!/bin/sh
printf 'plain\033[31mred\033[0m\n'
i=0
while [ "$i" -lt {ticks} ]; do
  printf 'tick %s\n' "$i"
  i=$((i + 1))
  sleep 0.05
done
printf 'READY\n'
while IFS= read -r line; do
  case "$line" in
    ping)
      printf 'pong\n'
      ;;
    size)
      set -- $(/bin/stty size)
      printf 'SIZE %s %s\n' "$1" "$2"
      ;;
  esac
done
"#
    )
}

fn wait_snapshot(client: &mut Client, terminal_id: &str) -> Value {
    let start = Instant::now();
    loop {
        let result = wait_text(client, terminal_id, "READY");
        let text = result["text"].as_str().unwrap_or("");
        if text.contains("plain") && text.contains("red") {
            return result;
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!("timed out waiting for the colored fixture in {text}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn intent_path(runtime: &Path, operation_id: &str) -> PathBuf {
    let stem: String = operation_id
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    runtime
        .join(keeplined::INTENT_DIR_NAME)
        .join(format!("{stem}.json"))
}

fn wait_text(client: &mut Client, terminal_id: &str, needle: &str) -> Value {
    let start = Instant::now();
    loop {
        let response = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
        assert!(response["ok"].as_bool().unwrap_or(false), "{response}");
        let text = response["result"]["text"].as_str().unwrap_or("").to_owned();
        if text.contains(needle) {
            return response["result"].clone();
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!("timed out waiting for {needle} in {text}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_until(timeout: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if ready() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    ready()
}

fn mode_bits(path: &Path) -> u32 {
    fs::metadata(path)
        .unwrap_or_else(|err| panic!("metadata {}: {err}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

fn wait_revision_above(client: &mut Client, terminal_id: &str, revision: u64) -> Value {
    let start = Instant::now();
    loop {
        let response = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
        assert!(response["ok"].as_bool().unwrap_or(false), "{response}");
        let current = response["result"]["revision"].as_u64().unwrap_or(0);
        if current > revision {
            return response["result"].clone();
        }
        if start.elapsed() > Duration::from_secs(5) {
            panic!("revision stayed at {current}, wanted above {revision}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_quote_screen(client: &mut Client, terminal_id: &str, runtime: &Path) -> Value {
    let start = Instant::now();
    let mut last_revision = None;
    let mut changed_at = Instant::now();
    loop {
        if runtime.join("filled").exists() {
            let response = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
            assert!(response["ok"].as_bool().unwrap_or(false), "{response}");
            let revision = response["result"]["revision"].as_u64().unwrap_or(0);
            if Some(revision) != last_revision {
                last_revision = Some(revision);
                changed_at = Instant::now();
            }
            let text = response["result"]["text"].as_str().unwrap_or("");
            let quotes = text.matches('"').count();
            if quotes >= 400 * 180 && changed_at.elapsed() >= Duration::from_millis(250) {
                return response["result"].clone();
            }
            if start.elapsed() > Duration::from_secs(8) {
                panic!(
                    "quote screen did not settle: quotes={quotes} revision={revision} bytes={}",
                    text.len()
                );
            }
        } else if start.elapsed() > Duration::from_secs(8) {
            panic!("child did not fill the screen");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn quote_fill_script() -> &'static str {
    r#"import os, sys, time
runtime = sys.argv[1]
cols = 400
rows = 200
row = b'"' * cols

def write_all(payload):
    view = memoryview(payload)
    while view:
        written = os.write(1, view)
        if written == 0:
            raise SystemExit("pty write returned no bytes")
        view = view[written:]

parts = [b"\x1b[H"]
for number in range(1, rows + 1):
    parts.append(b"\x1b[%d;1H" % number)
    parts.append(row)
write_all(b"".join(parts))
open(os.path.join(runtime, "filled"), "w").close()
for index in range(8):
    flag = os.path.join(runtime, "go-%d" % index)
    while not os.path.exists(flag):
        time.sleep(0.02)
    write_all(b"\x1b[1;1H" + str(index).encode())
    open(os.path.join(runtime, "done-%d" % index), "w").close()
time.sleep(30)
"#
}

fn wait_mode(path: &Path, expected: u32) {
    assert!(
        wait_until(Duration::from_secs(2), || {
            fs::metadata(path)
                .map(|metadata| metadata.permissions().mode() & 0o777 == expected)
                .unwrap_or(false)
        }),
        "{} mode was {:o}",
        path.display(),
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .unwrap_or(0)
    );
}

fn include_capture_script() -> &'static str {
    r#"import fcntl
import os
import sys
import termios
import time

runtime = sys.argv[1]
attr = termios.tcgetattr(0)
attr[3] = attr[3] & ~termios.ECHO
termios.tcsetattr(0, termios.TCSANOW, attr)
os.write(1, b"READY\n")

def wait_flag(name):
    path = os.path.join(runtime, name)
    while not os.path.exists(path):
        time.sleep(0.02)

def read_exact(n):
    buf = bytearray()
    while len(buf) < n:
        chunk = os.read(0, n - len(buf))
        if not chunk:
            break
        buf += chunk
    return bytes(buf)

def read_extra():
    flags = fcntl.fcntl(0, fcntl.F_GETFL)
    fcntl.fcntl(0, fcntl.F_SETFL, flags | os.O_NONBLOCK)
    try:
        return os.read(0, 256)
    except BlockingIOError:
        return b""
    finally:
        fcntl.fcntl(0, fcntl.F_SETFL, flags)

wait_flag("go")
expect = int(open(os.path.join(runtime, "expect"), encoding="utf-8").read())
got = read_exact(expect)
extra = read_extra()
open(os.path.join(runtime, "captured"), "wb").write(got)
open(os.path.join(runtime, "captured.extra"), "wb").write(extra)
os.write(1, f"PHASE1 {len(got)}\n".encode())
wait_flag("go2")
expect2 = int(open(os.path.join(runtime, "expect2"), encoding="utf-8").read())
got2 = read_exact(expect2)
extra2 = read_extra()
open(os.path.join(runtime, "captured2"), "wb").write(got2)
open(os.path.join(runtime, "captured2.extra"), "wb").write(extra2)
os.write(1, f"PHASE2 {len(got2)}\n".encode())
time.sleep(30)
"#
}

fn foreground_tty(pid: u32) -> (u32, u32, String, u32) {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "pgid=,tpgid=,tty=,flags="])
        .output()
        .expect("ps");
    let line = String::from_utf8_lossy(&output.stdout);
    let mut parts = line.split_whitespace();
    let pgid = parts.next().unwrap_or("0").parse().expect("pgid");
    let tpgid = parts.next().unwrap_or("0").parse().expect("tpgid");
    let tty = parts.next().unwrap_or("??").to_owned();
    let flags = u32::from_str_radix(parts.next().unwrap_or("0"), 16).expect("flags");
    (pgid, tpgid, tty, flags)
}

fn process_stat(pid: u32) -> Option<String> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "stat="])
        .output()
        .expect("ps");
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn pid_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn child_has_exited(pid: u32) -> bool {
    if !pid_alive(pid) {
        return true;
    }
    process_stat(pid)
        .map(|stat| stat.starts_with('Z'))
        .unwrap_or(true)
}

fn pids_matching(marker: &str) -> Vec<u32> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,command="])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(marker))
        .filter_map(|line| line.split_whitespace().next()?.parse().ok())
        .collect()
}
