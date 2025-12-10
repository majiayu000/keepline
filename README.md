# Tasker

Claude Code session monitor and recovery tool with a cyberpunk-themed web UI.

## Features

- **Session Monitoring** - Track all active Claude Code sessions across your system
- **Process Management** - View running processes, stop sessions, recover lost sessions
- **Token Usage Tracking** - Real-time token counting and cost calculation
- **Cost Estimation** - Automatic pricing from LiteLLM (supports Claude 3.x, 4.x, 4.5 models)
- **Tool Call Visibility** - See which tools Claude is using in each session
- **Multi-Theme UI** - 5 cyberpunk themes (Neon, Matrix, Sunset, Ocean, Minimal)
- **CLI & Web Interface** - Terminal UI for quick checks, web dashboard for detailed monitoring

## Installation

```bash
# Clone the repository
git clone https://github.com/majiayu000/Claude-Code-Monitor.git
cd Claude-Code-Monitor/tasker

# Install dependencies
bun install

# Build the project
bun run build

# Build the web client
cd src/web/client && bun install && bun run build && cd ../../..
```

## Usage

### Web UI (Recommended)

```bash
# Start the web server
bun run dist/index.js web --port 3377

# Open in browser
open http://localhost:3377
```

### CLI Commands

```bash
# List all sessions
bun run dist/index.js list

# Show session details
bun run dist/index.js show <session-id>

# Interactive TUI
bun run dist/index.js tui

# Sync sessions from Claude Code
bun run dist/index.js sync
```

## Architecture

```
src/
├── claude/          # Claude Code session parsing
│   └── parser/      # JSONL file parser
├── commands/        # CLI command handlers
├── core/            # Core business logic
├── process/         # Process scanning and management
├── recovery/        # Session recovery logic
├── session/         # Session aggregation and stats
├── storage/         # SQLite database layer
├── ui/              # Terminal UI components (Ink)
├── usage/           # Token usage and pricing
│   ├── extractor.ts # Extract usage from JSONL
│   └── pricing.ts   # LiteLLM pricing integration
├── utils/           # Shared utilities
└── web/
    ├── api/         # Hono REST API server
    └── client/      # React frontend
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sessions` | GET | List all sessions with stats |
| `/api/sessions/:id` | GET | Get session details |
| `/api/sessions/:id/tools` | GET | Get tool calls for session |
| `/api/sessions/:id/usage` | GET | Get token usage stats |
| `/api/sessions/:id/stop` | POST | Stop a running session |
| `/api/sessions/:id/recover` | POST | Recover a lost session |
| `/api/sessions/:id/complete` | POST | Mark session as completed |
| `/api/sync` | POST | Sync sessions from Claude Code |

## Token Usage & Pricing

Tasker automatically tracks token usage from Claude Code sessions:

- Extracts `usage` data from JSONL conversation files
- Fetches real-time pricing from [LiteLLM](https://github.com/BerriAI/litellm)
- Caches pricing for 24 hours
- Falls back to default pricing if fetch fails

Supported models include:
- Claude 3 (Opus, Sonnet, Haiku)
- Claude 3.5 (Sonnet, Haiku)
- Claude 4 (Opus, Sonnet)
- Claude 4.5 (Opus, Sonnet)

## Development

```bash
# Run in development mode
bun run dev

# Type checking
bun run typecheck

# Run tests
bun test
```

## Tech Stack

- **Runtime**: [Bun](https://bun.sh)
- **Backend**: [Hono](https://hono.dev) (lightweight web framework)
- **Frontend**: React + Vite + CSS Modules
- **Database**: SQLite (via Bun's built-in SQLite)
- **CLI UI**: [Ink](https://github.com/vadimdemedes/ink) (React for CLI)

## License

MIT
