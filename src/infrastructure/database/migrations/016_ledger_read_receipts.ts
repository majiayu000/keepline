import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration016: Migration = {
  version: 16, name: 'ledger_read_receipts',
  up() {
    execSql('ALTER TABLE ledger_views ADD COLUMN last_viewed_turn_id TEXT');
  },
};
