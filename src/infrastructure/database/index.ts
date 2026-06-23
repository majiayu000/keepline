/**
 * Database layer exports
 */

// SQLite connection
export {
  getDatabase,
  closeDatabase,
  transaction,
  isDatabaseInitialized,
  runSql,
  execSql,
  queryAll,
  queryOne,
} from './sqlite.js';

// Migrations
export {
  runMigration,
  rollbackMigration,
  runAllMigrations,
  getCurrentVersion,
  isMigrationApplied,
  getAppliedMigrations,
} from './migrations/index.js';

export type { Migration } from './migrations/index.js';

export { allMigrations } from './migrations/all.js';

// Repositories
export { sessionRepository } from './repositories/session.repository.js';
export { memoryRepository } from './repositories/memory.repository.js';
export type { IMemoryRepository } from './repositories/memory.repository.js';
export { workItemRepository } from './repositories/work-item.repository.js';
export type { IWorkItemRepository } from './repositories/work-item.repository.js';
