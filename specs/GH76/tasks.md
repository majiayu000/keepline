# Truncated JSONL Tail Recovery Tasks

Issue: https://github.com/majiayu000/keepline/issues/76

## Tasks

- [x] Confirm root cause in Claude and Codex JSONL parser loops.
- [x] Add GH76 product and tech specs.
- [x] Update Claude parser to tolerate malformed final line only.
- [x] Update Codex parser to tolerate malformed final line only.
- [x] Add parser tests for truncated final line and corrupted middle line.
- [x] Add scanner/cache tests proving truncated sessions remain visible.
- [x] Run focused parser/scanner tests.
- [x] Run `bun run typecheck`.
- [x] Run `bun test`.
- [x] Run `bun run build`.

## Completion Mode

Final implementation PR should use `Fixes #76` only after all acceptance criteria pass. Partial follow-up slices should use `Refs #76`.
