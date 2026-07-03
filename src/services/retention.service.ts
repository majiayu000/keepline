import { config } from '../lib/config.js';
import { logger } from '../lib/logger.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

export interface RetentionCleanupResult {
  disabled: boolean;
  retentionDays: number;
  cutoff?: Date;
  sessionsDeleted: number;
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
    };
  }

  const cutoff = retentionCutoff(retentionDays, now);
  const sessionsDeleted = sessionRepository.deleteOldSessions(retentionDays, now);

  logger.info('Retention cleanup completed', {
    retentionDays,
    cutoff: cutoff.toISOString(),
    sessionsDeleted,
  });

  return {
    disabled: false,
    retentionDays,
    cutoff,
    sessionsDeleted,
  };
}
