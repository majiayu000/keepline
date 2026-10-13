import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration018: Migration = {
  version: 18, name: 'session_agent_identity',
  up() {
    execSql('ALTER TABLE sessions ADD COLUMN agent_role TEXT');
    execSql('ALTER TABLE sessions ADD COLUMN agent_nickname TEXT');
  },
};
