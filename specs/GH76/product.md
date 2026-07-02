# Truncated JSONL Tail Recovery Product Spec

Issue: https://github.com/majiayu000/keepline/issues/76

## Summary

Keepline must keep recoverable sessions visible when a Claude Code or Codex JSONL transcript ends with a truncated final line. A crash or interrupted write can leave the last JSONL record incomplete; the parser should preserve all complete records before that tail instead of rejecting the whole file.

## User Problem

The product promise is session recovery after terminal crashes. Crash-time JSONL files are exactly where a truncated final line is most likely. If one bad final line causes the whole file to be cached as a parse failure, the session disappears from the dashboard and recovery candidates.

## Product Behavior

1. Claude Code session JSONL files with complete prior lines and a malformed final line still produce a parsed session summary.
2. Codex rollout JSONL files with complete prior lines and a malformed final line still produce a parsed session summary.
3. Malformed non-final lines remain hard parse errors.
4. Scanner failure caches are not populated for tolerated truncated-tail files, so repeated scans keep the session visible.
5. Empty/blank trailing lines remain ignored.

## Non-Goals

- Do not silently tolerate corruption in the middle of a transcript.
- Do not attempt partial JSON repair or schema inference.
- Do not change scanner mtime cache semantics except through avoiding false parse failures.
- Do not broaden accepted file names or session IDs.

## Acceptance Criteria

1. Claude parser skips a malformed final JSONL line and returns data from earlier valid entries.
2. Codex parser skips a malformed final JSONL line and returns data from earlier valid entries.
3. Claude parser still throws `ParseError` for malformed non-final JSONL lines.
4. Codex parser still throws `ParseError` for malformed non-final JSONL lines.
5. Tests prove truncated-tail sessions are visible through parser/scanner paths.
6. Verification commands pass: focused parser/scanner tests, `bun run typecheck`, `bun test`, `bun run build`.
