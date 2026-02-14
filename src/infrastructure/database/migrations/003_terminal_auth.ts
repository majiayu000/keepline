/**
 * Migration 003: Terminal auth & sessions
 *
 * Creates tables for web terminal authentication and PTY session tracking.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration003: Migration = {
  version: 3,
  name: 'terminal_auth',

  up: () => {
    // Users table for terminal auth
    execSql(`
      CREATE TABLE IF NOT EXISTS terminal_users (
        id TEXT PRIMARY KEY,
        username TEXT UNIQUE NOT NULL,
        password_hash TEXT NOT NULL,
        totp_secret TEXT,
        totp_enabled INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
      )
    `);

    // Auth sessions (JWT tracking for revocation)
    execSql(`
      CREATE TABLE IF NOT EXISTS terminal_auth_sessions (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        token_hash TEXT NOT NULL,
        expires_at TEXT NOT NULL,
        revoked INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES terminal_users(id) ON DELETE CASCADE
      )
    `);

    // Audit log
    execSql(`
      CREATE TABLE IF NOT EXISTS terminal_audit_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id TEXT,
        action TEXT NOT NULL,
        ip TEXT,
        details TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
      )
    `);

    // PTY session tracking
    execSql(`
      CREATE TABLE IF NOT EXISTS terminal_sessions (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        pid INTEGER,
        cwd TEXT,
        status TEXT NOT NULL DEFAULT 'running',
        exit_code INTEGER,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES terminal_users(id) ON DELETE CASCADE
      )
    `);

    // Indexes
    execSql(`
      CREATE INDEX IF NOT EXISTS idx_terminal_auth_sessions_user ON terminal_auth_sessions(user_id);
      CREATE INDEX IF NOT EXISTS idx_terminal_auth_sessions_expires ON terminal_auth_sessions(expires_at);
      CREATE INDEX IF NOT EXISTS idx_terminal_audit_log_user ON terminal_audit_log(user_id);
      CREATE INDEX IF NOT EXISTS idx_terminal_audit_log_created ON terminal_audit_log(created_at);
      CREATE INDEX IF NOT EXISTS idx_terminal_sessions_user ON terminal_sessions(user_id);
      CREATE INDEX IF NOT EXISTS idx_terminal_sessions_status ON terminal_sessions(status);
    `);
  },

  down: () => {
    execSql('DROP INDEX IF EXISTS idx_terminal_sessions_status');
    execSql('DROP INDEX IF EXISTS idx_terminal_sessions_user');
    execSql('DROP INDEX IF EXISTS idx_terminal_audit_log_created');
    execSql('DROP INDEX IF EXISTS idx_terminal_audit_log_user');
    execSql('DROP INDEX IF EXISTS idx_terminal_auth_sessions_expires');
    execSql('DROP INDEX IF EXISTS idx_terminal_auth_sessions_user');
    execSql('DROP TABLE IF EXISTS terminal_sessions');
    execSql('DROP TABLE IF EXISTS terminal_audit_log');
    execSql('DROP TABLE IF EXISTS terminal_auth_sessions');
    execSql('DROP TABLE IF EXISTS terminal_users');
  },
};
