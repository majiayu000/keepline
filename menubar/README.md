# Claude Quota Menubar

A macOS menubar application for monitoring Claude usage quota.

## Features

- **5-Hour Session Limit**: Track your current session usage
- **7-Day Weekly Limits**: Monitor Opus and Sonnet usage separately
- **Status Indicators**: Good / Warning / Critical levels
- **Reset Countdown**: See when limits will reset
- **Auto-refresh**: Updates every 30 seconds

## Prerequisites

- Claude Code must be installed and logged in (the app reads OAuth tokens from macOS Keychain)
- Node.js 18+

## Installation

```bash
cd menubar
npm install
```

## Development

```bash
# Start development mode (hot reload)
npm run dev

# In another terminal, start Electron
npm start
```

## Build

```bash
# Build the application
npm run build

# Package for distribution
npm run package
```

## How it Works

1. Reads OAuth token from macOS Keychain (`Claude Code-credentials`)
2. Calls Anthropic's OAuth usage API (`/api/oauth/usage`)
3. Displays quota information in a menubar dropdown

## Icon Setup

Place the following icons in the `assets/` folder:
- `iconTemplate.png` - 16x16 or 22x22 menubar icon (use "@2x" suffix for retina)
- `icon.icns` - App icon for macOS (for distribution)

For menubar icons, use "Template" naming convention so macOS handles dark/light mode automatically.

## Configuration

The app automatically:
- Refreshes every 30 seconds when visible
- Supports macOS dark/light mode
- Shows the last update time

## Troubleshooting

**"OAuth token not found"**
- Make sure Claude Code is installed
- Log into Claude Code: `claude login`

**"Token may be expired"**
- Log out and back into Claude Code
- Run `claude logout` then `claude login`
