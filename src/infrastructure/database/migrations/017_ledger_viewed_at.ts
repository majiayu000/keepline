import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration017: Migration = {
  version: 17, name: 'ledger_viewed_at',
  up() {
    execSql('ALTER TABLE ledger_views ADD COLUMN last_viewed_at TEXT');
  },
};
