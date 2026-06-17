/**
 * Migration 006: Session client discriminator.
 *
 * Keepline monitors multiple agent clients. Existing rows are Claude rows.
 * Codex sessions are persisted with a scoped session_id such as
 * `codex_<uuid>` to preserve the existing session_id UNIQUE constraint.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';
import { logger } from '../../../lib/logger.js';

export const migration006: Migration = {
  version: 6,
  name: 'session_client',

  up: () => {
    try {
      execSql("ALTER TABLE sessions ADD COLUMN client TEXT NOT NULL DEFAULT 'claude'");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!message.includes('duplicate column') && !message.includes('already exists')) {
        throw error;
      }
      logger.debug('sessions.client already exists');
    }

    execSql("UPDATE sessions SET client = 'claude' WHERE client IS NULL OR client = ''");
    execSql('CREATE INDEX IF NOT EXISTS idx_sessions_client ON sessions(client)');
  },

  down: () => {
    // SQLite cannot drop columns without a table rebuild. Keep as no-op.
    execSql('DROP INDEX IF EXISTS idx_sessions_client');
  },
};
