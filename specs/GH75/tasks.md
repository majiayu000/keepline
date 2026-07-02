# Hook Pipeline Recovery Tasks

Issue: https://github.com/majiayu000/keepline/issues/75

## Tasks

- [x] Verify current Claude Code hook contract from official docs.
- [x] Add GH75 product and tech specs.
- [x] Update hook settings types for current matcher-group schema.
- [x] Replace env-var payload synthesis with stdin forwarding.
- [x] Register Keepline hook command for all consumed event types.
- [x] Preserve and upgrade legacy Keepline hook ownership detection.
- [x] Normalize official Claude Code payloads at the hook server boundary.
- [x] Keep legacy `event_type` payload compatibility.
- [x] Add focused installer and server tests.
- [x] Run focused hook tests.
- [x] Run `bun run typecheck`.
- [x] Run `bun test`.
- [x] Run `bun run build`.

## Completion Mode

Final implementation PR should use `Fixes #75` only after all acceptance criteria pass. Partial follow-up slices should use `Refs #75`.
