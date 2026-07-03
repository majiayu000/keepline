/**
 * SQLite database management (using Bun's built-in SQLite)
 */

import { Database } from 'bun:sqlite';
import { existsSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { ensureKeeplineDataHome, getKeeplineDb } from '../../lib/paths.js';
import { DatabaseError } from '../../lib/errors.js';
import { logger } from '../../lib/logger.js';

let db: Database | null = null;

/** Get database instance (singleton) */
export function getDatabase(): Database {
  if (db) return db;

  try {
    ensureKeeplineDataHome();
    // Ensure directory exists
    const dbPath = getKeeplineDb();
    const dbDir = dirname(dbPath);
    if (!existsSync(dbDir)) {
      mkdirSync(dbDir, { recursive: true });
    }

    db = new Database(dbPath);
    db.exec('PRAGMA journal_mode = WAL');
    db.exec('PRAGMA busy_timeout = 5000');
    db.exec('PRAGMA foreign_keys = ON');

    logger.debug('Database connection established');
    return db;
  } catch (error) {
    throw new DatabaseError('Failed to connect to database', {
      path: getKeeplineDb(),
      error: (error as Error).message,
    });
  }
}

/** Close database connection */
export function closeDatabase(): void {
  if (db) {
    db.close();
    db = null;
    logger.debug('Database connection closed');
  }
}

/** Execute a transaction */
export function transaction<T>(fn: () => T): T {
  const database = getDatabase();
  return database.transaction(fn)();
}

/** Check if database is initialized */
export function isDatabaseInitialized(): boolean {
  try {
    const database = getDatabase();
    const result = database
      .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'")
      .get();
    return !!result;
  } catch {
    return false;
  }
}

/** SQLite bind parameter type */
export type SQLBindParam = string | number | bigint | boolean | null | Uint8Array;

/** Run raw SQL query */
export function runSql(sql: string, params: SQLBindParam[] = []): void {
  const database = getDatabase();
  database.prepare(sql).run(...params);
}

/** Execute raw SQL (for DDL statements) */
export function execSql(sql: string): void {
  const database = getDatabase();
  database.exec(sql);
}

/** Query database and return all rows */
export function queryAll<T>(sql: string, params: SQLBindParam[] = []): T[] {
  const database = getDatabase();
  return database.prepare(sql).all(...params) as T[];
}

/** Query database and return first row */
export function queryOne<T>(sql: string, params: SQLBindParam[] = []): T | undefined {
  const database = getDatabase();
  return database.prepare(sql).get(...params) as T | undefined;
}
