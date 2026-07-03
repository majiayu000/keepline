# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Keepline** is the command center for agent CLI power users. It monitors Codex and Claude Code sessions across your system, tracks token usage and costs where supported, recovers lost sessions when terminals crash, and preserves context across iterations.

**Tagline:** "Never lose your Codex or Claude Code work again."

## Build & Development Commands

```bash
# Build the CLI (Bun)
bun run build

# Development - run CLI directly
bun run dev

# Type check
bun run typecheck

# Run the built CLI
bun run start

# Web client (React + Vite)
cd src/web/client
bun run dev          # Dev server with HMR
bun run build        # Production build (outputs to dist/)
bun run typecheck    # Type check client code
```

## CLI Commands

```bash
keepline                        # Start web dashboard (default)
keepline list                   # List sessions with options
keepline watch                  # Live terminal monitor
keepline recover [session-id]   # Recover a lost session
keepline daemon start|stop      # Background daemon
keepline hooks install|status   # Manage Claude hooks
keepline sync                   # Manual session sync
keepline web                    # Start web UI (port 3377)
keepline memory list|show|edit  # Session memory management
```

## Architecture

Uses a **layered DDD architecture**:

```
src/
├── index.ts              # CLI entry point (Commander.js)
├── adapters/             # External integrations
│   ├── claude/           # Claude data parsing (JSONL, plans)
│   ├── codex/            # Codex data parsing (JSONL, plans)
│   ├── process/          # System process scanning
│   ├── runtimes/         # Runtime registry and adapter metadata
│   └── hook/             # Local hook HTTP server
├── cli/                  # CLI commands
├── db/                   # Database compatibility and reset helpers
├── domain/               # Domain entities (DDD)
│   ├── session/          # Session management
│   ├── recovery/         # Recovery system
│   ├── runtime/          # Runtime identity contracts
│   ├── work-item/        # Work item entities
│   └── memory/           # Cross-session memory ("relay race")
├── infrastructure/       # Database, events, repositories
├── lib/                  # Utilities (logger, config, paths)
├── services/             # Business services
├── ui/                   # Terminal UI (Ink/React)
└── web/
    ├── api/              # Hono HTTP API
    └── client/           # React + Vite frontend
```

**Key data flow:**
1. `adapters/process/scanner.ts` scans system for agent processes
2. `adapters/claude/parser/jsonl.ts` and `adapters/codex/parser.ts` parse session files
3. `services/session.aggregator.ts` merges process + file state
4. `infrastructure/database/repositories/` persists to SQLite
5. `adapters/hook/server.ts` receives real-time events

Adapter-to-service imports are intentionally narrow and enforced by
`src/__tests__/architecture-imports.test.ts`. New adapter dependencies on
`src/services/` must either move the shared logic to a lower layer or be added
to that reviewed allowlist with a clear composition reason.

## Tech Stack

- **Runtime**: Bun
- **CLI Framework**: Commander.js
- **Terminal UI**: Ink (React for CLI)
- **Web Backend**: Hono
- **Web Frontend**: React 18 + Vite + TypeScript
- **Database**: SQLite (via Bun's built-in SQLite)

## Key Features

1. **Multi-session Monitoring** - Track all Claude Code instances
2. **Session Recovery** - Three methods: resume, continue, new
3. **Cost Analytics** - Cache token support, multi-model pricing
4. **Cross-session Memory** - "Relay race" pattern for context persistence
5. **Plans Tracking** - Parse and display Claude implementation plans
6. **Web Dashboard** - 5 cyberpunk themes, real-time WebSocket updates

## Session States

- **running** - Active process with recent activity
- **waiting** - Process waiting for user input
- **idle** - Process idle but still alive
- **lost** - Process terminated unexpectedly (recoverable)
- **completed** - Session finished normally
