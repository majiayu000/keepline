export const REALTIME_POLL_INTERVAL_MS = 5_000;
export const REALTIME_FULL_SYNC_INTERVAL_MS = 30_000;

export function shouldRunRealtimeFullSync(
  lastFullSyncAt: number,
  now: number,
  intervalMs: number = REALTIME_FULL_SYNC_INTERVAL_MS
): boolean {
  return lastFullSyncAt === 0 || now - lastFullSyncAt >= intervalMs;
}
