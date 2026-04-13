/**
 * Database module exports
 *
 * Re-exports from infrastructure layer for backward compatibility.
 * New code should import directly from '../infrastructure/database/index.js'
 */

// Re-export from infrastructure layer
export {
  getDatabase,
  closeDatabase,
  transaction,
  isDatabaseInitialized,
} from '../infrastructure/database/sqlite.js';

// Re-export migrations (keeping old API)
export { runMigrations, resetDatabase } from './migrations.js';

// Re-export session repository as namespace for compatibility
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

export const sessionRepo = {
  findBySessionId: sessionRepository.findBySessionId.bind(sessionRepository),
  findBySessionIds: sessionRepository.findBySessionIds.bind(sessionRepository),
  findAll: sessionRepository.findAll.bind(sessionRepository),
  findAllLightweight: sessionRepository.findAllLightweight.bind(sessionRepository),
  findByStatus: sessionRepository.findByStatus.bind(sessionRepository),
  findByDirectory: sessionRepository.findByDirectory.bind(sessionRepository),
  findActive: sessionRepository.findActive.bind(sessionRepository),
  findActiveLightweight: sessionRepository.findActiveLightweight.bind(sessionRepository),
  upsert: sessionRepository.upsert.bind(sessionRepository),
  deleteOldSessions: sessionRepository.deleteOldSessions.bind(sessionRepository),
};

// Also export the repository instance directly
export { sessionRepository };
