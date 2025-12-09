/**
 * Session repository - database operations for sessions
 */

import { randomUUID } from 'crypto';
import { getDatabase, transaction } from './database.js';
import type { Session, SessionStatus } from '../core/types.js';
import { SessionNotFoundError } from '../core/errors.js';

/** Database row type */
interface SessionRow {
  id: string;
  session_id: string;
  directory: string;
  status: string;
  title: string | null;
  initial_prompt: string;
  last_tool: string | null;
  last_tool_input: string | null;
  current_file: string | null;
  started_at: string | null;
  last_active_at: string;
  completed_at: string | null;
  pid: number | null;
  tty: string | null;
  tool_count: number;
  message_count: number;
  created_at: string;
  updated_at: string;
}

/** Convert database row to Session entity */
function rowToSession(row: SessionRow): Session {
  return {
    id: row.id,
    sessionId: row.session_id,
    directory: row.directory,
    status: row.status as SessionStatus,
    title: row.title || '',
    initialPrompt: row.initial_prompt,
    lastTool: row.last_tool || undefined,
    lastToolInput: row.last_tool_input || undefined,
    currentFile: row.current_file || undefined,
    startedAt: row.started_at ? new Date(row.started_at) : undefined,
    lastActiveAt: new Date(row.last_active_at),
    completedAt: row.completed_at ? new Date(row.completed_at) : undefined,
    pid: row.pid || undefined,
    tty: row.tty || undefined,
    toolCount: row.tool_count,
    messageCount: row.message_count,
    createdAt: new Date(row.created_at),
    updatedAt: new Date(row.updated_at),
  };
}

/** Find session by session ID */
export function findBySessionId(sessionId: string): Session | null {
  const db = getDatabase();
  const row = db
    .prepare('SELECT * FROM sessions WHERE session_id = ?')
    .get(sessionId) as SessionRow | undefined;

  return row ? rowToSession(row) : null;
}

/** Find all sessions */
export function findAll(): Session[] {
  const db = getDatabase();
  const rows = db
    .prepare('SELECT * FROM sessions ORDER BY last_active_at DESC')
    .all() as SessionRow[];

  return rows.map(rowToSession);
}

/** Find sessions by status */
export function findByStatus(status: SessionStatus): Session[] {
  const db = getDatabase();
  const rows = db
    .prepare('SELECT * FROM sessions WHERE status = ? ORDER BY last_active_at DESC')
    .all(status) as SessionRow[];

  return rows.map(rowToSession);
}

/** Find sessions by directory */
export function findByDirectory(directory: string): Session[] {
  const db = getDatabase();
  const rows = db
    .prepare('SELECT * FROM sessions WHERE directory = ? ORDER BY last_active_at DESC')
    .all(directory) as SessionRow[];

  return rows.map(rowToSession);
}

/** Find active sessions (not completed or lost) */
export function findActive(): Session[] {
  const db = getDatabase();
  const rows = db
    .prepare(`
      SELECT * FROM sessions
      WHERE status NOT IN ('completed', 'lost')
      ORDER BY last_active_at DESC
    `)
    .all() as SessionRow[];

  return rows.map(rowToSession);
}

/** Create or update session */
export function upsert(session: Partial<Session> & { sessionId: string }): Session {
  const db = getDatabase();
  const now = new Date().toISOString();

  return transaction(() => {
    const existing = findBySessionId(session.sessionId);

    if (existing) {
      // Update existing session
      db.prepare(`
        UPDATE sessions SET
          status = COALESCE(?, status),
          title = COALESCE(?, title),
          last_tool = COALESCE(?, last_tool),
          last_tool_input = COALESCE(?, last_tool_input),
          current_file = COALESCE(?, current_file),
          last_active_at = COALESCE(?, last_active_at),
          completed_at = COALESCE(?, completed_at),
          pid = ?,
          tty = ?,
          tool_count = COALESCE(?, tool_count),
          message_count = COALESCE(?, message_count),
          updated_at = ?
        WHERE session_id = ?
      `).run(
        session.status ?? null,
        session.title ?? null,
        session.lastTool ?? null,
        session.lastToolInput ?? null,
        session.currentFile ?? null,
        session.lastActiveAt?.toISOString() ?? null,
        session.completedAt?.toISOString() ?? null,
        session.pid ?? null,
        session.tty ?? null,
        session.toolCount ?? null,
        session.messageCount ?? null,
        now,
        session.sessionId
      );

      return findBySessionId(session.sessionId)!;
    } else {
      // Insert new session
      const id = randomUUID();
      db.prepare(`
        INSERT INTO sessions (
          id, session_id, directory, status, title, initial_prompt,
          last_tool, last_tool_input, current_file,
          started_at, last_active_at, pid, tty,
          tool_count, message_count, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        id,
        session.sessionId,
        session.directory || '',
        session.status || 'idle',
        session.title || '',
        session.initialPrompt || '',
        session.lastTool ?? null,
        session.lastToolInput ?? null,
        session.currentFile ?? null,
        session.startedAt?.toISOString() ?? null,
        session.lastActiveAt?.toISOString() || now,
        session.pid ?? null,
        session.tty ?? null,
        session.toolCount || 0,
        session.messageCount || 0,
        now,
        now
      );

      return findBySessionId(session.sessionId)!;
    }
  });
}

/** Update session status */
export function updateStatus(sessionId: string, status: SessionStatus): void {
  const db = getDatabase();
  const now = new Date().toISOString();

  db.prepare(`
    UPDATE sessions SET status = ?, updated_at = ? WHERE session_id = ?
  `).run(status, now, sessionId);

  // Check if update affected any rows
  const exists = findBySessionId(sessionId);
  if (!exists) {
    throw new SessionNotFoundError(sessionId);
  }
}

/** Delete session */
export function deleteSession(sessionId: string): void {
  const db = getDatabase();
  db.prepare('DELETE FROM sessions WHERE session_id = ?').run(sessionId);
}

/** Delete old sessions */
export function deleteOldSessions(daysOld: number): number {
  const db = getDatabase();
  const cutoff = new Date();
  cutoff.setDate(cutoff.getDate() - daysOld);

  // Get count before delete
  const beforeCount = (db.prepare(`
    SELECT COUNT(*) as count FROM sessions
    WHERE status = 'completed' AND last_active_at < ?
  `).get(cutoff.toISOString()) as { count: number }).count;

  db.prepare(`
    DELETE FROM sessions
    WHERE status = 'completed' AND last_active_at < ?
  `).run(cutoff.toISOString());

  return beforeCount;
}
