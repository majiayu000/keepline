# Claude Quota Monitor

A macOS menubar app to monitor Claude Code and Codex (ChatGPT) usage quotas and local cost estimates.

## Features

- **Claude Code Quota**: View your 5-hour and 7-day usage limits
- **Codex Quota**: View your ChatGPT Plus/Pro/Team usage limits
- **Local Cost Tracking**: View estimated today, week, and month costs from local Claude/Codex logs
- **Tab Switching**: Easily switch between Claude and Codex views
- **Tray Icon**: Shows remaining quota percentage for the active tab
- **Multiple Themes**: 7 beautiful themes including light, dark, and Claude brand
- **Lazy Loading**: Only loads data when you switch to a tab

## Installation

### Download

Download the latest `.dmg` file from the [Releases](https://github.com/majiayu000/keepline/releases) page.

### macOS Gatekeeper

Public GitHub Release DMGs are Developer ID signed and notarized. After
installing from Releases, macOS should open the app without removing quarantine
attributes.

Local `npm run tauri build` output is unsigned unless you set the Apple signing
environment variables. Those local builds may show a Gatekeeper warning; that
does not apply to the notarized Release artifacts.

## Requirements

- **macOS 10.15+** (Catalina or later)
- **Claude Code**: Must be logged in (`claude` command in terminal)
- **Codex**: Must be logged in (`codex` command in terminal)

## Usage

1. Click the menubar icon to open the quota panel
2. Use the tabs to switch between Claude and Codex
3. Review live quota windows and local cost estimates in the active tab
4. The tray icon shows the remaining quota % for the active tab
5. Click outside the panel to close it
6. Right-click the tray icon to quit

## Local Codex history statistics

The Rust menubar backend reads `history.jsonl` through `agent-sessions`. Set
`CODEX_HOME` to choose its history directory; otherwise it uses `~/.codex`.
An empty override is reported as an error. Authentication and quota requests
retain their existing configuration.

The API fields `totalSessions` and `todaySessions` count history entries, including
repeated entries for the same session. Daily counts use UTC. Valid JSON entries
with missing or invalid optional fields still count toward the total. Malformed
JSON, an incomplete last record, invalid UTF-8, or an I/O error clears the result
and reports an error instead of returning partial statistics. A missing history
file means no history yet.

The Bun session adapters continue to parse full transcripts independently; their
existing synthetic fixtures and the shared library's format documentation cover
the overlapping formats. History entries are a different source from transcript
messages and should not be combined into one session count.

## Migration candidate status

This branch targets registry releases `agent-sessions 0.2.0` and `ccstats 0.9.0`.
Integration checks currently use local Cargo patches for those release candidates.
Publish the upstream crates and refresh the registry lockfile before releasing
this menubar build; local checks alone do not establish registry availability.

## Building from Source

```bash
# Install dependencies
npm install

# Development
npm run tauri dev

# Build
npm run tauri build
```

## Tech Stack

- **Frontend**: React 19 + TypeScript + Vite
- **Backend**: Rust + Tauri 2
- **APIs**:
  - Claude: `api.anthropic.com/api/oauth/usage`
  - Codex: `chatgpt.com/backend-api/wham/usage`
- **Local Cost SDK**: `ccstats` reads Claude and Codex local logs for token and cost summaries

## License

MIT
