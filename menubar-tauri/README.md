# Keepline desktop

Keepline 1.2.0 is a local macOS agent progress dashboard with a menu-bar panel.
This source candidate has not been published merely because its version is 1.2.0.
Release downloads must be checked against their own tag and release notes.

## What it does

- Shows Codex and Claude Code sessions, requests that need you, and recent replies.
- Links requirements, execution evidence, goals and todos; acceptance stays explicit.
- Opens the dashboard from a menu-bar session, with focus mode and opt-in notifications.
- Embeds the Bun service and uses the same state semantics in the tray and dashboard.
- Reports unknown process state honestly; missing scan evidence is not proof of exit.

## Build from source

Requires macOS, Bun, Node/npm, Rust and Xcode command-line tools.
From the repository root:

```bash
bun install --frozen-lockfile
(cd src/web/client && bun install --frozen-lockfile)
(cd menubar-tauri && npm ci)
(cd menubar-tauri && npm run tauri -- build --bundles app)
```

Tauri's build hook builds the dashboard and embeds the service automatically.
The app is written under `menubar-tauri/src-tauri/target/release/bundle/macos/`.
Local builds use ad-hoc signing by default. Developer ID signing, notarization
and public release are separate delivery steps; see the release workflow.

## First run

1. Open Keepline and wait for the initial scan. Open the main panel from the tray.
2. In **设置 → 连接 agent**, click **连接实时事件**. Installation updates the Codex
   and Claude Code hook configuration and preserves unrelated commands.
3. Trust the hook in Codex, or restart Claude Code, then run one turn.
4. Check each runtime's status: configuration installed, receiver running and an
   actual received event are separate states. A configured hook alone is not verified.
5. Review a session's requirements and evidence, then explicitly accept or follow up.

Transcript scanning remains available without hooks. A scan or receiver failure
is shown in the dashboard; stale data is not presented as a healthy empty state.
System notifications are off by default and can be enabled in settings.

The legacy quota frontend remains in this directory for historical code context;
the desktop build serves the progress dashboard from `src/web/client`.

## Checks

```bash
bun run typecheck
bun test
(cd src/web/client && bun run test:ui)
bun run scripts/build-ledger-app.ts
(cd menubar-tauri/src-tauri && cargo test --lib)
```

Browser tests mock the native bridge. Passing them does not replace native app
launch, menu-bar, notification, login-item or signing/notarization verification.
