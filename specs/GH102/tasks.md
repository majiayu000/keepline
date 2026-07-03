# EventStore Lifecycle Task Plan

Issue: https://github.com/majiayu000/keepline/issues/102

Product spec: `specs/GH102/product.md`
Tech spec: `specs/GH102/tech.md`

## Tasks

### SP102-T1: Audit EventStore surface

Done when declarations, migrations, exports, retention, reset, and callers are
listed in implementation notes.

Verify:

```sh
rg -n "EventStore|events" src docs specs
```

### SP102-T2: Implement retain or remove decision

Done when the chosen lifecycle is consistent across code and docs.

Verify:

```sh
bun test src/__tests__/reset-database.test.ts src/__tests__/retention.service.test.ts
```

### SP102-T3: Run deterministic checks

Verify:

```sh
bun run typecheck
bun test
```

