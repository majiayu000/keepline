# Menubar changelog

## 1.1.4 - 2026-09-26

- Notarize the signed DMG separately from its enclosed app, require acceptance, staple its ticket with bounded retries, and verify both layers with Gatekeeper.
- Keep releases in draft until every platform build and macOS verification succeeds; attach SHA-256 checksums before publishing.
- Supersede 1.1.3, whose apps were notarized but DMG containers were not.

## 1.1.3 - 2026-09-26

- Lock npm dependencies and keep Tauri API/CLI on the Rust runtime's 2.10 minor line, preventing fresh release builds from selecting an incompatible 2.11 API.
- Use `npm ci` in the release workflow and validate the locked frontend with Node 20 in CI.
- Preserve failed 1.1.1/1.1.2 tags; neither candidate produced a completed release.

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
