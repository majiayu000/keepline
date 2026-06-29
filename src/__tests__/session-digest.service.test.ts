import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { memoryRepository } from '../infrastructure/database/repositories/memory.repository.js';
import type { AggregatedSession } from '../services/session.types.js';
import {
  generateDeterministicSessionDigest,
  getSessionDigestMap,
} from '../services/session-digest.service.js';

function aggregatedSession(sessionId: string): AggregatedSession {
  const session = sessionRepository.findBySessionId(sessionId);
  if (!session) throw new Error(`Missing test session ${sessionId}`);
  return {
    ...session,
    processRunning: false,
  };
}

describe('Session digest service', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('generates deterministic digest from memory handoff fields', () => {
    sessionRepository.upsert({
      sessionId: 'digest-service-memory',
      client: 'codex',
      directory: '/tmp/keepline-digest-service',
      status: 'waiting',
      title: 'Memory digest',
      lastMessage: 'Fallback message',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
    });
    memoryRepository.upsert({
      sessionId: 'digest-service-memory',
      directory: '/tmp/keepline-digest-service',
      lastProgress: 'Progress fallback',
      pendingTasks: ['Run focused tests'],
      knownIssues: ['Needs review'],
      handoffNotes: 'Current handoff summary',
      handoffPriority: ['Ship digest schema', 'Run focused tests'],
    });

    const digest = generateDeterministicSessionDigest(
      aggregatedSession('digest-service-memory')
    );

    expect(digest).toMatchObject({
      sessionId: 'digest-service-memory',
      summary: 'Current handoff summary',
      nextActions: ['Ship digest schema', 'Run focused tests'],
      blockers: ['Needs review'],
      waitingForHuman: true,
      source: 'deterministic',
      status: 'fresh',
    });
  });

  test('uses session fields and lost blocker when memory is absent', () => {
    sessionRepository.upsert({
      sessionId: 'digest-service-lost',
      client: 'claude',
      directory: '/tmp/keepline-digest-lost',
      status: 'lost',
      title: 'Lost digest',
      lastMessage: 'Last visible message',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
    });

    const digest = generateDeterministicSessionDigest(
      aggregatedSession('digest-service-lost')
    );

    expect(digest.summary).toBe('Last visible message');
    expect(digest.blockers).toEqual(['Session is lost and may need recovery']);
    expect(digest.waitingForHuman).toBe(false);
  });

  test('loads digest map for active overview enrichment', () => {
    sessionRepository.upsert({
      sessionId: 'digest-map-session',
      directory: '/tmp/keepline-digest-map',
      status: 'running',
      title: 'Map session',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
    });
    const digest = generateDeterministicSessionDigest(
      aggregatedSession('digest-map-session')
    );

    const map = getSessionDigestMap(['missing', 'digest-map-session']);

    expect(map.get('digest-map-session')?.id).toBe(digest.id);
    expect(map.has('missing')).toBe(false);
  });
});
