/**
 * Migration 001: Initial schema
 *
 * Creates the base tables for Keepline.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration001: Migration = {
  version: 1,
  name: 'initial_schema',

  up: () => {
    // Create sessions table
    execSql(`
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
        last_message TEXT,
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
    execSql(`
      CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
      CREATE INDEX IF NOT EXISTS idx_sessions_directory ON sessions(directory);
      CREATE INDEX IF NOT EXISTS idx_sessions_last_active ON sessions(last_active_at);
      CREATE INDEX IF NOT EXISTS idx_sessions_pid ON sessions(pid);
    `);

    // Create tool_usage table
    execSql(`
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

    // Create hook_events table
    execSql(`
      CREATE TABLE IF NOT EXISTS hook_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT,
        event_type TEXT NOT NULL,
        payload TEXT,
        timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
      )
    `);

    // Create metadata table
    execSql(`
      CREATE TABLE IF NOT EXISTS metadata (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
      )
    `);
  },

  down: () => {
    execSql('DROP TABLE IF EXISTS tool_usage');
    execSql('DROP TABLE IF EXISTS hook_events');
    execSql('DROP TABLE IF EXISTS metadata');
    execSql('DROP TABLE IF EXISTS sessions');
  },
};
