# Hook Server Request Security Tech Spec

Product spec: `specs/GH79/product.md`

Issue: https://github.com/majiayu000/keepline/issues/79

## Context

- `src/adapters/hook/server.ts` builds a standalone Fastify server bound to `127.0.0.1`.
- The hook server exposes `/hook`, `/context`, `/compression/stats`, and `/health`.
- `src/web/api/request-security.ts` already exposes request security helpers:
  - `isLoopbackHostHeader`
  - `isLoopbackOrigin`
  - `isAllowedFetchMetadata`
- Fastify request headers are plain Node headers, so the hook server needs a small adapter for header lookup.

## Design

Add a hook-server request gate:

```ts
function isAllowedHookServerRequest(request: FastifyRequest): boolean
```

Rules:

1. `Host` must pass `isLoopbackHostHeader`.
2. If `Origin` is present, it must pass `isLoopbackOrigin`.
3. If `Sec-Fetch-Site` is present, it must be `same-origin`, `same-site`, or `none`.

Register the gate with Fastify `onRequest` before all route handlers. Reject denied requests with 403 and a generic `Forbidden` response.

Extract server construction into `createHookServer()` so tests can use Fastify `inject()` without binding a real port. `startHookServer()` should continue to call the same route registration path.

## Verification Plan

- Add `src/__tests__/hook.server.test.ts`:
  - rejects non-loopback Host for `/hook`
  - rejects cross-origin `/context`
  - accepts loopback `/health`
  - allows loopback `/hook` to reach payload validation and return 400
- Run:
  - `bun test src/__tests__/hook.server.test.ts`
  - `bun run typecheck`
  - `bun test`
  - `bun run build`

## Rollback

- Remove the Fastify `onRequest` gate and `createHookServer()` extraction.
- Remove the focused hook server security tests.
