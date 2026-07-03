import { describe, expect, test } from 'bun:test';
import {
  REALTIME_FULL_SYNC_INTERVAL_MS,
  shouldRunRealtimeFullSync,
} from '../web/api/realtime-updates.js';
import {
  isSessionStats,
  isSessionsUpdateData,
  isSyncCompleteData,
} from '../web/client/src/services/session-ws-contract.js';

describe('Realtime Update Policy', () => {
  test('runs a full sync when no previous realtime sync has completed', () => {
    expect(shouldRunRealtimeFullSync(0, 1_000)).toBe(true);
  });

  test('skips full syncs before the throttle interval elapses', () => {
    const lastSyncAt = 10_000;
    const now = lastSyncAt + REALTIME_FULL_SYNC_INTERVAL_MS - 1;
    expect(shouldRunRealtimeFullSync(lastSyncAt, now)).toBe(false);
  });

  test('allows a full sync once the throttle interval elapses', () => {
    const lastSyncAt = 10_000;
    const now = lastSyncAt + REALTIME_FULL_SYNC_INTERVAL_MS;
    expect(shouldRunRealtimeFullSync(lastSyncAt, now)).toBe(true);
  });
});

describe('Realtime WebSocket client contracts', () => {
  test('accepts complete sessions:update payload stats', () => {
    expect(isSessionStats({
      total: 2,
      running: 1,
      waiting: 0,
      idle: 0,
      lost: 1,
      completed: 0,
    })).toBe(true);
  });

  test('rejects sessions:update payloads without stats', () => {
    expect(isSessionsUpdateData({ sessions: [] })).toBe(false);
  });

  test('accepts named sync:complete payloads', () => {
    expect(isSyncCompleteData({ timestamp: '2026-07-03T00:00:00.000Z' })).toBe(true);
    expect(isSyncCompleteData({ timestamp: '' })).toBe(false);
  });
});
