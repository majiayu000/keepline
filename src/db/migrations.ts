/**
 * Database migrations
 */

import { getDatabase } from './database.js';
import { logger } from '../lib/logger.js';

/** Run all migrations */
export function runMigrations(): void {
  const db = getDatabase();

  // Create sessions table
  db.exec(`
    CREATE TABLE IF NOT EXISTS sessions (
      id TEXT PRIMARY KEY,
      session_id TEXT UNIQUE NOT NULL,
      directory TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'idle',
      title TEXT,
      initial_prompt TEXT NOT NULL,
      last_tool TEXT,
      last_tool_input TEXT,
      current_file TEXT,
      started_at TEXT,
      last_active_at TEXT NOT NULL,
      completed_at TEXT,
      pid INTEGER,
      tty TEXT,
      tool_count INTEGER DEFAULT 0,
      message_count INTEGER DEFAULT 0,
      created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )
  `);

  // Create indexes
  db.exec(`
    CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
    CREATE INDEX IF NOT EXISTS idx_sessions_directory ON sessions(directory);
    CREATE INDEX IF NOT EXISTS idx_sessions_last_active ON sessions(last_active_at);
    CREATE INDEX IF NOT EXISTS idx_sessions_pid ON sessions(pid);
  `);

  // Create tool_usage table for detailed tracking
  db.exec(`
    CREATE TABLE IF NOT EXISTS tool_usage (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      session_id TEXT NOT NULL,
      tool TEXT NOT NULL,
      input TEXT,
      output TEXT,
      duration INTEGER,
      timestamp TEXT NOT NULL,
      FOREIGN KEY (session_id) REFERENCES sessions(session_id)
    )
  `);

  // Create hooks table for event tracking
  db.exec(`
    CREATE TABLE IF NOT EXISTS hook_events (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      session_id TEXT,
      event_type TEXT NOT NULL,
      payload TEXT,
      timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )
  `);

  // Create metadata table for app state
  db.exec(`
    CREATE TABLE IF NOT EXISTS metadata (
      key TEXT PRIMARY KEY,
      value TEXT NOT NULL,
      updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )
  `);

  // Migration: Add last_message column if not exists
  try {
    db.exec(`ALTER TABLE sessions ADD COLUMN last_message TEXT`);
    logger.debug('Added last_message column');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    // Only ignore "duplicate column" errors, log others
    if (message.includes('duplicate column') || message.includes('already exists')) {
      logger.debug('last_message column already exists');
    } else {
      logger.error('Failed to add last_message column', { error: message });
    }
  }

  logger.info('Database migrations completed');
}

/** Reset database (for testing) */
export function resetDatabase(): void {
  const db = getDatabase();
  db.exec(`
    DROP TABLE IF EXISTS tool_usage;
    DROP TABLE IF EXISTS hook_events;
    DROP TABLE IF EXISTS sessions;
    DROP TABLE IF EXISTS metadata;
  `);
  runMigrations();
  logger.info('Database reset completed');
}
