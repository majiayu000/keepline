/**
 * All migrations registry
 *
 * Import all migrations and export them as an array.
 */

import type { Migration } from './index.js';
import { migration001 } from './001_initial.js';
import { migration002 } from './002_memory.js';

/** All available migrations */
export const allMigrations: Migration[] = [
  migration001,
  migration002,
  // Add new migrations here as they are created
  // migration003,
];
