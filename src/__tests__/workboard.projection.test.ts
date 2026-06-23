import { describe, expect, test } from 'bun:test';
import {
  buildWorkboardProjection,
  type AgentSession,
  type ProgressEvidence,
  type WorkItem,
  type WorkItemSessionLink,
} from '../domain/work-item/index.js';

const NOW = new Date('2026-06-23T12:00:00.000Z');
const RECENT = new Date('2026-06-23T11:00:00.000Z');
const OLD = new Date('2026-06-20T12:00:00.000Z');

function workItem(id: string, status: WorkItem['status']): WorkItem {
  return {
    id,
    title: id,
    kind: 'todo',
    status,
    statusSource: 'user',
    createdAt: OLD,
    updatedAt: RECENT,
    completedAt: status === 'done' ? RECENT : undefined,
  };
}

function agentSession(
  id: string,
  status: AgentSession['status'],
  lastActiveAt: Date
): AgentSession {
  return {
    id,
    runtimeId: 'codex',
    runtimeSessionId: id,
    cwd: '/tmp/project',
    status,
    title: id,
    lastActiveAt,
    createdAt: OLD,
    updatedAt: lastActiveAt,
  };
}

function sessionLink(
  workItemId: string,
  agentSessionId: string,
  acceptanceStatus: WorkItemSessionLink['acceptanceStatus'] = 'accepted'
): WorkItemSessionLink {
  return {
    id: `${workItemId}-${agentSessionId}`,
    workItemId,
    agentSessionId,
    linkSource: acceptanceStatus === 'accepted' ? 'user' : 'heuristic_suggestion',
    acceptanceStatus,
    acceptedAt: acceptanceStatus === 'accepted' ? RECENT : undefined,
    createdAt: RECENT,
    updatedAt: RECENT,
  };
}

function progressEvidence(input: {
  id: string;
  workItemId?: string;
  agentSessionId?: string;
  outcome?: ProgressEvidence['outcome'];
  confidence?: ProgressEvidence['confidence'];
  occurredAt: Date;
}): ProgressEvidence {
  return {
    id: input.id,
    workItemId: input.workItemId,
    agentSessionId: input.agentSessionId,
    runtimeId: 'codex',
    kind: 'message',
    outcome: input.outcome,
    summary: input.id,
    occurredAt: input.occurredAt,
    confidence: input.confidence ?? 'explicit',
    createdAt: input.occurredAt,
  };
}

describe('buildWorkboardProjection', () => {
  test('applies bucket precedence without mutating work item state', () => {
    const items = [
      workItem('done-item', 'done'),
      workItem('blocked-item', 'blocked'),
      workItem('waiting-item', 'planned'),
      workItem('completed-waiting-item', 'planned'),
      workItem('stale-active-item', 'active'),
      workItem('active-item', 'active'),
      workItem('planned-running-item', 'planned'),
      workItem('pending-running-item', 'planned'),
      workItem('active-no-evidence-item', 'active'),
      workItem('blank-planned-item', 'planned'),
    ];
    const sessions = [
      agentSession('waiting-session', 'waiting', RECENT),
      agentSession('completed-waiting-session', 'waiting', OLD),
      agentSession('stale-session', 'completed', OLD),
      agentSession('running-session', 'running', RECENT),
      agentSession('pending-running-session', 'running', RECENT),
    ];
    const links = [
      sessionLink('waiting-item', 'waiting-session'),
      sessionLink('completed-waiting-item', 'completed-waiting-session'),
      sessionLink('stale-active-item', 'stale-session'),
      sessionLink('planned-running-item', 'running-session'),
      sessionLink('pending-running-item', 'pending-running-session', 'pending'),
    ];
    const evidence = [
      progressEvidence({
        id: 'explicit-completed-after-waiting',
        workItemId: 'completed-waiting-item',
        agentSessionId: 'completed-waiting-session',
        outcome: 'completed',
        confidence: 'explicit',
        occurredAt: RECENT,
      }),
      progressEvidence({
        id: 'recent-active-progress',
        workItemId: 'active-item',
        outcome: 'progress',
        occurredAt: RECENT,
      }),
      progressEvidence({
        id: 'pending-session-progress',
        workItemId: 'pending-running-item',
        agentSessionId: 'pending-running-session',
        outcome: 'progress',
        occurredAt: RECENT,
      }),
    ];

    const projection = buildWorkboardProjection({
      items,
      links,
      agentSessions: sessions,
      evidence,
      now: NOW,
      staleWindowHours: 24,
    });

    expect(projection.done.map((entry) => entry.item.id)).toEqual(['done-item']);
    expect(projection.waiting.map((entry) => entry.item.id)).toEqual([
      'blocked-item',
      'waiting-item',
    ]);
    expect(projection.stale.map((entry) => entry.item.id)).toEqual(['stale-active-item']);
    expect(projection.now.map((entry) => entry.item.id)).toEqual([
      'active-item',
      'active-no-evidence-item',
      'planned-running-item',
    ]);
    expect(projection.suggestions.map((entry) => entry.item.id)).toEqual(['pending-running-item']);
    expect(projection.now.find((entry) => entry.item.id === 'active-no-evidence-item')?.progressSummary)
      .toBeUndefined();
    expect(items.find((item) => item.id === 'completed-waiting-item')?.status).toBe('planned');
    expect([
      ...projection.now,
      ...projection.waiting,
      ...projection.stale,
      ...projection.done,
    ].some((entry) => entry.item.id === 'pending-running-item')).toBe(false);
  });

  test('does not treat inferred completion as a waiting-clear signal', () => {
    const items = [workItem('inferred-completed-item', 'planned')];
    const sessions = [agentSession('waiting-session', 'waiting', OLD)];
    const links = [sessionLink('inferred-completed-item', 'waiting-session')];
    const evidence = [
      progressEvidence({
        id: 'inferred-completed-after-waiting',
        workItemId: 'inferred-completed-item',
        agentSessionId: 'waiting-session',
        outcome: 'completed',
        confidence: 'inferred',
        occurredAt: RECENT,
      }),
    ];

    const projection = buildWorkboardProjection({
      items,
      links,
      agentSessions: sessions,
      evidence,
      now: NOW,
      staleWindowHours: 24,
    });

    expect(projection.waiting.map((entry) => entry.item.id)).toEqual(['inferred-completed-item']);
    expect(items[0].status).toBe('planned');
  });
});
