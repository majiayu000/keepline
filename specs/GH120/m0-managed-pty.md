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
runtime directory is mode `0700` and the socket is mode `0600`. A peer whose
uid does not match the daemon is rejected.

`launch` writes an intent file and fsyncs it before `Command` spawns
`argv`. `argv[0]` is an absolute path; the daemon does not join a shell
string. The same `operation_id` and payload return the original terminal and
do not spawn again. A different payload for that id is `operation_conflict`.
A failed spawn is recorded and is not retried. After the daemon process
restarts, a recorded operation is not spawned again and is not attachable,
because this slice does not hand off a live file descriptor.

PTY bytes are parsed once with pinned `alacritty_terminal` 0.25.1. Clients
attach read-only and receive the same snapshot revision and grid checksum.
The checksum covers cell text and SGR color. The visible grid for the fixture
includes plain text plus one red SGR sequence.

Input and resize require an explicit fencing lease. `acquire` fails when a
lease is already held. `takeover` increments the generation and replaces the
token; the previous token is rejected. A resize without the current token
does not change the PTY winsize. Disconnecting every client revokes the
lease and leaves the child running. A later attach uses the same pid.

The daemon keeps eight grid revisions. `pull` returns those deltas only when
they are contiguous. A gap returns `resync_required` and a snapshot, with no
delta list to splice.

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
