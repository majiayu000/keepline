/**
 * Migration 002: Memory system
 *
 * Creates the session_memories table for the "relay race" pattern.
 * Stores context that persists across session recoveries.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration002: Migration = {
  version: 2,
  name: 'memory_system',

  up: () => {
    // Create session_memories table
    execSql(`
      CREATE TABLE IF NOT EXISTS session_memories (
        id TEXT PRIMARY KEY,
        session_id TEXT UNIQUE NOT NULL,
        directory TEXT NOT NULL,

        -- Progress tracking
        last_progress TEXT DEFAULT '',
        pending_tasks TEXT DEFAULT '[]',
        completed_tasks TEXT DEFAULT '[]',

        -- Context information
        known_issues TEXT DEFAULT '[]',
        decisions TEXT DEFAULT '[]',
        notes TEXT DEFAULT '',

        -- Handoff information (for next iteration)
        handoff_notes TEXT DEFAULT '',
        handoff_priority TEXT DEFAULT '[]',

        -- Metadata
        iteration_count INTEGER DEFAULT 0,
        total_tokens_used INTEGER DEFAULT 0,

        -- Timestamps
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,

        -- Foreign key to sessions (optional, memory can outlive session)
        FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE SET NULL
      )
    `);

    // Create indexes for fast lookups
    execSql(`
      CREATE INDEX IF NOT EXISTS idx_memories_session_id ON session_memories(session_id);
      CREATE INDEX IF NOT EXISTS idx_memories_directory ON session_memories(directory);
      CREATE INDEX IF NOT EXISTS idx_memories_updated_at ON session_memories(updated_at);
    `);
  },

  down: () => {
    execSql('DROP INDEX IF EXISTS idx_memories_updated_at');
    execSql('DROP INDEX IF EXISTS idx_memories_directory');
    execSql('DROP INDEX IF EXISTS idx_memories_session_id');
    execSql('DROP TABLE IF EXISTS session_memories');
  },
};
