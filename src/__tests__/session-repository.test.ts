import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepo } from '../db/index.js';

describe('Session Repository Upsert', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('preserves pid and tty when omitted from an update', () => {
    sessionRepo.upsert({
      sessionId: 'session-preserve',
      directory: '/tmp/repo',
      status: 'running',
      title: 'Initial',
      initialPrompt: 'Prompt',
      pid: 1234,
      tty: 'ttys001',
      lastActiveAt: new Date('2026-04-13T15:00:00.000Z'),
      toolCount: 1,
      messageCount: 1,
    });

    const updated = sessionRepo.upsert({
      sessionId: 'session-preserve',
      title: 'Updated',
      toolCount: 2,
    });

    expect(updated.pid).toBe(1234);
    expect(updated.tty).toBe('ttys001');
    expect(updated.title).toBe('Updated');
    expect(updated.toolCount).toBe(2);
  });

  test('clears pid and tty when explicitly updated to undefined', () => {
    sessionRepo.upsert({
      sessionId: 'session-clear',
      directory: '/tmp/repo',
      status: 'running',
      title: 'Initial',
      initialPrompt: 'Prompt',
      pid: 5678,
      tty: 'ttys002',
      lastActiveAt: new Date('2026-04-13T15:00:00.000Z'),
      toolCount: 1,
      messageCount: 1,
    });

    const updated = sessionRepo.upsert({
      sessionId: 'session-clear',
      pid: undefined,
      tty: undefined,
    });

    expect(updated.pid).toBeUndefined();
    expect(updated.tty).toBeUndefined();
  });
});
