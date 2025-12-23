/**
 * Database migration system
 *
 * Manages versioned schema migrations.
 */

import { getDatabase, execSql, queryOne, runSql } from '../sqlite.js';
import { logger } from '../../../lib/logger.js';

/** Migration interface */
export interface Migration {
  version: number;
  name: string;
  up: () => void;
  down?: () => void;
}

/** Migration record in database */
interface MigrationRecord {
  version: number;
  name: string;
  applied_at: string;
}

/** Ensure migrations table exists */
function ensureMigrationsTable(): void {
  execSql(`
    CREATE TABLE IF NOT EXISTS schema_migrations (
      version INTEGER PRIMARY KEY,
      name TEXT NOT NULL,
      applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )
  `);
}

/** Get current schema version */
export function getCurrentVersion(): number {
  ensureMigrationsTable();
  const result = queryOne<{ version: number }>(
    'SELECT MAX(version) as version FROM schema_migrations'
  );
  return result?.version ?? 0;
}

/** Check if migration was applied */
export function isMigrationApplied(version: number): boolean {
  ensureMigrationsTable();
  const result = queryOne<MigrationRecord>(
    'SELECT * FROM schema_migrations WHERE version = ?',
    [version]
  );
  return !!result;
}

/** Record migration as applied */
function recordMigration(migration: Migration): void {
  runSql(
    'INSERT INTO schema_migrations (version, name) VALUES (?, ?)',
    [migration.version, migration.name]
  );
}

/** Remove migration record */
function removeMigrationRecord(version: number): void {
  runSql('DELETE FROM schema_migrations WHERE version = ?', [version]);
}

/** Run a single migration */
export function runMigration(migration: Migration): void {
  if (isMigrationApplied(migration.version)) {
    logger.debug(`Migration ${migration.version} (${migration.name}) already applied`);
    return;
  }

  logger.info(`Running migration ${migration.version}: ${migration.name}`);

  const db = getDatabase();
  db.transaction(() => {
    migration.up();
    recordMigration(migration);
  })();

  logger.info(`Migration ${migration.version} completed`);
}

/** Rollback a migration */
export function rollbackMigration(migration: Migration): void {
  if (!isMigrationApplied(migration.version)) {
    logger.debug(`Migration ${migration.version} not applied, nothing to rollback`);
    return;
  }

  if (!migration.down) {
    throw new Error(`Migration ${migration.version} does not support rollback`);
  }

  logger.info(`Rolling back migration ${migration.version}: ${migration.name}`);

  const db = getDatabase();
  db.transaction(() => {
    migration.down!();
    removeMigrationRecord(migration.version);
  })();

  logger.info(`Migration ${migration.version} rolled back`);
}

/** Run all pending migrations */
export function runAllMigrations(migrations: Migration[]): void {
  // Sort by version
  const sorted = [...migrations].sort((a, b) => a.version - b.version);

  for (const migration of sorted) {
    runMigration(migration);
  }
}

/** Get all applied migrations */
export function getAppliedMigrations(): MigrationRecord[] {
  ensureMigrationsTable();
  const db = getDatabase();
  return db.prepare('SELECT * FROM schema_migrations ORDER BY version').all() as MigrationRecord[];
}
