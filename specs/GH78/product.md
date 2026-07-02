# High-Cost Notification Product Spec

Issue: https://github.com/majiayu000/keepline/issues/78

## Summary

Keepline's high-cost notification must fire when a session crosses the user's configured cost threshold. The session list and realtime dashboard paths currently use the basic session payload, so that payload must preserve persisted usage data required by notifications.

## User Problem

Users can enable "High Cost Warning" and set a dollar threshold, but the notification never fires because the list and WebSocket payloads omit `usageStats`. A session can exceed the configured threshold while the dashboard keeps seeing `session.usageStats === undefined`.

## Product Behavior

1. Basic session responses include persisted `usageStats` when the database has token or cost data.
2. Basic session responses continue to omit heavy transcript/detail fields such as `initialPrompt`, `lastMessage`, tool input, and current file.
3. WebSocket session updates consider usage changes as meaningful changes so cost-only updates are broadcast.
4. The client session version signature considers usage changes so notification checks run when cost changes.
5. High-cost notifications fire once when a session cost crosses from below the configured threshold to at or above that threshold.
6. Missing usage data remains missing; Keepline must not infer or fabricate costs.

## Non-Goals

- Do not fetch full session details only to power high-cost notifications.
- Do not change pricing extraction or cost calculation.
- Do not add a new notification settings UI.
- Do not emit repeated high-cost notifications while the session remains above the same threshold.

## Acceptance Criteria

1. `GET /api/sessions?fields=basic` returns `usageStats` for sessions with persisted usage data.
2. Basic serialization still excludes heavy session detail fields.
3. Realtime dashboard updates broadcast when persisted usage cost or token totals change.
4. Client session versioning changes when usage cost or token totals change.
5. A regression test with a mocked high-cost session proves the high-cost notification event is produced when the cost crosses the threshold.
6. Verification passes: focused tests, `bun run typecheck`, `bun test`, and `bun run build`.
