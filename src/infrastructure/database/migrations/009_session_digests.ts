/**
 * Migration 009: Orchestrator session digests.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration009: Migration = {
  version: 9,
  name: 'session_digests',

  up: () => {
    execSql(`
      CREATE TABLE IF NOT EXISTS session_digests (
        id TEXT PRIMARY KEY,
        session_id TEXT UNIQUE NOT NULL,
        summary TEXT NOT NULL DEFAULT '',
        next_actions TEXT NOT NULL DEFAULT '[]',
        blockers TEXT NOT NULL DEFAULT '[]',
        waiting_for_human INTEGER NOT NULL DEFAULT 0,
        source TEXT NOT NULL CHECK (source IN ('deterministic', 'local_model')),
        status TEXT NOT NULL CHECK (status IN ('fresh', 'stale', 'error')),
        source_updated_at TEXT NOT NULL,
        generated_at TEXT NOT NULL,
        provider TEXT,
        error_message TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
      )
    `);

    execSql(`
      CREATE INDEX IF NOT EXISTS idx_session_digests_session_id ON session_digests(session_id);
      CREATE INDEX IF NOT EXISTS idx_session_digests_status ON session_digests(status);
      CREATE INDEX IF NOT EXISTS idx_session_digests_source ON session_digests(source);
      CREATE INDEX IF NOT EXISTS idx_session_digests_generated_at ON session_digests(generated_at);
    `);
  },

  down: () => {
    execSql('DROP INDEX IF EXISTS idx_session_digests_generated_at');
    execSql('DROP INDEX IF EXISTS idx_session_digests_source');
    execSql('DROP INDEX IF EXISTS idx_session_digests_status');
    execSql('DROP INDEX IF EXISTS idx_session_digests_session_id');
    execSql('DROP TABLE IF EXISTS session_digests');
  },
};
