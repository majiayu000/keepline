# Hook Server Request Security Task Plan

Issue: https://github.com/majiayu000/keepline/issues/79

## Tasks

### SP79-T1: Add SpecRail artifacts

Done when:

- `specs/GH79/product.md` exists.
- `specs/GH79/tech.md` exists.
- `specs/GH79/tasks.md` exists.

Verify:

```sh
test -f specs/GH79/product.md
test -f specs/GH79/tech.md
test -f specs/GH79/tasks.md
```

### SP79-T2: Add hook request gate

Writable files:

- `src/adapters/hook/server.ts`

Done when:

- Hook server rejects non-loopback Host headers.
- Hook server rejects non-loopback Origins when present.
- Hook server rejects cross-site Fetch Metadata when present.
- Loopback local automation without browser headers still works.

Verify:

```sh
bun test src/__tests__/hook.server.test.ts
```

### SP79-T3: Add regression tests

Writable files:

- `src/__tests__/hook.server.test.ts`

Done when:

- Non-loopback Host on `/hook` returns 403.
- Cross-origin `/context` returns 403.
- Loopback `/health` returns 200.
- Loopback invalid hook payload returns 400.

Verify:

```sh
bun test src/__tests__/hook.server.test.ts
```

### SP79-T4: Full verification and PR

Done when:

- Focused tests pass.
- Typecheck passes.
- Full tests pass.
- Build passes.
- PR links and closes #79.

Verify:

```sh
bun test src/__tests__/hook.server.test.ts
bun run typecheck
bun test
bun run build
```
