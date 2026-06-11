/**
 * All migrations registry
 *
 * Import all migrations and export them as an array.
 */

import type { Migration } from './index.js';
import { migration001 } from './001_initial.js';
import { migration002 } from './002_memory.js';
import { migration003 } from './003_terminal_auth.js';
import { migration004 } from './004_sessions_last_message.js';
import { migration005 } from './005_session_metadata.js';

/** All available migrations */
export const allMigrations: Migration[] = [
  migration001,
  migration002,
  migration003,
  migration004,
  migration005,
];
