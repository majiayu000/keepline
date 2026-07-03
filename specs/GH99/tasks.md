# LanceDB Vector Dimension And Bulk Maintenance Task Plan

Issue: https://github.com/majiayu000/keepline/issues/99

Product spec: `specs/GH99/product.md`
Tech spec: `specs/GH99/tech.md`

## Tasks

### SP99-T1: Add vector dimension guards

Done when:

- Single inserts reject vectors whose length differs from config.
- Batch inserts reject mixed or incompatible vectors before any write.
- Existing table dimension mismatch raises a clear error.

Verify:

```sh
bun test src/__tests__/lancedb.adapter.test.ts
```

### SP99-T2: Remove full-table bulk maintenance paths

Done when:

- `count()` uses LanceDB native row counting when available.
- `deleteBySessionId()` uses predicate deletion and returns the deleted count.
- Unsupported SDK shapes fail explicitly.

Verify:

```sh
bun test src/__tests__/lancedb.adapter.test.ts
```

### SP99-T3: Run deterministic checks

Verify:

```sh
bun run typecheck
bun test
```

