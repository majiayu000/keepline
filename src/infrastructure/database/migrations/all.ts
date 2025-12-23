/**
 * All migrations registry
 *
 * Import all migrations and export them as an array.
 */

import type { Migration } from './index.js';
import { migration001 } from './001_initial.js';

/** All available migrations */
export const allMigrations: Migration[] = [
  migration001,
  // Add new migrations here as they are created
  // migration002,
  // migration003,
];
