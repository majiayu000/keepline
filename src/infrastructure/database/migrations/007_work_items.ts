/**
 * Migration 007: Work item foundation.
 *
 * Adds durable Inbox/Todo/Idea capture without requiring a project.
 */

import type { Migration } from './index.js';
import { execSql } from '../sqlite.js';

export const migration007: Migration = {
  version: 7,
  name: 'work_items',

  up: () => {
    execSql(`
      CREATE TABLE IF NOT EXISTS areas (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        project_root TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
      )
    `);

    execSql(`
      CREATE TABLE IF NOT EXISTS work_items (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL CHECK (kind IN ('todo', 'idea', 'note', 'project_task')),
        status TEXT NOT NULL CHECK (status IN ('inbox', 'planned', 'active', 'blocked', 'done', 'archived')),
        title TEXT NOT NULL,
        body TEXT,
        project_root TEXT,
        area_id TEXT,
        status_source TEXT NOT NULL CHECK (status_source IN ('user', 'accepted_agent_suggestion')),
        completed_at TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (area_id) REFERENCES areas(id) ON DELETE SET NULL
      )
    `);

    execSql(`
      CREATE INDEX IF NOT EXISTS idx_work_items_status ON work_items(status);
      CREATE INDEX IF NOT EXISTS idx_work_items_kind ON work_items(kind);
      CREATE INDEX IF NOT EXISTS idx_work_items_project_root ON work_items(project_root);
      CREATE INDEX IF NOT EXISTS idx_work_items_updated_at ON work_items(updated_at);
      CREATE INDEX IF NOT EXISTS idx_work_items_area_id ON work_items(area_id);
      CREATE INDEX IF NOT EXISTS idx_areas_project_root ON areas(project_root);
    `);
  },

  down: () => {
    execSql('DROP TABLE IF EXISTS work_items');
    execSql('DROP TABLE IF EXISTS areas');
  },
};
