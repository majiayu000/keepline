import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { sessionDigestRepository } from '../infrastructure/database/repositories/session-digest.repository.js';

describe('SessionDigestRepository', () => {
  beforeEach(() => {
    resetDatabase();
    sessionRepository.upsert({
      sessionId: 'digest-repo-session',
      client: 'codex',
      directory: '/tmp/keepline-digest-repo',
      status: 'running',
      title: 'Digest repo session',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
    });
  });

  afterEach(() => {
    closeDatabase();
  });

  test('upserts and reads a session digest', () => {
    const digest = sessionDigestRepository.upsert({
      sessionId: 'digest-repo-session',
      summary: 'Ready for review',
      nextActions: ['Review pending diff'],
      blockers: ['Waiting on CI'],
      waitingForHuman: true,
      source: 'deterministic',
      sourceUpdatedAt: new Date('2026-06-29T10:00:00.000Z'),
    });

    expect(digest).toMatchObject({
      sessionId: 'digest-repo-session',
      summary: 'Ready for review',
      nextActions: ['Review pending diff'],
      blockers: ['Waiting on CI'],
      waitingForHuman: true,
      source: 'deterministic',
      status: 'fresh',
    });
  });

  test('markError preserves the previous digest content and source', () => {
    sessionDigestRepository.upsert({
      sessionId: 'digest-repo-session',
      summary: 'Deterministic summary',
      nextActions: ['Keep local summary'],
      blockers: [],
      waitingForHuman: false,
      source: 'deterministic',
      sourceUpdatedAt: new Date('2026-06-29T10:00:00.000Z'),
    });

    const digest = sessionDigestRepository.markError({
      sessionId: 'digest-repo-session',
      source: 'local_model',
      sourceUpdatedAt: new Date('2026-06-29T11:00:00.000Z'),
      provider: 'ollama',
      errorMessage: 'Local provider schema error',
    });

    expect(digest).toMatchObject({
      sessionId: 'digest-repo-session',
      summary: 'Deterministic summary',
      nextActions: ['Keep local summary'],
      source: 'deterministic',
      status: 'error',
      provider: 'ollama',
      errorMessage: 'Local provider schema error',
    });
    expect(digest.sourceUpdatedAt.toISOString()).toBe('2026-06-29T10:00:00.000Z');
  });

  test('findBySessionIds ignores duplicate and empty ids', () => {
    sessionDigestRepository.upsert({
      sessionId: 'digest-repo-session',
      summary: 'One digest',
      source: 'deterministic',
      sourceUpdatedAt: new Date('2026-06-29T10:00:00.000Z'),
    });

    const digests = sessionDigestRepository.findBySessionIds([
      '',
      'digest-repo-session',
      'digest-repo-session',
    ]);

    expect(digests).toHaveLength(1);
    expect(digests[0].sessionId).toBe('digest-repo-session');
  });
});
