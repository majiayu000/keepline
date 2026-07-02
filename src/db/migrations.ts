import { getDatabase } from '../infrastructure/database/sqlite.js';
import { runAllMigrations, allMigrations } from '../infrastructure/database/index.js';
import { logger } from '../lib/logger.js';

/** Run all migrations */
export function runMigrations(): void {
  runAllMigrations(allMigrations);
  logger.info('Database migrations completed');
}

/** Reset database (for testing) */
export function resetDatabase(): void {
  const db = getDatabase();
  db.exec(`
    DROP TABLE IF EXISTS progress_evidence;
    DROP TABLE IF EXISTS session_digests;
    DROP TABLE IF EXISTS work_item_session_links;
    DROP TABLE IF EXISTS agent_sessions;
    DROP TABLE IF EXISTS work_items;
    DROP TABLE IF EXISTS areas;
    DROP TABLE IF EXISTS terminal_sessions;
    DROP TABLE IF EXISTS terminal_audit_log;
    DROP TABLE IF EXISTS terminal_auth_sessions;
    DROP TABLE IF EXISTS terminal_users;
    DROP TABLE IF EXISTS session_memories;
    DROP TABLE IF EXISTS tool_usage;
    DROP TABLE IF EXISTS hook_events;
    DROP TABLE IF EXISTS events;
    DROP TABLE IF EXISTS sessions;
    DROP TABLE IF EXISTS metadata;
    DROP TABLE IF EXISTS schema_migrations;
  `);
  runMigrations();
  logger.info('Database reset completed');
}
