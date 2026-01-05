# Claude Quota Monitor

A macOS menubar app to monitor your Claude Code and Codex (ChatGPT) usage quotas.

## Features

- **Claude Code Quota**: View your 5-hour and 7-day usage limits
- **Codex Quota**: View your ChatGPT Plus/Pro/Team usage limits
- **Tab Switching**: Easily switch between Claude and Codex views
- **Tray Icon**: Shows remaining quota percentage for the active tab
- **Multiple Themes**: 7 beautiful themes including light, dark, and Claude brand
- **Lazy Loading**: Only loads data when you switch to a tab

## Installation

### Download

Download the latest `.dmg` file from the [Releases](https://github.com/anthropics/claude-hub/releases) page.

### macOS Gatekeeper Warning

Since this app is not notarized by Apple, you may see a warning saying the app is "damaged" or can't be opened. This is normal for unsigned apps.

**To fix this, run one of these commands in Terminal:**

```bash
# Option 1: Remove quarantine from DMG before installing
xattr -cr ~/Downloads/Claude\ Quota_*.dmg

# Option 2: Remove quarantine from installed app
xattr -cr /Applications/Claude\ Quota.app

# Option 3: Allow the specific app (after first launch attempt)
sudo xattr -rd com.apple.quarantine /Applications/Claude\ Quota.app
```

Then try opening the app again.

## Requirements

- **macOS 10.15+** (Catalina or later)
- **Claude Code**: Must be logged in (`claude` command in terminal)
- **Codex**: Must be logged in (`codex` command in terminal)

## Usage

1. Click the menubar icon to open the quota panel
2. Use the tabs to switch between Claude and Codex
3. The tray icon shows the remaining quota % for the active tab
4. Click outside the panel to close it
5. Right-click the tray icon to quit

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

- **Frontend**: React 18 + TypeScript + Vite
- **Backend**: Rust + Tauri 2
- **APIs**:
  - Claude: `api.anthropic.com/api/oauth/usage`
  - Codex: `chatgpt.com/backend-api/wham/usage`

## License

MIT
