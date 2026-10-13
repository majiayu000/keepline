import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration019: Migration = {
  version: 19, name: 'session_process_state',
  up() {
    execSql("ALTER TABLE sessions ADD COLUMN process_state TEXT NOT NULL DEFAULT 'unknown'");
  },
};
