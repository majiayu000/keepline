import { beforeEach, describe, expect, test } from 'bun:test';
import { scanRuntimeSessions } from '../services/session.service.js';
import {
  clearRuntimeScanStatus,
  getRuntimeScanStatus,
} from '../services/runtime-status.js';

describe('runtime session scan wrapper', () => {
  beforeEach(() => {
    clearRuntimeScanStatus();
  });

  test('records returned scanner failures without hiding sessions', async () => {
    const result = await scanRuntimeSessions('claude-code', async () => ({
      sessions: [{ sessionId: 'claude-visible-session' }],
      failures: [{
        filePath: '/tmp/claude/bad-session.jsonl',
        message: 'Invalid JSONL',
      }],
    }));

    expect(result.sessions).toEqual([{ sessionId: 'claude-visible-session' }]);
    expect(result.failures).toHaveLength(1);

    const claudeStatus = getRuntimeScanStatus().find(
      (scan) => scan.runtimeId === 'claude-code'
    );
    expect(claudeStatus).toMatchObject({
      degraded: true,
      errorCount: 1,
      errors: [expect.objectContaining({
        code: 'parse-failed',
        message: 'Invalid JSONL',
        sourcePath: '/tmp/claude/bad-session.jsonl',
      })],
    });
  });

  test('records scanner rejection as a degraded runtime scan result', async () => {
    const result = await scanRuntimeSessions('codex', async () => {
      throw new Error('Cannot read Codex sessions directory');
    });

    expect(result.sessions).toEqual([]);
    expect(result.failures).toEqual([expect.objectContaining({
      code: 'unknown',
      message: 'Cannot read Codex sessions directory',
    })]);

    const codexStatus = getRuntimeScanStatus().find(
      (scan) => scan.runtimeId === 'codex'
    );
    expect(codexStatus).toMatchObject({
      degraded: true,
      errorCount: 1,
      errors: [expect.objectContaining({
        code: 'unknown',
        message: 'Cannot read Codex sessions directory',
      })],
    });
  });
});
