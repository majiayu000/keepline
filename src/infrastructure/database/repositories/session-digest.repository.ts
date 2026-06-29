/**
 * Repository for persisted orchestrator session digests.
 */

import { randomUUID } from 'crypto';
import { getDatabase, transaction } from '../sqlite.js';
import type {
  SessionDigest,
  SessionDigestErrorInput,
  SessionDigestSource,
  SessionDigestStatus,
  SessionDigestUpsertInput,
} from '../../../domain/orchestrator/index.js';

interface SessionDigestRow {
  id: string;
  session_id: string;
  summary: string;
  next_actions: string;
  blockers: string;
  waiting_for_human: number;
  source: string;
  status: string;
  source_updated_at: string;
  generated_at: string;
  provider: string | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

function parseStringArray(raw: string, field: string): string[] {
  const parsed = JSON.parse(raw || '[]');
  if (!Array.isArray(parsed) || parsed.some((item) => typeof item !== 'string')) {
    throw new Error(`Invalid ${field} payload`);
  }
  return parsed;
}

function rowToSessionDigest(row: SessionDigestRow): SessionDigest {
  return {
    id: row.id,
    sessionId: row.session_id,
    summary: row.summary,
    nextActions: parseStringArray(row.next_actions, 'next_actions'),
    blockers: parseStringArray(row.blockers, 'blockers'),
    waitingForHuman: row.waiting_for_human === 1,
    source: row.source as SessionDigestSource,
    status: row.status as SessionDigestStatus,
    sourceUpdatedAt: new Date(row.source_updated_at),
    generatedAt: new Date(row.generated_at),
    provider: row.provider || undefined,
    errorMessage: row.error_message || undefined,
    createdAt: new Date(row.created_at),
    updatedAt: new Date(row.updated_at),
  };
}

export interface ISessionDigestRepository {
  findBySessionId(sessionId: string): SessionDigest | null;
  findBySessionIds(sessionIds: string[]): SessionDigest[];
  findAll(): SessionDigest[];
  upsert(input: SessionDigestUpsertInput): SessionDigest;
  markError(input: SessionDigestErrorInput): SessionDigest;
}

class SessionDigestRepository implements ISessionDigestRepository {
  findBySessionId(sessionId: string): SessionDigest | null {
    const db = getDatabase();
    const row = db.prepare('SELECT * FROM session_digests WHERE session_id = ?')
      .get(sessionId) as SessionDigestRow | undefined;
    return row ? rowToSessionDigest(row) : null;
  }

  findBySessionIds(sessionIds: string[]): SessionDigest[] {
    const uniqueIds = [...new Set(sessionIds)].filter(Boolean);
    if (uniqueIds.length === 0) return [];

    const db = getDatabase();
    const placeholders = uniqueIds.map(() => '?').join(', ');
    const rows = db.prepare(`
      SELECT * FROM session_digests
      WHERE session_id IN (${placeholders})
    `).all(...uniqueIds) as SessionDigestRow[];

    return rows.map(rowToSessionDigest);
  }

  findAll(): SessionDigest[] {
    const db = getDatabase();
    const rows = db.prepare('SELECT * FROM session_digests ORDER BY generated_at DESC')
      .all() as SessionDigestRow[];
    return rows.map(rowToSessionDigest);
  }

  upsert(input: SessionDigestUpsertInput): SessionDigest {
    const db = getDatabase();
    const now = new Date().toISOString();
    const generatedAt = (input.generatedAt ?? new Date()).toISOString();

    db.prepare(`
      INSERT INTO session_digests (
        id, session_id, summary, next_actions, blockers, waiting_for_human,
        source, status, source_updated_at, generated_at, provider, error_message,
        created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(session_id) DO UPDATE SET
        summary = excluded.summary,
        next_actions = excluded.next_actions,
        blockers = excluded.blockers,
        waiting_for_human = excluded.waiting_for_human,
        source = excluded.source,
        status = excluded.status,
        source_updated_at = excluded.source_updated_at,
        generated_at = excluded.generated_at,
        provider = excluded.provider,
        error_message = excluded.error_message,
        updated_at = excluded.updated_at
    `).run(
      randomUUID(),
      input.sessionId,
      input.summary,
      JSON.stringify(input.nextActions ?? []),
      JSON.stringify(input.blockers ?? []),
      input.waitingForHuman ? 1 : 0,
      input.source,
      input.status ?? 'fresh',
      input.sourceUpdatedAt.toISOString(),
      generatedAt,
      input.provider ?? null,
      input.errorMessage ?? null,
      now,
      now
    );

    return this.findBySessionId(input.sessionId)!;
  }

  markError(input: SessionDigestErrorInput): SessionDigest {
    const now = new Date().toISOString();

    return transaction(() => {
      const existing = this.findBySessionId(input.sessionId);
      if (existing) {
        getDatabase().prepare(`
          UPDATE session_digests
          SET status = 'error',
              provider = ?,
              error_message = ?,
              generated_at = ?,
              updated_at = ?
          WHERE session_id = ?
        `).run(
          input.provider ?? null,
          input.errorMessage,
          now,
          now,
          input.sessionId
        );

        return this.findBySessionId(input.sessionId)!;
      }

      return this.upsert({
        sessionId: input.sessionId,
        summary: '',
        nextActions: [],
        blockers: [],
        waitingForHuman: false,
        source: input.source,
        status: 'error',
        sourceUpdatedAt: input.sourceUpdatedAt,
        generatedAt: new Date(now),
        provider: input.provider,
        errorMessage: input.errorMessage,
      });
    });
  }
}

export const sessionDigestRepository = new SessionDigestRepository();
