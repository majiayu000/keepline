import { describe, expect, test } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import {
  getClaudeHubDb,
  getClaudeHubHome,
  migrateClaudeHubDataHome,
} from '../lib/paths.js';

describe('Codex Hub path migration', () => {
  test('migrates legacy Tasker data into the Codex Hub home', () => {
    const root = mkdtempSync(join(tmpdir(), 'claude-hub-paths-'));
    const legacyHome = join(root, '.tasker');
    const nextHome = join(root, '.codex-hub');
    const legacyFile = join(legacyHome, 'config.json');

    mkdirSync(legacyHome, { recursive: true });
    writeFileSync(legacyFile, '{"ok":true}', { flag: 'w' });

    migrateClaudeHubDataHome(legacyHome, nextHome);

    expect(existsSync(nextHome)).toBe(true);
    expect(existsSync(legacyHome)).toBe(false);
    expect(readFileSync(join(nextHome, 'config.json'), 'utf-8')).toContain('"ok":true');

    rmSync(root, { recursive: true, force: true });
  });

  test('prefers CODEX_HUB_HOME for new storage paths', () => {
    const root = mkdtempSync(join(tmpdir(), 'codex-hub-paths-'));
    const previousCodexHome = process.env.CODEX_HUB_HOME;
    const previousClaudeHome = process.env.CLAUDE_HUB_HOME;

    try {
      process.env.CODEX_HUB_HOME = root;
      delete process.env.CLAUDE_HUB_HOME;

      expect(getClaudeHubHome()).toBe(root);
      expect(getClaudeHubDb()).toBe(join(root, 'codex-hub.db'));
    } finally {
      if (previousCodexHome === undefined) {
        delete process.env.CODEX_HUB_HOME;
      } else {
        process.env.CODEX_HUB_HOME = previousCodexHome;
      }
      if (previousClaudeHome === undefined) {
        delete process.env.CLAUDE_HUB_HOME;
      } else {
        process.env.CLAUDE_HUB_HOME = previousClaudeHome;
      }
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('keeps legacy CLAUDE_HUB_HOME database file when present', () => {
    const root = mkdtempSync(join(tmpdir(), 'codex-hub-legacy-paths-'));
    const legacyDb = join(root, 'claude-hub.db');
    const previousCodexHome = process.env.CODEX_HUB_HOME;
    const previousClaudeHome = process.env.CLAUDE_HUB_HOME;

    try {
      delete process.env.CODEX_HUB_HOME;
      process.env.CLAUDE_HUB_HOME = root;
      writeFileSync(legacyDb, '');

      expect(getClaudeHubHome()).toBe(root);
      expect(getClaudeHubDb()).toBe(legacyDb);
    } finally {
      if (previousCodexHome === undefined) {
        delete process.env.CODEX_HUB_HOME;
      } else {
        process.env.CODEX_HUB_HOME = previousCodexHome;
      }
      if (previousClaudeHome === undefined) {
        delete process.env.CLAUDE_HUB_HOME;
      } else {
        process.env.CLAUDE_HUB_HOME = previousClaudeHome;
      }
      rmSync(root, { recursive: true, force: true });
    }
  });
});
