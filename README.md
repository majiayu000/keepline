<div align="center">

```
 ▀█▀ ▄▀█ █▀ █▄▀ █▀▀ █▀█
  █  █▀█ ▄█ █ █ ██▄ █▀▄
```

# Claude Code Monitor

**Never lose a Claude Code session again.**

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Bun](https://img.shields.io/badge/runtime-Bun-f9f1e1.svg)](https://bun.sh)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0-blue.svg)](https://www.typescriptlang.org/)
[![Tests](https://img.shields.io/badge/tests-135%20passed-brightgreen.svg)]()

</div>

---

<div align="center">

### The Problem

*You're deep in a Claude Code session. Terminal crashes. Session lost. Hours of context—gone.*

### The Solution

**Tasker** monitors all your Claude Code sessions, tracks their status in real-time, and lets you recover lost sessions with a single command.

</div>

---

## Demo

<div align="center">

### Terminal UI
```
 ▀█▀ ▄▀█ █▀ █▄▀ █▀▀ █▀█  ║  CLAUDE CODE MONITOR v1.0
 ──────────────────────────────────────────────────────────────────────

 SYS::NODES[47]  ● LIVE:3  ● WAIT:2  ● IDLE:14  ● DEAD:28


 ▌ACTIVE [5]
 [01] ▶ EXEC  tasker           Implementing Projects View...           now
 [02] ▶ EXEC  my-api           Fix authentication bug                  2m
 [03] ◆ WAIT  frontend         Waiting for user input...               5m

 ▌STANDBY [14]
 [04] ◇ IDLE  data-pipeline    ETL optimization complete              15m
 [05] ◇ IDLE  ml-model         Training finished                       1h

 ▌DISCONNECTED [28]
 [06] ✕ LOST  old-project      Session terminated                      2h
      → tasker recover 06

 ──────────────────────────────────────────────────────────────────────
 CMD:: list=tasker ls  │  watch=tasker w  │  recover=tasker r <n>
```

### Web Dashboard

| Sessions View | Projects View | Analytics |
|:-------------:|:-------------:|:---------:|
| ![Sessions](docs/assets/sessions.png) | ![Projects](docs/assets/projects.png) | ![Analytics](docs/assets/analytics.png) |

*5 cyberpunk themes included: Neon, Matrix, Synthwave, Minimal, Tokyo*

</div>

---

## Features

<table>
<tr>
<td width="50%">

### 🔍 **Real-time Monitoring**
- Track all Claude Code sessions system-wide
- Live status updates (running/waiting/idle/lost)
- CPU & memory usage per session
- Sub-agent tracking for Task tool calls

</td>
<td width="50%">

### 💰 **Cost Tracking**
- Real-time token counting
- Automatic cost calculation
- Per-session and aggregate stats
- Supports all Claude models (3.x, 4.x, 4.5)

</td>
</tr>
<tr>
<td width="50%">

### 🔧 **Tool Visibility**
- See which tools Claude is using
- Track file edits, bash commands, searches
- Tool call history with timestamps
- Input/output inspection

</td>
<td width="50%">

### 🚀 **Session Recovery**
- Detect crashed/lost sessions
- One-command recovery
- Preserve full conversation context
- Never lose work again

</td>
</tr>
<tr>
<td width="50%">

### 📊 **Projects View** <sup>NEW</sup>
- Group sessions by project directory
- Bird's-eye view of all active projects
- Quick filtering and navigation
- Aggregate stats per project

</td>
<td width="50%">

### 📈 **Analytics Dashboard**
- Token usage over time
- Cost trends and forecasts
- Tool usage distribution
- Session status breakdown

</td>
</tr>
</table>

---

## Quick Start

```bash
# Install
git clone https://github.com/majiayu000/Claude-Code-Monitor.git
cd Claude-Code-Monitor
bun install

# Build
bun run build

# Run
bun run start web
```

Then open **http://localhost:3377**

---

## Usage

### CLI Commands

```bash
tasker                    # List all sessions (default)
tasker list               # List with options
tasker watch              # Live monitor mode
tasker recover <id>       # Recover a lost session
tasker web                # Start web UI (port 3377)
tasker daemon start       # Run as background service
tasker sync               # Manual session sync
```

### Keyboard Shortcuts (Web UI)

| Key | Action |
|-----|--------|
| `r` | Refresh sessions |
| `s` | Sync from Claude |
| `/` | Focus search |
| `t` | Cycle theme |
| `1-5` | Switch theme directly |
| `?` | Show help |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         TASKER                                   │
├─────────────┬─────────────┬─────────────┬─────────────┬─────────┤
│   CLI UI    │   Web UI    │   Daemon    │   Hooks     │   API   │
│   (Ink)     │  (React)    │  (Background)│  (HTTP)    │ (Hono)  │
├─────────────┴─────────────┴─────────────┴─────────────┴─────────┤
│                      Session Service                             │
├─────────────┬─────────────┬─────────────┬───────────────────────┤
│  Process    │   Claude    │   Storage   │   Recovery            │
│  Scanner    │   Parser    │   (SQLite)  │   Engine              │
└─────────────┴─────────────┴─────────────┴───────────────────────┘
```

### Tech Stack

- **Runtime**: [Bun](https://bun.sh) - Fast all-in-one JavaScript runtime
- **CLI**: [Commander.js](https://github.com/tj/commander.js) + [Ink](https://github.com/vadimdemedes/ink)
- **Web**: [Hono](https://hono.dev) + [React](https://react.dev) + [Vite](https://vitejs.dev)
- **Database**: SQLite (Bun native)
- **Charts**: [Recharts](https://recharts.org)

---

## API Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sessions` | GET | List all sessions with stats |
| `/api/sessions/:id` | GET | Get session details |
| `/api/sessions/:id/tools` | GET | Get tool call history |
| `/api/sessions/:id/subagents` | GET | Get sub-agent sessions |
| `/api/sessions/:id/stop` | POST | Stop a running session |
| `/api/sessions/:id/recover` | POST | Recover a lost session |
| `/api/sessions/:id/complete` | POST | Mark as completed |
| `/api/sync` | POST | Sync all sessions |
| `/api/stats` | GET | Get aggregate statistics |

---

## Configuration

Tasker stores data in `~/.tasker/`:

```
~/.tasker/
├── tasker.db          # SQLite database
├── config.json        # User configuration
└── logs/              # Application logs
```

### Claude Hooks Integration

```bash
# Install hooks for real-time updates
tasker hooks install

# Check hook status
tasker hooks status
```

---

## Development

```bash
# Development mode (hot reload)
bun run dev

# Type checking
bun run typecheck

# Run tests
bun test

# Build for production
bun run build
```

---

## Roadmap

- [x] Session monitoring & recovery
- [x] Web dashboard with themes
- [x] Token usage & cost tracking
- [x] Projects View (group by directory)
- [x] Sub-agent tracking
- [x] Real-time WebSocket updates
- [ ] OpenTelemetry integration
- [ ] Webhook notifications (Slack/Discord)
- [ ] Session timeline/replay
- [ ] Team collaboration features

See [FEATURE_ROADMAP.md](docs/FEATURE_ROADMAP.md) for detailed plans.

---

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

```bash
# Fork the repo
# Create your feature branch
git checkout -b feature/amazing-feature

# Commit your changes
git commit -m 'Add amazing feature'

# Push to the branch
git push origin feature/amazing-feature

# Open a Pull Request
```

---

## Related Projects

- [Claude Code](https://claude.ai/code) - Anthropic's AI coding assistant
- [LiteLLM](https://github.com/BerriAI/litellm) - LLM pricing data source

---

<div align="center">

## License

MIT © 2024

---

**If Tasker saved your session, consider giving it a ⭐**

</div>
