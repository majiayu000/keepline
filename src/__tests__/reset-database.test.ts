import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { queryOne, runSql, closeDatabase } from '../infrastructure/database/sqlite.js';
import { resetDatabase, runMigrations } from '../db/migrations.js';
import { memoryRepository } from '../infrastructure/database/index.js';
import { setupUser } from '../services/auth.service.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

describe('resetDatabase', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('clears newer tables and reapplies migrations', async () => {
    runMigrations();
    await setupUser('reset-user', 'password123');
    sessionRepository.upsert({
      sessionId: 'session-reset-test',
      directory: '/tmp/reset-test',
      status: 'running',
    });
    memoryRepository.upsert({
      sessionId: 'session-reset-test',
      directory: '/tmp/reset-test',
      lastProgress: 'in progress',
    });
    runSql(
      'INSERT INTO terminal_sessions (id, user_id, pid, cwd, status) VALUES (?, ?, ?, ?, ?)',
      ['pty-test', queryOne<{ id: string }>('SELECT id FROM terminal_users LIMIT 1')!.id, 1234, '/tmp/reset-test', 'running']
    );

    resetDatabase();

    expect(queryOne<{ count: number }>('SELECT COUNT(*) as count FROM terminal_users')?.count).toBe(0);
    expect(queryOne<{ count: number }>('SELECT COUNT(*) as count FROM session_memories')?.count).toBe(0);
    expect(queryOne<{ count: number }>('SELECT COUNT(*) as count FROM terminal_sessions')?.count).toBe(0);
    expect((queryOne<{ count: number }>('SELECT COUNT(*) as count FROM schema_migrations')?.count ?? 0)).toBeGreaterThan(0);
  });
});
