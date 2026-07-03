# Web Mode Hook Availability Tech Spec

Product spec: `specs/GH101/product.md`

Issue: https://github.com/majiayu000/keepline/issues/101

## Implementation Scope

- `src/cli/status.ts`
- `src/web/api/routes/status.ts` if present, or a small status route under
  existing API routing
- `src/adapters/hook/server.ts`
- `src/adapters/hook/installer.ts`
- `README.md`
- `README_CN.md`
- focused tests

## Design

Do not make `keepline web` start the hook server in this tranche. Instead expose
an explicit hook availability contract:

- installed: read from hook installer status;
- receiverRunning: read from hook server runtime state or a local loopback
  health probe;
- degraded: installed is true and receiverRunning is false.

The CLI status command and Web API should use the same helper so labels do not
drift.

## Verification

```sh
bun test src/__tests__/hook.server.test.ts src/__tests__/hook.installer.test.ts
bun run typecheck
```

## Risks And Rollback

The helper must not probe external hosts. Rollback is limited to removing the
status helper and UI/API fields.

