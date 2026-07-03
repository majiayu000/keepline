# Centralized Web Port And Environment Config Tech Spec

Product spec: `specs/GH105/product.md`

Issue: https://github.com/majiayu000/keepline/issues/105

## Implementation Scope

- `src/lib/config.ts`
- `src/cli/web.ts`
- `src/web/api/server.ts`
- `src/web/client/vite.config.ts`
- `README.md`
- `README_CN.md`
- `src/__tests__/config.test.ts`

## Design

Add `webPort` to `KeeplineConfig` defaults and validation. `webCommand()` should
use the CLI option when provided, otherwise `config.get().webPort`.
`startWebServer()` should default to the same config value.

Keep direct env reads only at process boundary modules where they describe OS,
auth, or proxy state. Document these as boundary exceptions instead of pretending
all env access can be removed in one tranche.

Treat `embeddingDimension` in vector config as the store-side required dimension
and align GH99 validation with that meaning.

## Verification

```sh
bun test src/__tests__/config.test.ts
bun run typecheck
```

## Risks And Rollback

Changing config defaults can affect startup. Preserve current default values and
only centralize their source.

