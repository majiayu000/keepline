# M0 managed PTY

Issue #120 is the epic for a shared Rust runtime, a terminal TUI, and a native
macOS app. This slice is only the experimental background PTY. It does not
close the epic.

`keeplined` lives under `next/` and speaks protocol major 0. The default
`keepline` command still opens the web observer. It does not spawn this daemon.
Observed sessions stay outside the managed PTY. This crate does not write the
Bun SQLite database and does not restore a browser terminal.

## What this slice does

`keeplined serve --runtime <dir>` creates a user-private Unix socket. The
runtime directory is mode `0700` and the socket is mode `0600`. After
canonicalizing, the daemon refuses the runtime directory and `intents/` unless
`st_uid` is the daemon euid, and it does not change the mode of a directory it
does not own. The runtime lock is opened without following a symlink. The process
umask is tightened only while those paths are created, then restored, so a
launched child keeps the umask the daemon started with. A peer whose uid does
not match the daemon euid is rejected. Creating `intents/` fsyncs that directory
entry in the runtime directory before any child is spawned. If this process
created any missing path component, it fsyncs that component's parent. One
runtime directory has one live daemon. A second `serve` that can reach the
existing socket exits with
`already_running` and does not unlink that socket or touch its children. A
stale socket is replaced only after an exclusive lock shows that no peer is
accepting on it.

`launch` writes an intent file and fsyncs it before `Command` spawns
`argv`. The stored cols and rows stay the requested geometry, so the
canonical payload still matches if the child changes the PTY winsize before
the daemon reads it. That readback becomes the live size only inside 2..=400
columns by 2..=200 rows. A readback outside that range is a failed spawn.
`argv[0]` is an absolute path; the daemon does not join
a shell string. The child is a session leader. Its controlling terminal and foreground
process group are the PTY, so an interrupt written to the master and
`SIGWINCH` from a winsize change reach that child. The child starts with the
default dispositions for those terminal signals, including when the daemon
inherited them as ignored. Repeating an `operation_id`
compares the canonical payload before any cwd existence check. The same
payload returns the original terminal even if that directory was removed. A
different payload is `operation_conflict`. A new launch still rejects a
missing cwd. A failed spawn, including a winsize or reader-setup failure
after the child exists, kills and reaps that child and records `failed`.
Replaying it returns `spawn_failed` and not a live terminal. After the daemon
process restarts, a recorded operation is not spawned again and is not
attachable, because this slice does not hand off a live file descriptor.

PTY bytes are parsed once with pinned `alacritty_terminal` 0.25.1. One
non-empty read records one revision and hashes the grid once. `Event::PtyWrite`
replies, including device-status and cursor-position reports, are written
back to the PTY master by the daemon. Clipboard, color, and text-area requests
are not executed as client actions and are not answered separately by each
client. Clients attach read-only and receive the same snapshot revision and
grid checksum. The checksum covers cell text, including at most one zero-width
scalar per cell, and SGR color. The visible grid for the fixture includes plain
text plus one red SGR sequence. Snapshot and pull text keep that cell text,
including trailing non-ASCII spacing, and omit only U+0020 padding. Queued
writes to the master stay bounded: once they pass the input-tail budget, the
reader stops taking further PTY output
until those bytes flush.

Input and resize require an explicit fencing lease. `acquire` fails when a
lease is already held. `takeover` increments the generation and replaces the
token; the previous token is rejected. A resize without the current token
does not change the PTY winsize. A resize readback inside 2..=400 columns by
2..=200 rows is kept, including one that differs from the request. A readback
outside that range restores the previous winsize and does not resize the
screen. A later child `stty` does not resize the parser. A resize after the
child exit has been published does not change the grid. An input request is either accepted in full,
with any unwritten tail queued in daemon order, or rejected with
`pty_backpressure` before any byte of that request is written. A later flush
writes the queued tail once, so a client retry does not duplicate an accepted
prefix. Disconnecting every client revokes the lease and leaves the child
running. A later attach uses the same pid. When the PTY reaches EOF or the
reader hits a read or poll error, the child is waited and the exit is
published without holding the daemon-wide mutex across that wait. That
publication waits until the reader has drained the PTY, so an earlier child
exit does not hide output still buffered in the master. Exit with
no new bytes is visible from `alive` and `exit_code`; it does not invent a
grid revision.

The daemon keeps eight grid views. A contiguous `pull` returns, for each
revision after `after_revision`, the grid text, checksum, and geometry from
that revision when that response fits in one frame. Every pull response
includes `alive` and `exit_code`, including a client that is already at the
current revision. A gap, and a contiguous pull whose encoded frame would
exceed 1 MiB, returns one snapshot with `resync_required` and no delta list
to splice.

Frames are a 4-byte big-endian length plus JSON, at most 1 MiB. Major 0 is
experimental. A hello with any other major returns `unsupported_protocol`.

## Out of this slice

Protobuf, a Ratatui TUI, AppKit/Metal, WorkItem migration, hooks, attention,
search, usage, SSH, Windows, LaunchAgent, notarization, a libghostty
comparison, and the W0–W3 budgets are later milestones.

## Check

```bash
cargo test --manifest-path next/Cargo.toml
cargo fmt --check --manifest-path next/Cargo.toml
cargo clippy --manifest-path next/Cargo.toml --all-targets -- -D warnings
bun run typecheck
```
