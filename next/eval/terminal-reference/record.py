#!/usr/bin/env python3
"""Record scripted sessions of real TUIs at a fixed grid, as raw VT output (fixtures/*.vt).

Equivalent to `script -q out cmd`, but with the PTY size pinned to COLS x ROWS and keys
sent on a timeline, so the recording is reproducible. Every checkpoint writes the output
received so far, so a single run yields several screens (e.g. vim normal vs insert mode).
Keystrokes are scripted; nothing typed by a person is recorded.

Usage: python3 record.py [codex|vim|lazygit ...]   (default: all)
"""

from __future__ import annotations

import fcntl
import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

COLS, ROWS = 100, 30
HERE = Path(__file__).resolve().parent
OUT = HERE / "fixtures"


def vim_file(work: Path) -> Path:
    path = work / "sample.rs"
    path.write_text(
        "// 终端对照：vim 样例，含中文注释与制表线 ─│┌┐\n"
        "use std::collections::HashMap;\n\n"
        "fn main() {\n"
        "    let mut counts: HashMap<&str, usize> = HashMap::new();\n"
        "    for word in \"a b a c 中 文 中\".split(' ') {\n"
        "        *counts.entry(word).or_default() += 1;\n"
        "    }\n"
        "    println!(\"{counts:?}\"); // ✅ done\n"
        "}\n"
    )
    return path


def lazygit_repo(work: Path) -> Path:
    repo = work / "repo"
    repo.mkdir()
    git = lambda *a: subprocess.run(["git", "-C", str(repo), *a], check=True, capture_output=True)  # noqa: E731
    git("init", "-q", "-b", "main")
    git("config", "user.email", "fixture@example.invalid")
    git("config", "user.name", "Fixture")
    for i, msg in enumerate(["feat: 初始化终端对照", "fix: box drawing ─│┌┐", "docs: emoji ✅ 🚀"]):
        (repo / f"file{i}.txt").write_text(f"line {i}\n中文 {i}\n")
        git("add", ".")
        git("commit", "-q", "-m", msg)
    (repo / "file0.txt").write_text("changed\n修改\n")
    (repo / "new.txt").write_text("untracked\n")
    return repo


# (delay seconds, bytes to send or None, checkpoint name or None)
def scenarios(work: Path) -> dict[str, tuple[list[str], Path, list]]:
    vim_rc = work / "vimrc"
    # Fixed config so the screen does not depend on the user's ~/.vimrc.
    vim_rc.write_text("set nocompatible\nsyntax on\nset number\nset laststatus=2\nset showmode\n"
                      "set termguicolors\nset encoding=utf-8\n"
                      "let &t_SI = \"\\e[6 q\"\nlet &t_EI = \"\\e[2 q\"\n")
    sample = vim_file(work)
    codex = shutil.which("codex") or "codex"
    return {
        "codex": ([codex], work, [
            (1.2, None, "codex-logo"), (4.8, None, "codex-start"),
            (0.5, b"\x03", None), (0.5, b"\x03", None), (1.0, None, None),
        ]),
        "vim": (["vim", "-u", str(vim_rc), "-i", "NONE", str(sample)], work, [
            (2.0, None, "vim-normal"),
            (0.5, b"5Gi", None), (0.8, None, "vim-insert"),
            (0.3, "    // 插入模式 ✏️ insert".encode(), None),
            (0.8, b"\x1b", None), (0.8, None, "vim-back-normal"),
            (0.3, b":q!\r", None), (0.8, None, None),
        ]),
        # CONFIG_DIR isolates lazygit state, so the first-run popup always appears and is dismissed.
        "lazygit": (["env", f"CONFIG_DIR={work / 'lazygit'}", "lazygit", "-p", str(lazygit_repo(work))], work, [
            (4.0, b"\r", None), (1.0, b"2", None), (1.5, None, "lazygit-status"),
            (0.3, b"q", None), (1.0, None, None),
        ]),
    }


def record(name: str, argv: list[str], cwd: Path, steps: list) -> None:
    env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor", COLUMNS=str(COLS), LINES=str(ROWS))
    env.pop("TMUX", None)
    pid, master = pty.fork()
    if pid == 0:
        os.chdir(cwd)
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        os.execvpe(argv[0], argv, env)
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    buf = bytearray()

    def pump(seconds: float) -> None:
        end = time.monotonic() + seconds
        while (left := end - time.monotonic()) > 0:
            r, _, _ = select.select([master], [], [], left)
            if r:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    return
                if not data:
                    return
                buf.extend(data)

    for delay, keys, checkpoint in steps:
        pump(delay)
        if keys:
            os.write(master, keys)
        if checkpoint:
            (OUT / f"{checkpoint}.vt").write_bytes(bytes(buf))
            print(f"{name}: {checkpoint}.vt ({len(buf)} bytes)")
    try:
        os.kill(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    os.waitpid(pid, 0)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    want = sys.argv[1:] or ["codex", "vim", "lazygit"]
    with tempfile.TemporaryDirectory(prefix="kl-ref-") as tmp:
        all_scenarios = scenarios(Path(tmp))
        for name in want:
            argv, cwd, steps = all_scenarios[name]
            tool = argv[2] if argv[0] == "env" else argv[0]
            if not shutil.which(tool):
                print(f"{name}: skipped, {tool} not installed", file=sys.stderr)
                continue
            record(name, argv, cwd, steps)
    return 0


if __name__ == "__main__":
    sys.exit(main())
