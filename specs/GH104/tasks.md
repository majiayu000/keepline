# Adapter Boundary And Architecture Docs Task Plan

Issue: https://github.com/majiayu000/keepline/issues/104

Product spec: `specs/GH104/product.md`
Tech spec: `specs/GH104/tech.md`

## Tasks

### SP104-T1: Audit adapter service imports

Done when each `src/adapters/**` import from `services` is classified as moved,
allowed composition, or removed.

Verify:

```sh
rg -n "from ['\\\"].*services" src/adapters
```

### SP104-T2: Enforce architecture boundary

Done when the architecture import test catches unapproved adapter-to-service
imports.

Verify:

```sh
bun test src/__tests__/architecture-imports.test.ts
```

### SP104-T3: Align docs

Done when `AGENTS.md` and `CLAUDE.md` match the current source tree.

Verify:

```sh
bun run typecheck
```

