use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
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
        static NEXT_RUNTIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT_RUNTIME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let runtime = PathBuf::from(format!("/tmp/kl{}-{id}", std::process::id()));
        fs::create_dir_all(&runtime).expect("runtime dir");
        let script = runtime.join("child.sh");
        fs::write(&script, script_body).expect("script");
        let mut child = Command::new(env!("CARGO_BIN_EXE_keeplined"))
            .args(["serve", "--runtime"])
            .arg(&runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn keeplined");
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

    let intent_path = daemon
        .runtime
        .join(keeplined::INTENT_DIR_NAME)
        .join("op-main.json");
    let intent: Value = serde_json::from_str(&fs::read_to_string(&intent_path).expect("intent"))
        .expect("intent json");
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
    assert_eq!(
        accepted["result"]["accepted"].as_u64(),
        Some(u64::try_from("ping\n".len()).expect("len"))
    );
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
        &fs::read_to_string(
            daemon
                .runtime
                .join(keeplined::INTENT_DIR_NAME)
                .join("op-missing.json"),
        )
        .unwrap(),
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

#[test]
fn second_serve_does_not_replace_a_live_socket() {
    let daemon = Daemon::start(&script(1));
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-live",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;

    let mut second = Command::new(env!("CARGO_BIN_EXE_keeplined"))
        .args(["serve", "--runtime"])
        .arg(&daemon.runtime)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("second serve");
    let started = Instant::now();
    let status = loop {
        match second.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > Duration::from_secs(5) => {
                let _ = second.kill();
                let _ = second.wait();
                let mut note = String::new();
                if let Some(mut err) = second.stderr.take() {
                    let _ = err.read_to_string(&mut note);
                }
                panic!("second serve kept running: {note}");
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(err) => panic!("wait for second serve: {err}"),
        }
    };
    let mut note = String::new();
    if let Some(mut err) = second.stderr.take() {
        err.read_to_string(&mut note).expect("second stderr");
    }
    assert!(!status.success(), "second serve exited 0: {note}");
    assert!(
        note.contains("already accepts"),
        "second serve stderr was {note}"
    );

    let mut again = Client::connect(&daemon.runtime);
    let attach = again.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(attach["ok"].as_bool().unwrap_or(false), "{attach}");
    assert_eq!(attach["result"]["pid"], pid);
    assert!(pid_alive(pid), "original child {pid} died");
}

#[test]
fn replay_ignores_a_removed_cwd() {
    let daemon = Daemon::start(&script(1));
    let work = daemon.runtime.join("work");
    fs::create_dir(&work).expect("work");
    let cwd = work.canonicalize().expect("cwd");
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
    let terminal_id = launch["result"]["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    let pid = launch["result"]["pid"].as_u64().expect("pid");
    fs::rename(&work, daemon.runtime.join("work-gone")).expect("rename work");

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
    assert_eq!(
        pids_matching(&daemon.script.display().to_string()),
        vec![pid as u32]
    );

    let conflict = client.call(json!({
        "op": "launch",
        "operation_id": "op-cwd",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 30,
    }));
    assert_eq!(conflict["error"]["code"], "operation_conflict");

    let rejected = client.call(json!({
        "op": "launch",
        "operation_id": "op-cwd-new",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert_eq!(rejected["error"]["code"], "cwd_rejected");
    assert_eq!(
        pids_matching(&daemon.script.display().to_string()),
        vec![pid as u32]
    );
}

#[test]
fn exited_child_is_reaped_without_another_request() {
    let daemon = Daemon::start("#!/bin/sh\nprintf 'bye\\n'\nexit 0\n");
    let cwd = daemon.runtime.canonicalize().expect("cwd");
    let mut client = Client::connect(&daemon.runtime);
    let launch = client.call(json!({
        "op": "launch",
        "operation_id": "op-exit",
        "argv": ["/bin/sh", daemon.script],
        "cwd": cwd,
        "cols": 80,
        "rows": 24,
    }));
    assert!(launch["ok"].as_bool().unwrap_or(false), "{launch}");
    let terminal_id = launch["result"]["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    let pid = launch["result"]["pid"].as_u64().expect("pid") as u32;
    assert!(
        wait_until(Duration::from_secs(2), || process_state(pid).is_none()),
        "child {pid} was not reaped (state {:?})",
        process_state(pid)
    );
    let attach = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
    assert!(attach["ok"].as_bool().unwrap_or(false), "{attach}");
    assert_eq!(attach["result"]["alive"], false);
    assert_eq!(attach["result"]["exit_code"], 0);
}

fn script(ticks: usize) -> String {
    format!(
        r#"#!/bin/sh
printf 'plain\033[31mred\033[0m\n'
i=0
while [ "$i" -lt {ticks} ]; do
  printf 'tick %s\n' "$i"
  i=$((i + 1))
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
    wait_text(client, terminal_id, "READY")
}

fn wait_text(client: &mut Client, terminal_id: &str, needle: &str) -> Value {
    let start = Instant::now();
    loop {
        let response = client.call(json!({"op": "attach", "terminal_id": terminal_id}));
        assert!(response["ok"].as_bool().unwrap_or(false), "{response}");
        let text = response["result"]["text"].as_str().unwrap_or("").to_owned();
        if text.contains(needle)
            && (needle != "READY" || (text.contains("plain") && text.contains("red")))
        {
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

fn process_state(pid: u32) -> Option<String> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "state="])
        .output()
        .expect("ps");
    let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if state.is_empty() {
        None
    } else {
        Some(state)
    }
}

fn pid_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
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
