<div align="center">

<img src="docs/assets/logo.svg" alt="Keepline Logo" width="120" />

# Keepline

### Know what your agents did, what was checked, and what needs you.

**A local progress ledger for Codex and Claude Code. Compare your requests with execution evidence, review the gaps, and explicitly accept the result.**

[![GitHub stars](https://img.shields.io/github/stars/majiayu000/keepline?style=flat-square&color=ffcc00)](https://github.com/majiayu000/keepline)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)

[Quick Start](#quick-start) | [Features](#features) | [Dashboard](#web-dashboard) | [CLI reference](#cli-reference) | [中文](README_CN.md)

<br />


</div>

---

## What Keepline helps you decide

Return after a break and see which agent needs an answer, what it changed, which
checks ran, and whether the result meets your request. Open the original session
to continue; completion requires your explicit acceptance.

Keepline is moving to a native agent workbench: a Rust background process with a
GPUI client, where each session is a real terminal inside the app. See the
[product card](https://github.com/majiayu000/keepline/blob/feat/agent-workbench/specs/product-card.md) and [#147](https://github.com/majiayu000/keepline/pull/147). Until that work is merged, the Bun CLI,
browser dashboard and Tauri menu-bar app in this branch remain available for
session monitoring and recovery; the 1.2.0 menu-bar candidate is not planned for release.

## Progress ledger

- **Requests:** preserve your original words and the requirements to check.
- **Evidence:** link recorded command results, test output, file operations and
  commits to the work. An agent's own completion claim is not execution evidence.
- **Acceptance:** show gaps and off-plan work; only you accept the result.
- **Goals and todos:** link sessions to work items and roll progress up.
- **Process and turn state:** keep them separate. Missing process observations
  mean unknown, rather than proving the process exited or the task finished.

Parsing local transcripts works without hooks. Optional hooks deliver live events;
optional model analysis helps assess requirements and evidence. Neither is a
prerequisite for viewing recorded activity, and model analysis cannot invent evidence.
See the [product specification](specs/progress-ledger/product.md) and
[technical design](specs/progress-ledger/tech.md).

## Repository scope

- `src/`: Bun service, CLI, session adapters, progress ledger and React dashboard.
- `menubar-tauri/`: macOS desktop app, embedded service, tray and notifications.
- `sdk/swift/`: client for the stable local API.
- `next/crates/keeplined/`: experimental Rust PTY daemon.
- [`specs/terminal-workbench/`](https://github.com/majiayu000/keepline/tree/feat/agent-workbench/specs/terminal-workbench) (on the `feat/agent-workbench` branch): specification for the native
  agent workbench, implemented on that branch and not yet merged.

---

## Quick Start

### Bun CLI and dashboard from source

Requires Git and Bun 1.1+.

```bash
git clone https://github.com/majiayu000/keepline.git keepline
cd keepline
bun install --frozen-lockfile
(cd src/web/client && bun install --frozen-lockfile)
bun run build
bun run start web
```

Open **http://127.0.0.1:3377**. For CLI commands shown below, run
`bun dist/index.js` in place of `keepline` from this checkout.

By default the web server binds to loopback only. To expose it intentionally, set `KEEPLINE_HOST`.

If you put Keepline behind a reverse proxy (Caddy, nginx, cloudflared), set `KEEPLINE_PUBLIC_ORIGIN=https://your-public-host.example` so the dashboard accepts the browser's public host. For multiple public origins, use comma-separated `KEEPLINE_ALLOWED_ORIGINS`.

If you also want the in-process rate limiter to identify real clients behind that proxy, set `KEEPLINE_TRUST_PROXY=true`. Without that flag, `X-Forwarded-For` is treated as untrusted input and the limiter keys on the actual TCP peer instead — this is intentional, so a malicious caller cannot bypass throttling by spoofing forwarded headers.

### Keepline desktop candidate (1.2.0)

The desktop app now combines a compact menu-bar panel with a progress dashboard.
It embeds the local Bun service; running the dashboard does not require a separate
CLI installation. This checkout is a **candidate**, not evidence of a published
1.2.0 release, and it is not planned for release: the menu bar moves to the native
workbench. Existing release downloads may still contain the older quota app.

See [desktop build and first-run instructions](menubar-tauri/README.md).

1. Open Keepline and let the initial local scan finish.
2. Open **Settings → Connect agents** (连接 agent) and click **Connect live events**
   (连接实时事件). This installs Keepline hooks while preserving other commands.
3. Approve the new hook in Codex, or restart Claude Code, then run a turn.
4. Check that the status reports a real received event. “Installed” alone does
   not mean events are arriving. Local transcript scanning also works without hooks.
5. Open a session to review requirements, execution evidence and remaining gaps;
   acceptance is your explicit decision.

---

## Features

### Progress dashboard

- Requirements, execution steps, test evidence and explicit acceptance in one view.
- Goals and todos linked to sessions; follow-up prompts and session recovery.
- Separate process state from turn state: an unobserved process stays unknown.
- Menu-bar counts and session navigation, focus mode and opt-in system notifications.
- Local transcript parsing by default; optional model analysis is configured separately.


### Real-time Session Monitoring

Monitor local agent runtime sessions across your system. See runtime, status, current file, last tool, and activity at a glance.

```
┌─────────────────────────────────────────────────────────────────┐
│ SESSIONS                                            3 active    │
├─────────────────────────────────────────────────────────────────┤
│ ● RUNNING   my-app          Edit: src/auth.ts      2s ago      │
│ ● WAITING   api-service     Read: README.md        5m ago      │
│ ○ IDLE      docs            Write: guide.md       15m ago      │
│ ✗ LOST      old-project     —                      2h ago      │
└─────────────────────────────────────────────────────────────────┘
```

### One-Click Session Recovery

After a terminal exits, locate a session marked lost and choose an available recovery method. Recovery depends on local files, the project directory and the owning CLI.

```bash
keepline recover <session-id>
```

Three recovery methods:
- **Resume** — Ask the owning CLI to resume the specified session when its file exists
- **Continue** — Continue the owning CLI's latest session in that directory (may differ from the selected ID)
- **New** — Fresh start with original prompt

### Recover an interrupted Codex or Claude Code session

```bash
keepline list --status lost
keepline recover
keepline recover <session-id> --method resume
```

The first command lists lost sessions; `recover` without an argument lists
recovery candidates and methods. Prefer the full session ID, confirm the project
directory and owning CLI, then choose `resume`, `continue` or `new`.
Without `--terminal`, recovery prints a command for you to run; with
`--terminal`, it attempts to open a terminal and execute that command.
Running/completed sessions are outside lost-session recovery, and a missing
project directory is an error.

`resume` cannot recreate a deleted transcript. `new` starts with the original
prompt rather than the previous conversation. For encrypted backup and export
recovery drills, see [chat-archive-rs](https://github.com/majiayu000/chat-archive-rs).
Session monitoring and runtime recovery serve a different purpose from archives.


### Usage, costs and cross-session memory

Usage and cost views use the token data available from each runtime, including
cache reads and writes where recorded. Model rates come from the pricing table;
costs are estimates and do not establish your provider's billed amount or quota.
See [pricing](src/services/usage.pricing.ts).

The memory commands preserve session notes and recovery context. These are
separate from the progress ledger's execution evidence and user acceptance.

### Plans Tracking

Track progress on Claude's implementation plans. See phases, tasks, and completion rates.

```
┌─────────────────────────────────────────────────────────────────┐
│ PLANS                                                           │
├─────────────────────────────────────────────────────────────────┤
│ Auth System Refactor                              ████████░░ 80% │
│   Phase 1: Database Schema                        ✓ Complete     │
│   Phase 2: API Endpoints                          ✓ Complete     │
│   Phase 3: Frontend Integration                   ● In Progress  │
│   Phase 4: Testing                                ○ Pending      │
└─────────────────────────────────────────────────────────────────┘
```

### Web dashboard

The main interface is the progress ledger, with overview, todos, goals and review.
Session records, plans, memory and usage views are available from the additional
navigation menu. The desktop app serves this same dashboard.

`/` focuses search; `Cmd+,` (or `Ctrl+,`) opens settings. The screenshots under
[`docs/assets/`](docs/assets/) show the earlier monitoring dashboard and are
historical references rather than the current desktop design.

---

## CLI Reference

```bash
# Core commands
keepline                      # Start web dashboard (default)
keepline list                 # List all sessions
keepline watch                # Live terminal monitor
keepline recover <id>         # Recover a lost session

# Session management
keepline list -s running      # Filter by status
keepline list -d ./my-app     # Filter by directory

# Memory management
keepline memory list          # List session memories
keepline memory show <id>     # Show memory details
keepline memory export <id>   # Export as recovery context

# Background service
keepline daemon start         # Start background monitor
keepline daemon stop          # Stop daemon
keepline hooks install        # Install Claude Code and Codex lifecycle hooks
```

`keepline web` starts the dashboard only. `keepline daemon start` runs the
background scanner and hook receiver. `keepline hooks install` configures
the forwarding commands and Claude context collector; run `keepline status` or `keepline hooks status` to
check whether a receiver is running.

Installing hooks also captures Claude Code context usage through its `statusLine`
JSON input. Keepline preserves the existing status-line output and settings;
uninstalling hooks restores the original command. Per-session snapshots live in
`KEEPLINE_HOME/context/claude-code/` and feed the dashboard's context ring using
the reported window capacity and latest input/cache token counts. New or resumed
terminal sessions populate this data on their next status-line update. Without a
snapshot, including Desktop sessions, Keepline uses the latest assistant request's
input and cache tokens from the transcript with a 1,000,000-token window. This is
the configured assumption for this setup, not a measured capacity for every model.
Synthetic error messages do not count as usage; compaction clears the old value
until the next request. A current status-line snapshot takes precedence and uses
its reported capacity.

The embedded service performs a full history reconciliation before serving session
and task routes. Startup scans have a five-minute watchdog; subsequent incremental
scans retain a 30-second watchdog. Health remains available at `/api/v1/health`
during reconciliation. If startup fails, operational routes return the scan error
with HTTP 503; the service logs each scan's elapsed time and retries on its scan
interval.

Transcript summaries persist in `KEEPLINE_HOME/session-summaries-v1.sqlite` so
fresh scan processes can reuse unchanged files. The first scan still reads the
full archive. File identity, size, modification time and change time invalidate
cached summaries; files changing during parsing are not cached. Process matching
and session status reconciliation always run again, and full tool-call details
are parsed on demand. Scan completion logs include cache hits and misses.
This database is derived data; parser semantic changes must bump the cache
version in `src/infrastructure/session-summary-cache.ts`.


---

## How it works

1. Process scanners discover Codex and Claude Code processes; adapters parse their
   local session records.
2. The session aggregator reconciles process observations with transcript activity.
   Startup reconciliation completes before business routes become available.
3. The ledger associates requests, requirements and recorded execution evidence;
   SQLite stores the local state and explicit acceptance decisions.
4. The Hono API and WebSocket feed the React dashboard and desktop app. Hooks can
   provide additional live events.

[Architecture import tests](src/__tests__/architecture-imports.test.ts) enforce
layer boundaries. `/api/v1` is the stable local
API used by the Swift SDK; see the [recovery OpenAPI contract](docs/local-api-v1-recovery.openapi.yaml).
`/api` also serves the web dashboard.

---

## Configuration

Keepline stores data in `~/.keepline/` by default:

```
~/.keepline/
├── keepline.db    # SQLite database
├── config.json      # Configuration
└── logs/            # Log files
```

### Options

```json
{
  "scanInterval": 5000,
  "hookPort": 7890,
  "webPort": 3377,
  "logLevel": "info"
}
```

Environment variables that describe process or security boundaries, such as
`KEEPLINE_HOME`, `KEEPLINE_HOST`, `KEEPLINE_PUBLIC_ORIGIN`,
`KEEPLINE_ALLOWED_ORIGINS`, and `KEEPLINE_TRUST_PROXY`, remain read at their
runtime boundary instead of being persisted into `config.json`.

---

## API

REST API available at `http://localhost:3377/api`

| Endpoint | Description |
|----------|-------------|
| `GET /sessions` | List all sessions |
| `GET /sessions/:id` | Session details |
| `POST /sessions/:id/recover` | Recover session |
| `GET /memory` | List memories |
| `GET /memory/:id/context` | Get recovery context |
| `GET /plans` | List plans |
| `GET /usage` | Cost analytics |
| `WS /ws` | Real-time updates |

---

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md).

```bash
# Development
bun run dev          # Watch mode
bun run typecheck    # Type check
bun run test         # Run isolated backend tests
```

---

## Next steps and release status

The source package, CLI, local service and desktop candidate report **1.2.0**.
This is a source version, not a claim that the candidate is signed, notarized,
publicly released or accepted in native testing. Follow the
[verification record](specs/progress-ledger/verification.md) and
[desktop instructions](menubar-tauri/README.md) for those separate gates.
The [2026-10-19 maintenance record](specs/progress-ledger/maintenance-20261019.md)
records the current source sync, local checks and outstanding release gates.

The next product step is the [native agent workbench](https://github.com/majiayu000/keepline/tree/feat/agent-workbench/specs/terminal-workbench): start and switch real
agent CLIs inside the app, answer requests and continue work. It is being built on
the `feat/agent-workbench` branch ([#147](https://github.com/majiayu000/keepline/pull/147)) and is not part of this branch.

---

## Support

- [GitHub Issues](https://github.com/majiayu000/keepline/issues)

---

<div align="center">

**Built for local agent runtime power users**

[GitHub](https://github.com/majiayu000/keepline) | [Source setup](#quick-start) | [CLI reference](#cli-reference)

MIT License

</div>

---

## The Agent Infra Stack

This project is one layer of an open-source stack for running coding agents (Claude Code, Codex) as serious infrastructure. Every piece works standalone; together they close the loop:

`keepline` is the **Keep** layer: compare requests with recorded work, accept results, and return to the right session.

| Layer | Project | What it does |
|---|---|---|
| Extend | [claude-skill-registry](https://github.com/majiayu000/claude-skill-registry) | Discover and search community Claude Code skills |
| Extend | [spellbook](https://github.com/majiayu000/spellbook) | Cross-runtime skills for Claude Code, Codex, and multi-agent workflows |
| Trust | [argus](https://github.com/majiayu000/argus) | Static install-time scanner for supply-chain attacks (npm / PyPI / crates.io) |
| Trust | [vibeguard](https://github.com/majiayu000/vibeguard) | Rules, hooks, and guards against hallucinated or unverified agent changes |
| Remember | [remem](https://github.com/majiayu000/remem) | Local-first persistent memory for Claude Code and Codex sessions |
| Orchestrate | [harness](https://github.com/majiayu000/harness) | Rust agent orchestration platform — rules, skills, GC, observability |
| Route | [litellm-rs](https://github.com/majiayu000/litellm-rs) | High-performance Rust AI gateway — 100+ LLM APIs via OpenAI format |
| Keep | [keepline](https://github.com/majiayu000/keepline) **◀ you are here** | Session command center — monitor, recover, never lose agent work |
