import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

describe('Session Repository Upsert', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('preserves pid and tty when omitted from an update', () => {
    sessionRepository.upsert({
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

    const updated = sessionRepository.upsert({
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
    sessionRepository.upsert({
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

    const updated = sessionRepository.upsert({
      sessionId: 'session-clear',
      pid: undefined,
      tty: undefined,
    });

    expect(updated.pid).toBeUndefined();
    expect(updated.tty).toBeUndefined();
  });

  test('persists multi-agent metadata, usage stats, and tool calls (issue #13)', () => {
    const created = sessionRepository.upsert({
      sessionId: 'session-metadata',
      directory: '/tmp/repo',
      status: 'running',
      title: 'Meta',
      initialPrompt: 'Prompt',
      lastActiveAt: new Date('2026-04-13T15:00:00.000Z'),
      toolCount: 3,
      messageCount: 5,
      agentId: 'agent-xyz',
      parentSessionId: 'parent-session-1',
      isSubAgent: true,
      usageStats: {
        totalInputTokens: 100,
        totalOutputTokens: 200,
        totalTokens: 300,
        totalCost: 0.42,
        apiCalls: 7,
      },
      toolCalls: [
        { name: 'Read', input: { file: 'a.ts' }, timestamp: '2026-04-13T15:00:00.000Z' },
        { name: 'Bash', input: { cmd: 'ls' }, timestamp: '2026-04-13T15:01:00.000Z' },
      ],
    });

    expect(created.agentId).toBe('agent-xyz');
    expect(created.parentSessionId).toBe('parent-session-1');
    expect(created.isSubAgent).toBe(true);
    expect(created.usageStats?.totalInputTokens).toBe(100);
    expect(created.usageStats?.totalCost).toBe(0.42);
    expect(created.usageStats?.apiCalls).toBe(7);
    expect(created.toolCalls).toHaveLength(2);
    expect(created.toolCalls?.[0].name).toBe('Read');

    // Re-read from DB to confirm true persistence, not just the in-memory return
    const fetched = sessionRepository.findBySessionId('session-metadata');
    expect(fetched?.agentId).toBe('agent-xyz');
    expect(fetched?.isSubAgent).toBe(true);
    expect(fetched?.usageStats?.totalTokens).toBe(300);
    expect(fetched?.toolCalls?.[1].name).toBe('Bash');
  });

  test('persists completedAt when inserting completed sessions', () => {
    const completedAt = new Date('2026-04-13T16:00:00.000Z');

    sessionRepository.upsert({
      sessionId: 'session-completed-insert',
      directory: '/tmp/repo',
      status: 'completed',
      title: 'Completed',
      initialPrompt: 'Prompt',
      startedAt: new Date('2026-04-13T15:00:00.000Z'),
      lastActiveAt: completedAt,
      completedAt,
      toolCount: 0,
      messageCount: 1,
    });

    const fetched = sessionRepository.findBySessionId('session-completed-insert');
    expect(fetched?.completedAt?.toISOString()).toBe('2026-04-13T16:00:00.000Z');
  });
});
