import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import {
  getAllCodexSessions,
  scanCodexSessionsDirectory,
} from '../adapters/codex/scanner.js';
import { logger } from '../lib/logger.js';

describe('Codex session scanner', () => {
  let tempDir = '';
  let originalWarn: typeof logger.warn;
  let originalDebug: typeof logger.debug;
  const warnings: Array<{ message: string; data?: unknown }> = [];
  const debugs: Array<{ message: string; data?: unknown }> = [];

  beforeEach(() => {
    tempDir = mkdtempSync(join(tmpdir(), 'keepline-codex-scan-'));
    originalWarn = logger.warn.bind(logger);
    originalDebug = logger.debug.bind(logger);
    warnings.length = 0;
    debugs.length = 0;
    logger.warn = (message: string, data?: unknown) => {
      warnings.push({ message, data });
    };
    logger.debug = (message: string, data?: unknown) => {
      debugs.push({ message, data });
    };
  });

  afterEach(() => {
    logger.warn = originalWarn;
    logger.debug = originalDebug;
    rmSync(tempDir, { recursive: true, force: true });
  });

  test('treats a missing sessions directory as debug no-op', () => {
    const missingDir = join(tempDir, 'missing-sessions');

    const sessions = scanCodexSessionsDirectory({ sessionsDir: missingDir });

    expect(sessions).toEqual([]);
    expect(warnings).toHaveLength(0);
    expect(debugs[0]).toMatchObject({
      message: 'Codex sessions directory not found, skipping scan',
      data: { path: missingDir },
    });
  });

  test('warns when the sessions path cannot be read', () => {
    const filePath = join(tempDir, 'sessions');
    writeFileSync(filePath, 'not a directory');

    const sessions = scanCodexSessionsDirectory({ sessionsDir: filePath });

    expect(sessions).toEqual([]);
    expect(warnings[0]).toMatchObject({
      message: 'Skipped unreadable Codex session paths during scan',
      data: { count: 1, sample: [filePath] },
    });
  });

  test('warns with count and sample for invalid Codex session files', async () => {
    const sessionsDir = join(tempDir, 'sessions');
    mkdirSync(sessionsDir);
    const invalidFile = join(sessionsDir, 'rollout-12345678-1234-1234-1234-123456789abc.jsonl');
    writeFileSync(
      invalidFile,
      [
        '{not-json}',
        JSON.stringify({
          type: 'session_meta',
          timestamp: '2026-06-23T01:00:00.000Z',
          payload: {
            id: '12345678-1234-1234-1234-123456789abc',
            cwd: '/tmp/broken-codex',
          },
        }),
      ].join('\n') + '\n'
    );

    const sessions = await getAllCodexSessions({ sessionsDir });

    expect(sessions).toEqual([]);
    expect(warnings[0]).toMatchObject({
      message: 'Skipped invalid Codex session files during scan',
      data: { count: 1, sample: [invalidFile] },
    });
  });
});
