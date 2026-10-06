import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration015: Migration = {
  version: 15, name: 'progress_ledger',
  up() {
    execSql(`
      ALTER TABLE work_items ADD COLUMN parent_id TEXT REFERENCES work_items(id) ON DELETE RESTRICT;
      ALTER TABLE work_items ADD COLUMN level TEXT NOT NULL DEFAULT 'task';
      ALTER TABLE work_items ADD COLUMN outcome TEXT;
      ALTER TABLE work_items ADD COLUMN acceptance TEXT NOT NULL DEFAULT '[]';
      ALTER TABLE sessions ADD COLUMN status_reason TEXT;
      CREATE INDEX idx_work_items_parent ON work_items(parent_id);
      CREATE TABLE ledger_asks (
        id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
        ordinal INTEGER NOT NULL, text TEXT NOT NULL, authored_text TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('initial','addition','question')),
        occurred_at TEXT NOT NULL, created_at TEXT NOT NULL, turn_id TEXT
      );
      CREATE TABLE requirement_items (
        id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
        ordinal INTEGER NOT NULL, title TEXT NOT NULL, anchors TEXT NOT NULL DEFAULT '{}', constraints TEXT NOT NULL DEFAULT '[]',
        source TEXT NOT NULL CHECK(source IN ('work_item','model','fallback','user')),
        status TEXT NOT NULL CHECK(status IN ('todo','doing','done','unverified')),
        status_source TEXT NOT NULL CHECK(status_source IN ('rule','model','user')),
        evidence_ids TEXT NOT NULL DEFAULT '[]', deleted_by_user INTEGER NOT NULL DEFAULT 0,
        checklist_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
      );
      CREATE TABLE ledger_corrections (
        id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
        step_call_id TEXT NOT NULL, requirement_item_id TEXT REFERENCES requirement_items(id) ON DELETE SET NULL,
        accept_off_plan INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL,
        UNIQUE(agent_session_id, step_call_id)
      );
      CREATE TABLE ledger_rules (
        id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
        matcher TEXT NOT NULL, requirement_item_id TEXT NOT NULL REFERENCES requirement_items(id) ON DELETE CASCADE,
        created_at TEXT NOT NULL
      );
      CREATE TABLE ledger_acceptances (
        id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE, turn_id TEXT NOT NULL,
        decision TEXT NOT NULL CHECK(decision IN ('accepted','accepted_with_gaps','follow_up')),
        dropped_item_ids TEXT NOT NULL DEFAULT '[]', reason TEXT, created_at TEXT NOT NULL,
        UNIQUE(agent_session_id, turn_id)
      );
      CREATE TABLE ledger_alerts (
        id TEXT PRIMARY KEY, agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK(kind IN ('needs_input','off_plan','claimed_unverified','stalled','limited')),
        detail TEXT NOT NULL, raised_at TEXT NOT NULL, cleared_at TEXT, notified INTEGER NOT NULL DEFAULT 0
      );
      CREATE INDEX idx_ledger_alerts_session ON ledger_alerts(agent_session_id,kind,raised_at);
      CREATE INDEX idx_requirement_items_session ON requirement_items(agent_session_id);
      CREATE INDEX idx_ledger_asks_session ON ledger_asks(agent_session_id);
      CREATE TABLE ledger_views (session_id TEXT PRIMARY KEY, expires_at TEXT NOT NULL);
      CREATE TABLE hook_spool_events (id TEXT PRIMARY KEY, processed_at TEXT NOT NULL);
      CREATE TABLE ledger_judgments (
        agent_session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
        ask_id TEXT NOT NULL, judged_at TEXT NOT NULL, items TEXT,
        PRIMARY KEY(agent_session_id,ask_id)
      );
      CREATE TABLE ledger_attributions (
        agent_session_id TEXT PRIMARY KEY REFERENCES agent_sessions(id) ON DELETE CASCADE,
        no_goal INTEGER NOT NULL DEFAULT 0, carry_item_id TEXT REFERENCES work_items(id) ON DELETE SET NULL
      );
    `);
  },
};
