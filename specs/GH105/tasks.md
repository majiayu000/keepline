# Centralized Web Port And Environment Config Task Plan

Issue: https://github.com/majiayu000/keepline/issues/105

Product spec: `specs/GH105/product.md`
Tech spec: `specs/GH105/tech.md`

## Tasks

### SP105-T1: Centralize web port config

Done when CLI and API server defaults read the same config field and `--port`
still overrides it.

Verify:

```sh
bun test src/__tests__/config.test.ts
```

### SP105-T2: Classify env boundaries and dead fields

Done when direct env reads are either moved to config or documented as boundary
exceptions, and dead fields have an explicit decision.

Verify:

```sh
bun run typecheck
```

### SP105-T3: Align docs and deterministic checks

Verify:

```sh
bun test src/__tests__/config.test.ts
bun run typecheck
```

