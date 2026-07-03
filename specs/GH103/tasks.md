# Shared Session Status Presentation Task Plan

Issue: https://github.com/majiayu000/keepline/issues/103

Product spec: `specs/GH103/product.md`
Tech spec: `specs/GH103/tech.md`

## Tasks

### SP103-T1: Add shared status metadata

Done when all `SESSION_STATUSES` have one shared metadata source.

Verify:

```sh
bun test src/__tests__/runtime-status.test.ts
```

### SP103-T2: Consume shared labels in Web and Ink

Done when Web constants and Ink views no longer hardcode conflicting labels for
the same status semantics.

Verify:

```sh
bun run typecheck
```

### SP103-T3: Add coverage tests

Done when a missing status mapping fails tests.

Verify:

```sh
bun test src/__tests__/runtime-status.test.ts
```

