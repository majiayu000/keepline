import { describe, expect, test } from 'bun:test';
import type { AggregatedSession } from '../services/session.types.js';
import { buildAttentionOverview } from '../services/attention.prioritizer.js';
import type { SessionDigest } from '../domain/orchestrator/index.js';

const NOW = new Date('2026-06-29T12:00:00.000Z');
const RECENT = new Date('2026-06-29T11:55:00.000Z');
const OLD = new Date('2026-06-27T12:00:00.000Z');

function session(input: Partial<AggregatedSession> & {
  sessionId: string;
  status: AggregatedSession['status'];
}): AggregatedSession {
  return {
    id: input.sessionId,
    sessionId: input.sessionId,
    client: input.client ?? 'codex',
    directory: input.directory ?? '/tmp/keepline',
    status: input.status,
    title: input.title ?? input.sessionId,
    initialPrompt: '',
    startedAt: RECENT,
    lastActiveAt: input.lastActiveAt ?? RECENT,
    completedAt: input.completedAt,
    pid: input.pid,
    tty: input.tty,
    toolCount: input.toolCount ?? 0,
    messageCount: input.messageCount ?? 0,
    usageStats: input.usageStats,
    processRunning: input.processRunning ?? false,
    cpuUsage: input.cpuUsage,
    memoryUsage: input.memoryUsage,
    createdAt: RECENT,
    updatedAt: RECENT,
  };
}

describe('buildAttentionOverview', () => {
  test('ranks waiting above lost, high-cost, stale, idle, running, and completed', () => {
    const overview = buildAttentionOverview([
      session({ sessionId: 'running', status: 'running' }),
      session({ sessionId: 'completed', status: 'completed' }),
      session({ sessionId: 'idle', status: 'idle' }),
      session({ sessionId: 'stale', status: 'running', lastActiveAt: OLD }),
      session({
        sessionId: 'high-cost',
        status: 'running',
        usageStats: {
          totalInputTokens: 10,
          totalOutputTokens: 20,
          totalTokens: 30,
          totalCost: 5,
          apiCalls: 1,
        },
      }),
      session({ sessionId: 'lost', status: 'lost' }),
      session({ sessionId: 'waiting', status: 'waiting' }),
    ], {
      now: NOW,
      highCostThreshold: 1,
      staleHours: 24,
    });

    expect(overview.items.map((item) => item.sessionId)).toEqual([
      'waiting',
      'lost',
      'high-cost',
      'stale',
      'idle',
      'running',
    ]);
    expect(overview.items[0]).toMatchObject({
      rank: 1,
      recommendedAction: 'review',
      reasons: [expect.objectContaining({ code: 'waiting_for_human' })],
    });
    expect(overview.items[1].recommendedAction).toBe('recover');
    expect(overview.items.find((item) => item.sessionId === 'completed')).toBeUndefined();
  });

  test('keeps higher-priority states above lower-priority stacked reasons', () => {
    const overview = buildAttentionOverview([
      session({
        sessionId: 'lost-high-cost',
        status: 'lost',
        usageStats: {
          totalInputTokens: 100,
          totalOutputTokens: 200,
          totalTokens: 300,
          totalCost: 100,
          apiCalls: 5,
        },
      }),
      session({
        sessionId: 'running-high-cost-stale',
        status: 'running',
        lastActiveAt: OLD,
        usageStats: {
          totalInputTokens: 100,
          totalOutputTokens: 200,
          totalTokens: 300,
          totalCost: 100,
          apiCalls: 5,
        },
      }),
      session({ sessionId: 'waiting-only', status: 'waiting' }),
    ], {
      now: NOW,
      highCostThreshold: 1,
      staleHours: 24,
    });

    expect(overview.items.map((item) => item.sessionId)).toEqual([
      'waiting-only',
      'lost-high-cost',
      'running-high-cost-stale',
    ]);
  });

  test('does not create cost reasons without persisted usage cost', () => {
    const overview = buildAttentionOverview([
      session({ sessionId: 'no-usage', status: 'running' }),
      session({
        sessionId: 'below-threshold',
        status: 'running',
        usageStats: {
          totalInputTokens: 1,
          totalOutputTokens: 1,
          totalTokens: 2,
          totalCost: 0.5,
          apiCalls: 1,
        },
      }),
    ], {
      now: NOW,
      highCostThreshold: 1,
    });

    for (const item of overview.items) {
      expect(item.reasons.some((reason) => reason.code === 'high_cost')).toBe(false);
    }
  });

  test('uses deterministic tie breakers for equal scores', () => {
    const overview = buildAttentionOverview([
      session({ sessionId: 'b-session', status: 'idle', lastActiveAt: RECENT }),
      session({ sessionId: 'a-session', status: 'idle', lastActiveAt: RECENT }),
      session({ sessionId: 'newer-session', status: 'idle', lastActiveAt: NOW }),
    ], { now: NOW });

    expect(overview.items.map((item) => item.sessionId)).toEqual([
      'newer-session',
      'a-session',
      'b-session',
    ]);
  });

  test('can include completed sessions explicitly', () => {
    const overview = buildAttentionOverview([
      session({ sessionId: 'completed', status: 'completed' }),
    ], {
      includeCompleted: true,
      now: NOW,
    });

    expect(overview.items).toHaveLength(1);
    expect(overview.items[0]).toMatchObject({
      sessionId: 'completed',
      recommendedAction: 'none',
      score: 0,
    });
  });

  test('attaches serialized digest without changing queue order', () => {
    const digest: SessionDigest = {
      id: 'digest-id',
      sessionId: 'digest-session',
      summary: 'Digest summary',
      nextActions: ['Next action'],
      blockers: [],
      waitingForHuman: false,
      source: 'deterministic',
      status: 'fresh',
      sourceUpdatedAt: RECENT,
      generatedAt: NOW,
      createdAt: NOW,
      updatedAt: NOW,
    };

    const overview = buildAttentionOverview([
      session({ sessionId: 'waiting-session', status: 'waiting' }),
      session({ sessionId: 'digest-session', status: 'running' }),
    ], {
      now: NOW,
      digests: new Map([['digest-session', digest]]),
    });

    expect(overview.items.map((item) => item.sessionId)).toEqual([
      'waiting-session',
      'digest-session',
    ]);
    expect(overview.items[1].digest).toMatchObject({
      sessionId: 'digest-session',
      summary: 'Digest summary',
      nextActions: ['Next action'],
      source: 'deterministic',
      status: 'fresh',
      generatedAt: '2026-06-29T12:00:00.000Z',
    });
  });
});
