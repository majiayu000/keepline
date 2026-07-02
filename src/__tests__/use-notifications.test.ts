import { describe, expect, test } from 'bun:test';
import {
  getSessionNotificationEvents,
  type NotificationEventSettings,
} from '../web/client/src/hooks/notification-events.js';
import type { Session } from '../web/client/src/types/session.js';

const settings: NotificationEventSettings = {
  onStatusChange: true,
  onSessionLost: true,
  onHighCost: true,
  costThreshold: 5,
};

function makeSession(overrides: Partial<Session>): Session {
  return {
    id: 'row-1',
    sessionId: 'session-1',
    client: 'claude',
    runtimeId: 'claude-code',
    directory: '/tmp/project',
    status: 'running',
    title: 'Expensive task',
    initialPrompt: 'Run the task',
    lastActiveAt: '2026-04-13T10:00:05.000Z',
    toolCount: 1,
    messageCount: 1,
    createdAt: '2026-04-13T10:00:00.000Z',
    updatedAt: '2026-04-13T10:00:05.000Z',
    ...overrides,
  };
}

describe('notification event derivation', () => {
  test('emits high-cost notification when session cost crosses threshold', () => {
    const events = getSessionNotificationEvents(
      [makeSession({ usageStats: { totalInputTokens: 100, totalOutputTokens: 50, totalTokens: 150, totalCost: 4.99, apiCalls: 1 } })],
      [makeSession({ usageStats: { totalInputTokens: 200, totalOutputTokens: 100, totalTokens: 300, totalCost: 5.01, apiCalls: 2 } })],
      settings
    );

    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({
      title: 'High Cost Warning',
      options: {
        tag: 'high-cost-session-1',
      },
    });
    expect(events[0].options.body).toContain('$5.00');
    expect(events[0].options.body).toContain('$5.01');
  });

  test('does not repeat high-cost notification when previous cost was already above threshold', () => {
    const events = getSessionNotificationEvents(
      [makeSession({ usageStats: { totalInputTokens: 200, totalOutputTokens: 100, totalTokens: 300, totalCost: 5.01, apiCalls: 2 } })],
      [makeSession({ usageStats: { totalInputTokens: 300, totalOutputTokens: 200, totalTokens: 500, totalCost: 6, apiCalls: 3 } })],
      settings
    );

    expect(events).toEqual([]);
  });
});
