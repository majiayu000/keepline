# Web Mode Hook Availability Task Plan

Issue: https://github.com/majiayu000/keepline/issues/101

Product spec: `specs/GH101/product.md`
Tech spec: `specs/GH101/tech.md`

## Tasks

### SP101-T1: Add shared hook availability helper

Done when installed, receiver running, and degraded states are computed in one
place.

Verify:

```sh
bun test src/__tests__/hook.server.test.ts
```

### SP101-T2: Surface degraded status

Done when CLI status and Web API expose installed-but-not-running distinctly.

Verify:

```sh
bun run typecheck
```

### SP101-T3: Update docs

Done when README files describe `web`, `daemon`, and `hooks install` accurately.

Verify:

```sh
bun run typecheck
```

