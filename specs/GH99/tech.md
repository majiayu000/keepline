# LanceDB Vector Dimension And Bulk Maintenance Tech Spec

Product spec: `specs/GH99/product.md`

Issue: https://github.com/majiayu000/keepline/issues/99

## Implementation Scope

- `src/infrastructure/vector/lancedb.adapter.ts`
- `src/infrastructure/vector/types.ts`
- `src/__tests__/lancedb.adapter.test.ts`

## Design

The vector adapter owns the final persistence boundary, so it must validate
vector dimensions even when callers bypass the embedding service. The adapter
should:

1. validate each vector against `VectorStoreConfig.embeddingDimension`;
2. detect the opened table's stored vector length using a cheap sampled row when
   possible;
3. cache the detected table dimension after initialization;
4. fail before writes when configured and table dimensions differ;
5. use LanceDB native helpers for `count()` and predicate deletion when present.

If LanceDB SDK methods differ across versions, the adapter may use defensive
feature detection, but missing native support must still fail clearly for
session deletion instead of silently falling back to full-table filtering.

## Verification

```sh
bun test src/__tests__/lancedb.adapter.test.ts
bun run typecheck
```

## Risks And Rollback

The main compatibility risk is SDK method shape drift. Keep method access
localized in the adapter and covered by tests with mocked table objects. Rollback
is limited to reverting the adapter change and tests.

