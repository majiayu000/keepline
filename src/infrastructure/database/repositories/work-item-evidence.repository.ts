/**
 * Repository for AgentSession, WorkItemSessionLink, and ProgressEvidence records.
 */

import { randomUUID } from 'crypto';
import { getDatabase, transaction } from '../sqlite.js';
import type {
  AgentSession,
  AgentSessionUpsertInput,
  ProgressEvidence,
  ProgressEvidenceCreateInput,
  ProgressEvidenceConfidence,
  ProgressEvidenceKind,
  ProgressEvidenceOutcome,
  WorkItemSessionLink,
  WorkItemSessionLinkAcceptanceStatus,
  WorkItemSessionLinkCreateInput,
  WorkItemSessionLinkSource,
} from '../../../domain/work-item/index.js';
import { encodeAgentSessionId } from '../../../domain/work-item/index.js';
import type { RuntimeId } from '../../../domain/runtime/index.js';

interface AgentSessionRow {
  id: string;
  runtime_id: string;
  runtime_session_id: string;
  project_root: string | null;
  cwd: string;
  status: string;
  title: string;
  last_active_at: string;
  evidence_summary: string | null;
  created_at: string;
  updated_at: string;
}

interface WorkItemSessionLinkRow {
  id: string;
  work_item_id: string;
  agent_session_id: string;
  link_source: string;
  acceptance_status: string;
  accepted_at: string | null;
  created_at: string;
  updated_at: string;
}

interface ProgressEvidenceRow {
  id: string;
  work_item_id: string | null;
  agent_session_id: string | null;
  runtime_id: string | null;
  kind: string;
  outcome: string | null;
  summary: string;
  source_path: string | null;
  occurred_at: string;
  confidence: string;
  metadata: string | null;
  created_at: string;
}

function rowToAgentSession(row: AgentSessionRow): AgentSession {
  return {
    id: row.id,
    runtimeId: row.runtime_id as RuntimeId,
    runtimeSessionId: row.runtime_session_id,
    projectRoot: row.project_root || undefined,
    cwd: row.cwd,
    status: row.status as AgentSession['status'],
    title: row.title,
    lastActiveAt: new Date(row.last_active_at),
    evidenceSummary: row.evidence_summary || undefined,
    createdAt: new Date(row.created_at),
    updatedAt: new Date(row.updated_at),
  };
}

function rowToWorkItemSessionLink(row: WorkItemSessionLinkRow): WorkItemSessionLink {
  return {
    id: row.id,
    workItemId: row.work_item_id,
    agentSessionId: row.agent_session_id,
    linkSource: row.link_source as WorkItemSessionLinkSource,
    acceptanceStatus: row.acceptance_status as WorkItemSessionLinkAcceptanceStatus,
    acceptedAt: row.accepted_at ? new Date(row.accepted_at) : undefined,
    createdAt: new Date(row.created_at),
    updatedAt: new Date(row.updated_at),
  };
}

function parseMetadata(raw: string | null): Record<string, unknown> | undefined {
  if (!raw) return undefined;
  const parsed = JSON.parse(raw);
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Invalid progress evidence metadata payload');
  }
  return parsed as Record<string, unknown>;
}

function rowToProgressEvidence(row: ProgressEvidenceRow): ProgressEvidence {
  return {
    id: row.id,
    workItemId: row.work_item_id || undefined,
    agentSessionId: row.agent_session_id || undefined,
    runtimeId: row.runtime_id as RuntimeId | undefined,
    kind: row.kind as ProgressEvidenceKind,
    outcome: row.outcome as ProgressEvidenceOutcome | undefined,
    summary: row.summary,
    sourcePath: row.source_path || undefined,
    occurredAt: new Date(row.occurred_at),
    confidence: row.confidence as ProgressEvidenceConfidence,
    metadata: parseMetadata(row.metadata),
    createdAt: new Date(row.created_at),
  };
}

export interface IWorkItemEvidenceRepository {
  upsertAgentSession(input: AgentSessionUpsertInput): AgentSession;
  findAgentSessionById(id: string): AgentSession | null;
  createSessionLink(input: WorkItemSessionLinkCreateInput): WorkItemSessionLink;
  acceptSessionLink(id: string): WorkItemSessionLink | null;
  findSessionLinkById(id: string): WorkItemSessionLink | null;
  createProgressEvidence(input: ProgressEvidenceCreateInput): ProgressEvidence;
  findEvidenceById(id: string): ProgressEvidence | null;
}

class WorkItemEvidenceRepository implements IWorkItemEvidenceRepository {
  upsertAgentSession(input: AgentSessionUpsertInput): AgentSession {
    const db = getDatabase();
    const now = new Date().toISOString();
    const id = encodeAgentSessionId(input.runtimeId, input.runtimeSessionId);
    const lastActiveAt = (input.lastActiveAt ?? new Date()).toISOString();

    db.prepare(`
      INSERT INTO agent_sessions (
        id, runtime_id, runtime_session_id, project_root, cwd, status, title,
        last_active_at, evidence_summary, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET
        project_root = excluded.project_root,
        cwd = excluded.cwd,
        status = excluded.status,
        title = excluded.title,
        last_active_at = excluded.last_active_at,
        evidence_summary = excluded.evidence_summary,
        updated_at = excluded.updated_at
    `).run(
      id,
      input.runtimeId,
      input.runtimeSessionId,
      input.projectRoot ?? null,
      input.cwd,
      input.status,
      input.title,
      lastActiveAt,
      input.evidenceSummary ?? null,
      now,
      now
    );

    return this.findAgentSessionById(id)!;
  }

  findAgentSessionById(id: string): AgentSession | null {
    const db = getDatabase();
    const row = db.prepare('SELECT * FROM agent_sessions WHERE id = ?')
      .get(id) as AgentSessionRow | undefined;
    return row ? rowToAgentSession(row) : null;
  }

  createSessionLink(input: WorkItemSessionLinkCreateInput): WorkItemSessionLink {
    const db = getDatabase();
    const now = new Date().toISOString();
    const id = randomUUID();
    const linkSource = input.linkSource ?? 'user';
    const acceptanceStatus = linkSource === 'heuristic_suggestion' ? 'pending' : 'accepted';
    const acceptedAt = acceptanceStatus === 'accepted' ? now : null;

    return transaction(() => {
      const existing = this.findSessionLinkForPair(input.workItemId, input.agentSessionId);
      if (existing) {
        if (existing.acceptanceStatus === 'pending' && linkSource === 'user') {
          db.prepare(`
            UPDATE work_item_session_links
            SET link_source = 'user',
                acceptance_status = 'accepted',
                accepted_at = ?,
                updated_at = ?
            WHERE id = ?
          `).run(now, now, existing.id);

          return this.findSessionLinkById(existing.id)!;
        }
        return existing;
      }

      db.prepare(`
        INSERT INTO work_item_session_links (
          id, work_item_id, agent_session_id, link_source, acceptance_status,
          accepted_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        id,
        input.workItemId,
        input.agentSessionId,
        linkSource,
        acceptanceStatus,
        acceptedAt,
        now,
        now
      );

      return this.findSessionLinkById(id)!;
    });
  }

  acceptSessionLink(id: string): WorkItemSessionLink | null {
    const existing = this.findSessionLinkById(id);
    if (!existing) return null;
    if (existing.acceptanceStatus === 'accepted') return existing;

    const db = getDatabase();
    const now = new Date().toISOString();
    db.prepare(`
      UPDATE work_item_session_links
      SET link_source = 'accepted_agent_suggestion',
          acceptance_status = 'accepted',
          accepted_at = ?,
          updated_at = ?
      WHERE id = ?
    `).run(now, now, id);

    return this.findSessionLinkById(id);
  }

  findSessionLinkById(id: string): WorkItemSessionLink | null {
    const db = getDatabase();
    const row = db.prepare('SELECT * FROM work_item_session_links WHERE id = ?')
      .get(id) as WorkItemSessionLinkRow | undefined;
    return row ? rowToWorkItemSessionLink(row) : null;
  }

  private findSessionLinkForPair(
    workItemId: string,
    agentSessionId: string
  ): WorkItemSessionLink | null {
    const db = getDatabase();
    const row = db.prepare(`
      SELECT * FROM work_item_session_links
      WHERE work_item_id = ? AND agent_session_id = ?
    `).get(workItemId, agentSessionId) as WorkItemSessionLinkRow | undefined;
    return row ? rowToWorkItemSessionLink(row) : null;
  }

  createProgressEvidence(input: ProgressEvidenceCreateInput): ProgressEvidence {
    if (!input.workItemId && !input.agentSessionId) {
      throw new Error('ProgressEvidence requires workItemId or agentSessionId');
    }

    const db = getDatabase();
    const id = randomUUID();
    const now = new Date().toISOString();
    const occurredAt = (input.occurredAt ?? new Date()).toISOString();

    db.prepare(`
      INSERT INTO progress_evidence (
        id, work_item_id, agent_session_id, runtime_id, kind, outcome, summary,
        source_path, occurred_at, confidence, metadata, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).run(
      id,
      input.workItemId ?? null,
      input.agentSessionId ?? null,
      input.runtimeId ?? null,
      input.kind,
      input.outcome ?? null,
      input.summary,
      input.sourcePath ?? null,
      occurredAt,
      input.confidence ?? 'explicit',
      input.metadata ? JSON.stringify(input.metadata) : null,
      now
    );

    return this.findEvidenceById(id)!;
  }

  findEvidenceById(id: string): ProgressEvidence | null {
    const db = getDatabase();
    const row = db.prepare('SELECT * FROM progress_evidence WHERE id = ?')
      .get(id) as ProgressEvidenceRow | undefined;
    return row ? rowToProgressEvidence(row) : null;
  }
}

export const workItemEvidenceRepository = new WorkItemEvidenceRepository();
