# Hook Server Request Security Product Spec

Issue: https://github.com/majiayu000/keepline/issues/79

## Summary

The local hook HTTP server must reject browser-originated or DNS-rebound requests that do not target a loopback host. Hook events and context reads are local automation surfaces, not public browser APIs.

## User Problem

Keepline's main web server validates local Host and browser request metadata, but the hook server accepts every Host, Origin, and browser fetch metadata value. A malicious browser page can attempt DNS-rebinding or cross-origin requests against the local hook port to forge hook events or read context data.

## Product Behavior

1. The hook server only accepts requests whose `Host` header names a loopback host such as `127.0.0.1`, `localhost`, or `[::1]`.
2. Requests with a non-loopback `Host` are rejected with 403 before route handlers run.
3. Requests with an `Origin` header must have a loopback origin.
4. Requests with browser Fetch Metadata must be `same-origin`, `same-site`, or `none`.
5. CLI/Claude hook requests without `Origin` or `Sec-Fetch-Site` remain supported when their Host header is loopback.
6. The `/context` endpoint is protected by the same request gate as `/hook`, `/health`, and `/compression/stats`.

## Non-Goals

- Do not introduce a shared hook token in this tranche.
- Do not change hook event payload semantics.
- Do not make the hook server remotely accessible.
- Do not change the configured default hook port.

## Acceptance Criteria

1. A `POST /hook` request with `Host: attacker.example` receives 403.
2. A `GET /context` request with loopback Host but cross-origin `Origin` receives 403.
3. A `GET /health` request with loopback Host remains accepted.
4. A loopback `POST /hook` with an invalid body reaches validation and returns 400 rather than being rejected by the security gate.
5. Focused tests, `bun run typecheck`, `bun test`, and `bun run build` pass.
