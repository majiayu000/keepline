<div align="center">

<img src="docs/assets/logo.svg" alt="Claude Hub Logo" width="120" />

# Claude Hub

### Never lose your Claude Code work again.

**The command center for Claude Code power users**

[![npm version](https://img.shields.io/npm/v/claude-hub.svg?style=flat-square&color=00d4ff)](https://www.npmjs.com/package/claude-hub)
[![npm downloads](https://img.shields.io/npm/dm/claude-hub.svg?style=flat-square&color=ff00ff)](https://www.npmjs.com/package/claude-hub)
[![GitHub stars](https://img.shields.io/github/stars/majiayu000/claude-hub?style=flat-square&color=ffcc00)](https://github.com/majiayu000/claude-hub)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)

[Quick Start](#-quick-start) | [Features](#-features) | [Screenshots](#-screenshots) | [Documentation](#-documentation)

<br />

<img src="docs/assets/hero-demo.gif" alt="Claude Hub Demo" width="800" />

</div>

---

## The Problem

You're deep in a coding session with Claude Code. Everything is going great. Then...

- **Terminal crashes** — hours of context, gone
- **Multiple sessions** — which one was working on the auth bug?
- **Cost anxiety** — "How much have I spent today?"
- **Lost progress** — "What was I doing before lunch?"

## The Solution

**Claude Hub** monitors all your Claude Code sessions in real-time, automatically recovers crashed sessions, tracks costs, and preserves context across iterations.

```bash
bunx claude-hub
```

That is it. Open `http://127.0.0.1:3377` and take control.

---

## Why Claude Hub?

|  | Without Claude Hub | With Claude Hub |
|--|-------------------|-----------------|
| **Terminal crash** | Lose all context, start over | One-click recovery with full context |
| **Multiple sessions** | Switch terminals, lose track | See all sessions in one dashboard |
| **Cost tracking** | Check Anthropic console manually | Real-time costs with predictions |
| **Session context** | Gone when terminal closes | Persisted and searchable |
| **Project overview** | Scattered across directories | Aggregated by project |

## Repository Scope

This repository now contains the Claude Hub core application and the in-repo `menubar-tauri` companion only.

Experimental runner workspaces that used to live here have been moved out to the sibling directory `../Claude-Code-Monitor-extracted/` and are no longer part of this repository's build or test surface.

---

## Quick Start

### Option 1: bunx (Recommended)

```bash
bunx claude-hub
```

Requires Bun 1.1+ on your machine.

### Option 2: Install globally

```bash
bun install -g claude-hub
claude-hub web
```

### Option 3: From source

```bash
git clone https://github.com/majiayu000/claude-hub.git
cd claude-hub
bun install && bun run build
bun run start web
```

Open **http://127.0.0.1:3377**

By default the web server binds to loopback only. To expose it intentionally, set `CLAUDE_HUB_HOST`.

If you put Claude Hub behind a reverse proxy (Caddy, nginx, cloudflared), set `CLAUDE_HUB_PUBLIC_ORIGIN=https://your-public-host.example` so the terminal WebSocket accepts the browser's public `Origin`. For multiple public origins, use comma-separated `CLAUDE_HUB_ALLOWED_ORIGINS`.

If you also want the in-process rate limiter to identify real clients behind that proxy, set `CLAUDE_HUB_TRUST_PROXY=true`. Without that flag, `X-Forwarded-For` is treated as untrusted input and the limiter keys on the actual TCP peer instead — this is intentional, so a malicious caller cannot bypass throttling by spoofing forwarded headers.

### macOS Security Note

If you see an error like **"Claude Hub is damaged and can't be opened"** when running the desktop app, this is macOS Gatekeeper blocking unsigned apps — the app is not actually damaged.

**Fix it with:**

```bash
xattr -cr /Applications/Claude\ Hub.app
```

Or if downloaded elsewhere:

```bash
xattr -cr ~/Downloads/Claude\ Hub.app
```

This removes the quarantine attribute that macOS adds to downloaded files.

---

## Features

### Real-time Session Monitoring

Monitor all Claude Code instances across your system. See status, current file, last tool, and activity at a glance.

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

Terminal crashed? Session lost? Recover in seconds with full context preserved.

```bash
claude-hub recover <session-id>
```

Three recovery methods:
- **Resume** — Restore exact session state (recommended)
- **Continue** — New session in same directory
- **New** — Fresh start with original prompt

### Cost Analytics & Predictions

Track spending in real-time. Know exactly where your tokens go.

```
┌─────────────────────────────────────────────────────────────────┐
│ COST ANALYTICS                                                  │
├─────────────────────────────────────────────────────────────────┤
│ Today          $12.34  ▲ 23%                                    │
│ This Week      $67.89                                           │
│ This Month    $198.50  (Projected: $320)                        │
│                                                                 │
│ Cache Savings  $45.20  (38% of total)                           │
│ Your hit rate: 72%                                              │
└─────────────────────────────────────────────────────────────────┘
```

Features:
- Per-session cost breakdown
- Cache token tracking (creation 1.25x, read 0.1x)
- Multi-model support (Opus, Sonnet, Haiku)
- Daily/weekly/monthly trends

### Cross-Session Memory

Claude Hub implements the "relay race" pattern — your progress persists across sessions.

```
┌─────────────────────────────────────────────────────────────────┐
│ SESSION MEMORY                               my-app             │
├─────────────────────────────────────────────────────────────────┤
│ Last Progress: Implemented OAuth2 login flow                    │
│                                                                 │
│ Completed:                                                      │
│   ✓ Database schema design                                      │
│   ✓ User model with bcrypt                                      │
│   ✓ JWT token generation                                        │
│                                                                 │
│ Pending:                                                        │
│   ○ Refresh token rotation                                      │
│   ○ Password reset flow                                         │
│                                                                 │
│ Known Issues:                                                   │
│   ! Token expiry not handled in middleware                      │
└─────────────────────────────────────────────────────────────────┘
```

When you recover a session, this context is automatically injected.

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

### Web Dashboard

Beautiful, cyberpunk-themed dashboard with 5 color themes.

| Sessions | Analytics | Projects |
|:--------:|:---------:|:--------:|
| ![Sessions](docs/assets/Sessions.png) | ![Analytics](docs/assets/Analysis.png) | ![Projects](docs/assets/Projects.png) |

**Keyboard shortcuts:**
- `r` — Refresh sessions
- `s` — Sync from Claude
- `/` — Focus search
- `t` — Cycle themes
- `?` — Show help

**Themes:** Cyberpunk, Matrix, Synthwave, Minimal, Tokyo

---

## CLI Reference

```bash
# Core commands
claude-hub                      # Start web dashboard (default)
claude-hub list                 # List all sessions
claude-hub watch                # Live terminal monitor
claude-hub recover <id>         # Recover a lost session

# Session management
claude-hub list -s running      # Filter by status
claude-hub list -d ./my-app     # Filter by directory

# Memory management
claude-hub memory list          # List session memories
claude-hub memory show <id>     # Show memory details
claude-hub memory export <id>   # Export as recovery context

# Background service
claude-hub daemon start         # Start background monitor
claude-hub daemon stop          # Stop daemon
claude-hub hooks install        # Install Claude hooks
```

---

## How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                        Claude Hub                                │
├─────────────┬─────────────┬─────────────┬─────────────┬─────────┤
│   Web UI    │  Terminal   │   Daemon    │   Hooks     │   API   │
│   (React)   │   (Ink)     │ (Background)│  (HTTP)     │  (REST) │
├─────────────┴─────────────┴──────┬──────┴─────────────┴─────────┤
│                                  │                               │
│  ┌───────────────────────────────▼────────────────────────────┐ │
│  │                    Session Service                          │ │
│  │  • Aggregates process state + file state                   │ │
│  │  • Detects running/waiting/idle/lost sessions              │ │
│  │  • Calculates costs with cache token support               │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                  │                               │
├──────────────────────────────────┼───────────────────────────────┤
│  Process Scanner    │  Claude Parser    │  Memory System         │
│  (ps + /proc)       │  (~/.claude/)     │  (SQLite)              │
└─────────────────────┴───────────────────┴────────────────────────┘
```

**Data flow:**
1. **Process Scanner** — Finds all running `claude` processes
2. **Claude Parser** — Reads JSONL session files from `~/.claude/`
3. **Aggregator** — Merges process state with file state
4. **Memory System** — Persists progress for recovery
5. **Web/CLI** — Presents unified view

---

## Comparison

| Feature | Manual | claude-mem | **Claude Hub** |
|---------|:------:|:----------:|:--------------:|
| Multi-session monitoring | - | - | **Yes** |
| Session recovery | - | - | **3 methods** |
| Cost tracking | - | - | **Yes + predictions** |
| Cache token analysis | - | - | **Yes** |
| Cross-session memory | - | Yes | **Yes** |
| Web dashboard | - | Basic | **5 themes** |
| Plans tracking | - | - | **Yes** |
| Real-time hooks | - | Yes | **Yes** |
| Background daemon | - | - | **Yes** |
| Sub-agent tracking | - | - | **Yes** |

---

## Configuration

Data stored in `~/.claude-hub/`:

```
~/.claude-hub/
├── claude-hub.db    # SQLite database
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
bun test             # Run tests
```

---

## Roadmap

- [ ] Cost alerts & budgets
- [ ] Team dashboard
- [ ] Cursor/Copilot support
- [ ] Session replay/timeline
- [ ] Plugin system

---

## Support

- [GitHub Issues](https://github.com/majiayu000/claude-hub/issues)
- [Discussions](https://github.com/majiayu000/claude-hub/discussions)

---

<div align="center">

**Built for Claude Code power users**

[GitHub](https://github.com/majiayu000/claude-hub) | [npm](https://www.npmjs.com/package/claude-hub) | [Documentation](https://claude-hub.dev)

MIT License

</div>
