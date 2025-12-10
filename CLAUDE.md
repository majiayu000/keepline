# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Tasker** is a CLI tool for monitoring and recovering Claude Code sessions. It tracks multiple Claude Code instances, detects their status (running, waiting, idle, lost), and can recover sessions when terminals crash.

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
tasker                        # List sessions (default)
tasker list                   # List sessions with options
tasker watch                  # Live monitor
tasker recover [session-id]   # Recover a lost session
tasker daemon start|stop      # Background daemon
tasker hooks install|status   # Manage Claude hooks
tasker sync                   # Manual session sync
tasker web                    # Start web UI (port 3377)
```

## Architecture

Uses a **layered DDD architecture**:

```
src/
├── index.ts              # CLI entry point (Commander.js)
├── commands/             # CLI commands (list, watch, recover, daemon, web)
├── core/                 # Domain types, errors, events
├── session/              # Session domain (entity, service, aggregator)
├── claude/               # Claude data parsing (JSONL, history files)
├── process/              # System process scanning & detection
├── storage/              # SQLite database & repositories
├── hook/                 # HTTP server for Claude hooks
├── recovery/             # Session recovery & terminal operations
├── daemon/               # Background process management
├── ui/                   # Terminal UI (Ink/React)
├── utils/                # Logger, config, paths, formatting
└── web/
    ├── api/server.ts     # Hono HTTP API
    └── client/           # React + Vite frontend
```

**Key data flow:**
1. `process/scanner.ts` scans system for Claude processes
2. `claude/parser/jsonl.ts` parses Claude's JSONL conversation files
3. `session/aggregator.ts` merges process state + file state
4. `storage/session.repository.ts` persists to SQLite
5. `hook/server.ts` receives real-time events from Claude hooks

## Tech Stack

- **Runtime**: Bun
- **CLI Framework**: Commander.js
- **Terminal UI**: Ink (React for CLI)
- **Web Backend**: Hono
- **Web Frontend**: React 18 + Vite + TypeScript
- **Database**: SQLite (via Bun's built-in SQLite)

## Key Files

- `src/session/service.ts` - Core session sync logic
- `src/claude/parser/jsonl.ts` - Parses Claude's conversation format
- `src/process/scanner.ts` - Finds Claude processes via `ps`
- `src/web/api/server.ts` - REST API endpoints
- `src/web/client/src/App.tsx` - Web UI root component

## Session States

- **running** - Active process with recent activity
- **waiting** - Process waiting for user input
- **idle** - Process idle but still alive
- **lost** - Process terminated unexpectedly
- **completed** - Session finished normally
