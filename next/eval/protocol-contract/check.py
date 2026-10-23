"""Checks the CLI interfaces Keepline's status rules read, after a Codex or Claude Code upgrade.

    python3 -I check.py            # schema and command shape only, read-only
    python3 -I check.py --live     # also one read-only thread/read on the running Codex daemon

Exit 1 when a field Keepline depends on changed; unknown extra values are reported, since
Keepline skips them rather than guessing. See README.md for what each check protects.
"""
import base64
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile

FAILED = []
WARNED = []


def check(ok, what):
    print(("PASS " if ok else "FAIL ") + what)
    if not ok:
        FAILED.append(what)


def warn(what):
    print("WARN " + what)
    WARNED.append(what)


def run(*argv, timeout=20):
    return subprocess.run(argv, capture_output=True, text=True, timeout=timeout, check=True).stdout


def methods(schema):
    out = set()

    def walk(v):
        if isinstance(v, dict):
            m = v.get("properties", {}).get("method", {})
            out.update(m.get("enum", []))
            for x in v.values():
                walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)

    walk(schema)
    return out


def codex_schema():
    print("codex", run("codex", "--version").strip())
    with tempfile.TemporaryDirectory() as out:
        run("codex", "app-server", "generate-json-schema", "--out", out, timeout=60)
        v2 = os.path.join(out, "v2")
        load = lambda *p: json.load(open(os.path.join(*p), encoding="utf-8"))
        status = load(v2, "ThreadStatusChangedNotification.json")["definitions"]
        variants = {
            o["properties"]["type"]["enum"][0]: o for o in status["ThreadStatus"]["oneOf"]
        }
        check({"idle", "active"} <= set(variants), "ThreadStatus has idle and active")
        check(
            "activeFlags" in variants.get("active", {}).get("properties", {}),
            "active ThreadStatus carries activeFlags",
        )
        extra = set(variants) - {"notLoaded", "idle", "systemError", "active"}
        if extra:
            warn(f"new ThreadStatus variants Keepline ignores: {sorted(extra)}")
        flags = set(status["ThreadActiveFlag"]["enum"])
        check(
            {"waitingOnApproval", "waitingOnUserInput"} <= flags,
            "ThreadActiveFlag has waitingOnApproval and waitingOnUserInput",
        )
        if flags - {"waitingOnApproval", "waitingOnUserInput"}:
            warn(f"new ThreadActiveFlag values read as busy: {sorted(flags)}")
        read = load(v2, "ThreadReadParams.json")["properties"]
        check({"threadId", "includeTurns"} <= set(read), "thread/read takes threadId, includeTurns")
        requests = methods(load(out, "ClientRequest.json"))
        check({"initialize", "thread/read"} <= requests, "initialize and thread/read exist")


def codex_live():
    path = os.path.join(
        os.environ.get("CODEX_HOME", os.path.expanduser("~/.codex")),
        "app-server-control",
        "app-server-control.sock",
    )
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(5)
    s.connect(path)
    key = base64.b64encode(os.urandom(16)).decode()
    s.sendall(
        f"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode()
    )
    buf = b""
    while b"\r\n\r\n" not in buf:
        buf += s.recv(4096)
    head, _, buf = buf.partition(b"\r\n\r\n")
    check(head.startswith(b"HTTP/1.1 101"), "control socket upgrades to WebSocket")

    def send(o):
        d = json.dumps(o).encode()
        m = os.urandom(4)
        n = len(d)
        h = bytes([0x81]) + (
            bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n)
        )
        s.sendall(h + m + bytes(b ^ m[i % 4] for i, b in enumerate(d)))

    def message():
        nonlocal buf
        while True:
            while len(buf) < 2:
                buf += s.recv(65536)
            n, o = buf[1] & 0x7F, 2
            if n == 126:
                n, o = struct.unpack(">H", buf[2:4])[0], 4
            elif n == 127:
                n, o = struct.unpack(">Q", buf[2:10])[0], 10
            while len(buf) < o + n:
                buf += s.recv(65536)
            op, d, buf = buf[0] & 0xF, buf[o : o + n], buf[o + n :]
            if op == 1:
                return json.loads(d)

    def call(i, method, params):
        send({"id": i, "method": method, "params": params})
        while True:
            m = message()
            if m.get("id") == i:
                return m

    check("result" in call(1, "initialize", {"clientInfo": {"name": "keepline-contract", "version": "0"}}), "initialize answers")
    send({"method": "initialized"})
    loaded = call(2, "thread/loaded/list", {}).get("result", {}).get("data", [])
    if not loaded:
        warn("no loaded thread to read; thread/read not exercised")
        return
    thread = call(3, "thread/read", {"threadId": loaded[0], "includeTurns": False})["result"]["thread"]
    check(thread.get("id") == loaded[0], "thread/read returns the thread by id")
    check(isinstance(thread.get("status", {}).get("type"), str), "thread/read returns status.type")
    missing = call(4, "thread/read", {"threadId": "00000000-0000-0000-0000-000000000000", "includeTurns": False})
    check("error" in missing, "an unloaded thread is an error, not a guess")


def codex_subagent_header():
    """Subagent tracking reads the child's own session header for its parent."""
    root = os.path.join(os.environ.get("CODEX_HOME", os.path.expanduser("~/.codex")), "sessions")
    logs = sorted(
        (os.path.join(d, f) for d, _, fs in os.walk(root) for f in fs if f.endswith(".jsonl")),
        key=os.path.getmtime,
        reverse=True,
    )[:300]
    for path in logs:
        with open(path, encoding="utf-8") as f:
            head = json.loads(f.readline() or "{}")
        meta = head.get("payload", {}) if head.get("type") == "session_meta" else {}
        if isinstance(meta.get("source"), dict) and "subagent" in meta["source"]:
            spawn = meta["source"]["subagent"].get("thread_spawn", {})
            check(
                isinstance(spawn.get("parent_thread_id") or meta.get("parent_thread_id"), str),
                "a subagent's session header names its parent thread",
            )
            return
    warn("no subagent session among the newest 300 logs; header not checked")


def claude():
    print("claude", run("claude", "--version").strip())
    rows = json.loads(run("claude", "agents", "--json"))
    check(isinstance(rows, list), "claude agents --json is a list")
    known = {"cwd", "kind", "startedAt", "id", "state", "pid", "status", "waitingFor", "sessionId", "name"}
    keys = set().union(*(r.keys() for r in rows)) if rows else set()
    check({"sessionId", "status"} & keys or not rows, "entries carry sessionId and status")
    if keys - known:
        warn(f"new claude agents fields: {sorted(keys - known)}")
    statuses = {r.get("status") for r in rows if "status" in r}
    states = {r.get("state") for r in rows if "state" in r}
    waits = {r.get("waitingFor") for r in rows if "waitingFor" in r}
    if statuses - {"busy", "waiting", "idle"}:
        warn(f"unknown status values Keepline skips: {sorted(statuses)}")
    if states - {"working", "blocked", "done", "failed", "stopped"}:
        warn(f"unknown state values Keepline skips: {sorted(states)}")
    documented = {"permission prompt", "input needed", "sandbox request", "worker request", "dialog open"}
    if waits - documented:
        warn(f"waitingFor shown untranslated: {sorted(waits - documented)}")


if __name__ == "__main__":
    for step in [codex_schema, codex_subagent_header, claude] + ([codex_live] if "--live" in sys.argv else []):
        try:
            step()
        except (OSError, subprocess.SubprocessError, KeyError, ValueError) as e:
            check(False, f"{step.__name__}: {e}")
    print(f"{len(FAILED)} failed, {len(WARNED)} warnings")
    sys.exit(1 if FAILED else 0)
