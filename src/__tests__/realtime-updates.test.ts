import { describe, expect, test } from 'bun:test';
import {
  REALTIME_FULL_SYNC_INTERVAL_MS,
  shouldRunRealtimeFullSync,
} from '../web/api/realtime-updates.js';

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
