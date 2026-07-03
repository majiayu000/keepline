# Shared Session Status Presentation Tech Spec

Product spec: `specs/GH103/product.md`

Issue: https://github.com/majiayu000/keepline/issues/103

## Implementation Scope

- `src/domain/session/value-objects.ts`
- new shared presentation helper under `src/domain/session/`
- `src/web/client/src/constants/index.ts`
- `src/ui/views/*`
- focused tests

## Design

Add a shared status presentation module exporting ordered status metadata:

- status id;
- default label;
- short label;
- semantic group;
- recovery/action hint where already used.

Web and Ink can map colors/icons locally, but labels and ordering should come
from the shared module. Tests should assert every `SESSION_STATUSES` entry has
metadata and every UI mapping covers the same set.

## Verification

```sh
bun test src/__tests__/runtime-status.test.ts
bun run typecheck
```

## Risks And Rollback

Importing shared TypeScript into the Vite client must preserve existing path
resolution. Rollback is a helper and import revert.

