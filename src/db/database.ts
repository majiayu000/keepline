/**
 * Legacy database re-exports.
 *
 * Keep this file as a compatibility shim so old imports reuse the
 * infrastructure-layer singleton rather than opening a second DB handle.
 */

export {
  getDatabase,
  closeDatabase,
  transaction,
  isDatabaseInitialized,
  runSql,
  execSql,
  queryAll,
  queryOne,
} from '../infrastructure/database/sqlite.js';
