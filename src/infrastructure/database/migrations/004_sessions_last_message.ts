/**
 * Migration 004: Session last_message compatibility
 *
 * Older databases may already have the sessions table without the
 * later-added last_message column. Add it idempotently.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';
import { logger } from '../../../lib/logger.js';

export const migration004: Migration = {
  version: 4,
  name: 'sessions_last_message_compat',

  up: () => {
    try {
      execSql('ALTER TABLE sessions ADD COLUMN last_message TEXT');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message.includes('duplicate column') || message.includes('already exists')) {
        logger.debug('sessions.last_message already exists');
        return;
      }
      throw error;
    }
  },

  down: () => {
    // SQLite cannot drop columns without table rebuild. Keep as no-op.
  },
};
