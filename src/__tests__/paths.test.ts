import { describe, expect, test } from 'bun:test';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { migrateClaudeHubDataHome } from '../lib/paths.js';

describe('Claude Hub path migration', () => {
  test('migrates legacy Tasker data into the Claude Hub home', () => {
    const root = mkdtempSync(join(tmpdir(), 'claude-hub-paths-'));
    const legacyHome = join(root, '.tasker');
    const nextHome = join(root, '.claude-hub');
    const legacyFile = join(legacyHome, 'config.json');

    mkdirSync(legacyHome, { recursive: true });
    writeFileSync(legacyFile, '{"ok":true}', { flag: 'w' });

    migrateClaudeHubDataHome(legacyHome, nextHome);

    expect(existsSync(nextHome)).toBe(true);
    expect(existsSync(legacyHome)).toBe(false);
    expect(readFileSync(join(nextHome, 'config.json'), 'utf-8')).toContain('"ok":true');

    rmSync(root, { recursive: true, force: true });
  });
});
