/**
 * Migration 008: Work item agent-session evidence layer.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration008: Migration = {
  version: 8,
  name: 'work_item_evidence',

  up: () => {
    execSql(`
      CREATE TABLE IF NOT EXISTS agent_sessions (
        id TEXT PRIMARY KEY,
        runtime_id TEXT NOT NULL,
        runtime_session_id TEXT NOT NULL,
        project_root TEXT,
        cwd TEXT NOT NULL,
        status TEXT NOT NULL,
        title TEXT NOT NULL,
        last_active_at TEXT NOT NULL,
        evidence_summary TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        UNIQUE(runtime_id, runtime_session_id)
      )
    `);

    execSql(`
      CREATE TABLE IF NOT EXISTS work_item_session_links (
        id TEXT PRIMARY KEY,
        work_item_id TEXT NOT NULL,
        agent_session_id TEXT NOT NULL,
        link_source TEXT NOT NULL CHECK (link_source IN ('user', 'accepted_agent_suggestion', 'heuristic_suggestion')),
        acceptance_status TEXT NOT NULL CHECK (acceptance_status IN ('accepted', 'pending', 'rejected')),
        accepted_at TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (work_item_id) REFERENCES work_items(id) ON DELETE CASCADE,
        FOREIGN KEY (agent_session_id) REFERENCES agent_sessions(id) ON DELETE CASCADE,
        UNIQUE(work_item_id, agent_session_id)
      )
    `);

    execSql(`
      CREATE TABLE IF NOT EXISTS progress_evidence (
        id TEXT PRIMARY KEY,
        work_item_id TEXT,
        agent_session_id TEXT,
        runtime_id TEXT,
        kind TEXT NOT NULL CHECK (kind IN ('message', 'tool_call', 'file_change', 'plan_event', 'test_result')),
        outcome TEXT CHECK (outcome IS NULL OR outcome IN ('progress', 'blocked', 'completed', 'failed')),
        summary TEXT NOT NULL,
        source_path TEXT,
        occurred_at TEXT NOT NULL,
        confidence TEXT NOT NULL CHECK (confidence IN ('explicit', 'inferred')),
        metadata TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (work_item_id) REFERENCES work_items(id) ON DELETE CASCADE,
        FOREIGN KEY (agent_session_id) REFERENCES agent_sessions(id) ON DELETE CASCADE,
        CHECK (work_item_id IS NOT NULL OR agent_session_id IS NOT NULL)
      )
    `);

    execSql(`
      CREATE INDEX IF NOT EXISTS idx_agent_sessions_runtime ON agent_sessions(runtime_id, runtime_session_id);
      CREATE INDEX IF NOT EXISTS idx_agent_sessions_project_root ON agent_sessions(project_root);
      CREATE INDEX IF NOT EXISTS idx_work_item_session_links_work_item ON work_item_session_links(work_item_id);
      CREATE INDEX IF NOT EXISTS idx_work_item_session_links_agent_session ON work_item_session_links(agent_session_id);
      CREATE INDEX IF NOT EXISTS idx_work_item_session_links_acceptance ON work_item_session_links(acceptance_status);
      CREATE INDEX IF NOT EXISTS idx_progress_evidence_work_item ON progress_evidence(work_item_id);
      CREATE INDEX IF NOT EXISTS idx_progress_evidence_agent_session ON progress_evidence(agent_session_id);
      CREATE INDEX IF NOT EXISTS idx_progress_evidence_kind ON progress_evidence(kind);
      CREATE INDEX IF NOT EXISTS idx_progress_evidence_occurred_at ON progress_evidence(occurred_at);
    `);
  },

  down: () => {
    execSql('DROP TABLE IF EXISTS progress_evidence');
    execSql('DROP TABLE IF EXISTS work_item_session_links');
    execSql('DROP TABLE IF EXISTS agent_sessions');
  },
};
