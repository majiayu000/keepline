# Truncated JSONL Tail Recovery Tech Spec

Product spec: specs/GH76/product.md

Issue: https://github.com/majiayu000/keepline/issues/76

## Current Root Cause

`src/adapters/claude/parser/jsonl.ts` and `src/adapters/codex/parser.ts` parse each JSONL line as soon as it is read. Any `JSON.parse` failure throws `ParseError`. Scanner callers catch that error, cache the file as failed for the current mtime, and return no session data for that file.

This is correct for middle-of-file corruption, but wrong for a final partial line caused by a process crash or interrupted write.

## Design

Use one-line lookahead in both streaming parsers:

1. Keep the current line pending.
2. When the next line arrives, parse the previous pending line as definitely non-final.
3. After the stream ends, parse the last pending line with `allowTruncatedTail: true`.
4. If the last pending line fails JSON parse, return `null` for that line only and finalize the accumulator from earlier complete records.
5. If any non-final line fails JSON parse, throw `ParseError` with the original file path and line number.

This keeps memory constant and preserves the streaming parser shape.

## Implementation Notes

- Claude parser:
  - Extend `parseLine` to accept `allowTruncatedTail`.
  - Add a local `processLine` helper inside `parseSessionFile`.
  - Reuse existing accumulator/finalizer unchanged.

- Codex parser:
  - Add a small `parseCodexLine` helper with the same last-line tolerance.
  - Apply the same pending-line flow in `parseCodexSessionFile`.

- Tests:
  - Adjust invalid JSONL tests so non-final corruption is still asserted.
  - Add final-line truncation tests for both parsers.
  - Add scanner-level coverage that truncated tails do not become cached failures.

## Risk Notes

- Tolerating any malformed final line can hide a real final-record corruption. This is deliberate because final-record corruption is indistinguishable from crash truncation and should not hide an otherwise recoverable session.
- The parser does not log tolerated tail truncation. Avoid noisy logs for files actively being written.
