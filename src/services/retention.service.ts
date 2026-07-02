import { config } from '../lib/config.js';
import { logger } from '../lib/logger.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { eventStore } from '../infrastructure/events/store.js';

export interface RetentionCleanupResult {
  disabled: boolean;
  retentionDays: number;
  cutoff?: Date;
  sessionsDeleted: number;
  eventsDeleted: number;
}

function retentionCutoff(retentionDays: number, now: Date): Date {
  const cutoff = new Date(now);
  cutoff.setDate(cutoff.getDate() - retentionDays);
  return cutoff;
}

export async function runRetentionCleanup(
  retentionDays: number = config.get().retentionDays,
  now: Date = new Date()
): Promise<RetentionCleanupResult> {
  if (retentionDays <= 0) {
    logger.debug('Retention cleanup disabled', { retentionDays });
    return {
      disabled: true,
      retentionDays,
      sessionsDeleted: 0,
      eventsDeleted: 0,
    };
  }

  const cutoff = retentionCutoff(retentionDays, now);
  const sessionsDeleted = sessionRepository.deleteOldSessions(retentionDays, now);
  const eventsDeleted = await eventStore.deleteOlderThan(cutoff);

  logger.info('Retention cleanup completed', {
    retentionDays,
    cutoff: cutoff.toISOString(),
    sessionsDeleted,
    eventsDeleted,
  });

  return {
    disabled: false,
    retentionDays,
    cutoff,
    sessionsDeleted,
    eventsDeleted,
  };
}
