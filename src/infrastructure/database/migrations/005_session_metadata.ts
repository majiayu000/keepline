/**
 * Migration 005: Session multi-agent metadata, usage stats, and tool calls
 *
 * Persists parser-produced fields that were previously dropped at the
 * persistence boundary (issue #13): sub-agent tracking (agent_id /
 * parent_session_id / is_sub_agent), usage stats (tokens / cost / api_calls),
 * and the tool_calls array (stored as JSON, consistent with last_tool_input).
 *
 * Columns are added idempotently so existing databases upgrade cleanly.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';
import { logger } from '../../../lib/logger.js';

const COLUMNS: Array<[string, string]> = [
  ['agent_id', 'TEXT'],
  ['parent_session_id', 'TEXT'],
  ['is_sub_agent', 'INTEGER NOT NULL DEFAULT 0'],
  ['total_input_tokens', 'INTEGER'],
  ['total_output_tokens', 'INTEGER'],
  ['total_tokens', 'INTEGER'],
  ['total_cost', 'REAL'],
  ['api_calls', 'INTEGER'],
  ['tool_calls', 'TEXT'],
];

export const migration005: Migration = {
  version: 5,
  name: 'session_metadata_and_usage',

  up: () => {
    for (const [name, type] of COLUMNS) {
      try {
        execSql(`ALTER TABLE sessions ADD COLUMN ${name} ${type}`);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (message.includes('duplicate column') || message.includes('already exists')) {
          logger.debug(`sessions.${name} already exists`);
          continue;
        }
        throw error;
      }
    }
  },

  down: () => {
    // SQLite cannot drop columns without a table rebuild. Keep as no-op.
  },
};
