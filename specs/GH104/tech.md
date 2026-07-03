# Adapter Boundary And Architecture Docs Tech Spec

Product spec: `specs/GH104/product.md`

Issue: https://github.com/majiayu000/keepline/issues/104

## Implementation Scope

- `src/adapters/runtimes/*`
- `src/adapters/hook/server.ts`
- `src/__tests__/architecture-imports.test.ts`
- `AGENTS.md`
- `CLAUDE.md`

## Design

Keep adapter imports to low-level shared helpers or domain contracts. Where an
adapter currently imports a service only for a small pure helper, move that pure
helper to a lower-level module such as `domain` or `lib`. Keep hook server
service orchestration documented as an allowed edge if it remains the HTTP
composition point.

Add or update the architecture import test to fail on unapproved
`src/adapters/** -> src/services/**` imports.

## Verification

```sh
bun test src/__tests__/architecture-imports.test.ts
bun run typecheck
```

## Risks And Rollback

Overly strict static checks can block legitimate composition code. Keep the
allowlist explicit and small.

