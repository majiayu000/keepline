import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, getDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { eventStore } from '../infrastructure/events/store.js';
import { runRetentionCleanup } from '../services/retention.service.js';

const NOW = new Date('2026-07-02T12:00:00.000Z');
const OLD_DATE = new Date('2026-05-01T12:00:00.000Z');
const RECENT_DATE = new Date('2026-06-25T12:00:00.000Z');

describe('retention cleanup', () => {
  beforeEach(async () => {
    resetDatabase();
    await eventStore.deleteOlderThan(new Date('9999-01-01T00:00:00.000Z'));
  });

  afterEach(() => {
    closeDatabase();
  });

  function count(table: string, where: string = '1 = 1'): number {
    const row = getDatabase()
      .prepare(`SELECT COUNT(*) as count FROM ${table} WHERE ${where}`)
      .get() as { count: number };
    return row.count;
  }

  function insertSession(sessionId: string, status: 'completed' | 'running', lastActiveAt: Date) {
    sessionRepository.upsert({
      sessionId,
      directory: '/tmp/project',
      status,
      title: sessionId,
      initialPrompt: sessionId,
      startedAt: lastActiveAt,
      lastActiveAt,
      completedAt: status === 'completed' ? lastActiveAt : undefined,
      toolCount: 1,
      messageCount: 1,
    });
  }

  function insertUsageAndHookEvents(sessionId: string) {
    const db = getDatabase();
    db.prepare(`
      INSERT INTO tool_usage (session_id, tool, input, output, duration, timestamp)
      VALUES (?, 'Edit', '{}', '{}', 10, ?)
    `).run(sessionId, OLD_DATE.toISOString());
    db.prepare(`
      INSERT INTO hook_events (session_id, event_type, payload, timestamp)
      VALUES (?, 'PostToolUse', '{}', ?)
    `).run(sessionId, OLD_DATE.toISOString());
  }

  test('deletes old completed sessions, related rows, and old events', async () => {
    insertSession('old-completed-session', 'completed', OLD_DATE);
    insertSession('recent-completed-session', 'completed', RECENT_DATE);
    insertSession('old-running-session', 'running', OLD_DATE);
    insertUsageAndHookEvents('old-completed-session');
    insertUsageAndHookEvents('recent-completed-session');
    insertUsageAndHookEvents('old-running-session');

    await eventStore.append({
      type: 'old:event',
      aggregateId: 'old',
      payload: {},
      timestamp: OLD_DATE,
    });
    await eventStore.append({
      type: 'recent:event',
      aggregateId: 'recent',
      payload: {},
      timestamp: RECENT_DATE,
    });

    const result = await runRetentionCleanup(30, NOW);

    expect(result).toMatchObject({
      disabled: false,
      retentionDays: 30,
      sessionsDeleted: 1,
      eventsDeleted: 1,
    });
    expect(sessionRepository.findBySessionId('old-completed-session')).toBeNull();
    expect(sessionRepository.findBySessionId('recent-completed-session')).not.toBeNull();
    expect(sessionRepository.findBySessionId('old-running-session')).not.toBeNull();
    expect(count('tool_usage', "session_id = 'old-completed-session'")).toBe(0);
    expect(count('hook_events', "session_id = 'old-completed-session'")).toBe(0);
    expect(count('tool_usage', "session_id = 'recent-completed-session'")).toBe(1);
    expect(count('hook_events', "session_id = 'old-running-session'")).toBe(1);
    expect(await eventStore.count()).toBe(1);
  });

  test('retentionDays <= 0 disables cleanup', async () => {
    insertSession('old-completed-session', 'completed', OLD_DATE);
    insertUsageAndHookEvents('old-completed-session');
    await eventStore.append({
      type: 'old:event',
      aggregateId: 'old',
      payload: {},
      timestamp: OLD_DATE,
    });

    const result = await runRetentionCleanup(0, NOW);

    expect(result).toEqual({
      disabled: true,
      retentionDays: 0,
      sessionsDeleted: 0,
      eventsDeleted: 0,
    });
    expect(sessionRepository.findBySessionId('old-completed-session')).not.toBeNull();
    expect(count('tool_usage', "session_id = 'old-completed-session'")).toBe(1);
    expect(await eventStore.count()).toBe(1);
  });
});
