# Menubar changelog

## 1.1.2

- Run release builds through npm explicitly, matching the installed frontend
  toolchain and avoiding automatic selection of an unavailable Bun executable.

## 1.1.1

- Read Codex history statistics through `agent-sessions`, including custom
  `CODEX_HOME` directories and explicit errors for empty directory overrides.
- Update the local cost SDK to `ccstats 0.9.0`, which shares Claude Code and
  Codex session readers with the history integration.
- Preserve history-entry counting and UTC daily totals; malformed records,
  incomplete tails, invalid UTF-8, and read errors still reject partial results.
