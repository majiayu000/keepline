/**
 * Session repository implementation
 */

import { randomUUID } from 'crypto';
import { getDatabase, transaction } from '../sqlite.js';
import type {
  ActiveSessionRecord,
  Session,
  SessionListItem,
  SessionStatus,
} from '../../../domain/session/index.js';
import type { ISessionRepository, SessionUpsertData } from '../../../domain/session/repository.js';

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
  last_message: string | null;
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

interface SessionListRow {
  id: string;
  session_id: string;
  directory: string;
  status: string;
  title: string | null;
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

interface ActiveSessionRow {
  session_id: string;
  status: string;
  pid: number | null;
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
    lastMessage: row.last_message || undefined,
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

function rowToSessionListItem(row: SessionListRow): SessionListItem {
  return {
    id: row.id,
    sessionId: row.session_id,
    directory: row.directory,
    status: row.status as SessionStatus,
    title: row.title || '',
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

function rowToActiveSessionRecord(row: ActiveSessionRow): ActiveSessionRecord {
  return {
    sessionId: row.session_id,
    status: row.status as SessionStatus,
    pid: row.pid || undefined,
  };
}

/** Session repository implementation */
class SessionRepository implements ISessionRepository {
  findById(id: string): Session | null {
    const db = getDatabase();
    const row = db
      .prepare('SELECT * FROM sessions WHERE id = ?')
      .get(id) as SessionRow | undefined;

    return row ? rowToSession(row) : null;
  }

  findBySessionId(sessionId: string): Session | null {
    const db = getDatabase();
    const row = db
      .prepare('SELECT * FROM sessions WHERE session_id = ?')
      .get(sessionId) as SessionRow | undefined;

    return row ? rowToSession(row) : null;
  }

  findBySessionIds(sessionIds: string[]): Session[] {
    if (sessionIds.length === 0) {
      return [];
    }

    const db = getDatabase();
    const placeholders = sessionIds.map(() => '?').join(', ');
    const rows = db
      .prepare(`SELECT * FROM sessions WHERE session_id IN (${placeholders})`)
      .all(...sessionIds) as SessionRow[];

    return rows.map(rowToSession);
  }

  findAll(): Session[] {
    const db = getDatabase();
    const rows = db
      .prepare('SELECT * FROM sessions ORDER BY last_active_at DESC')
      .all() as SessionRow[];

    return rows.map(rowToSession);
  }

  findAllLightweight(): SessionListItem[] {
    const db = getDatabase();
    const rows = db
      .prepare(`
        SELECT
          id,
          session_id,
          directory,
          status,
          title,
          started_at,
          last_active_at,
          completed_at,
          pid,
          tty,
          tool_count,
          message_count,
          created_at,
          updated_at
        FROM sessions
        ORDER BY last_active_at DESC
      `)
      .all() as SessionListRow[];

    return rows.map(rowToSessionListItem);
  }

  findActive(): Session[] {
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

  findActiveLightweight(): ActiveSessionRecord[] {
    const db = getDatabase();
    const rows = db
      .prepare(`
        SELECT session_id, status, pid FROM sessions
        WHERE status NOT IN ('completed', 'lost')
      `)
      .all() as ActiveSessionRow[];

    return rows.map(rowToActiveSessionRecord);
  }

  findByStatus(status: SessionStatus): Session[] {
    const db = getDatabase();
    const rows = db
      .prepare('SELECT * FROM sessions WHERE status = ? ORDER BY last_active_at DESC')
      .all(status) as SessionRow[];

    return rows.map(rowToSession);
  }

  findByDirectory(directory: string): Session[] {
    const db = getDatabase();
    const rows = db
      .prepare('SELECT * FROM sessions WHERE directory = ? ORDER BY last_active_at DESC')
      .all(directory) as SessionRow[];

    return rows.map(rowToSession);
  }

  upsert(data: SessionUpsertData): Session {
    const db = getDatabase();
    const now = new Date().toISOString();

    return transaction(() => {
      const hasPid = 'pid' in data;
      const hasTty = 'tty' in data;
      const updateResult = db.prepare(`
        UPDATE sessions SET
          status = COALESCE(?, status),
          title = COALESCE(?, title),
          initial_prompt = COALESCE(?, initial_prompt),
          last_tool = COALESCE(?, last_tool),
          last_tool_input = COALESCE(?, last_tool_input),
          current_file = COALESCE(?, current_file),
          last_message = COALESCE(?, last_message),
          started_at = COALESCE(?, started_at),
          last_active_at = COALESCE(?, last_active_at),
          completed_at = COALESCE(?, completed_at),
          pid = CASE WHEN ? THEN ? ELSE pid END,
          tty = CASE WHEN ? THEN ? ELSE tty END,
          tool_count = COALESCE(?, tool_count),
          message_count = COALESCE(?, message_count),
          updated_at = ?
        WHERE session_id = ?
      `).run(
        data.status ?? null,
        data.title ?? null,
        data.initialPrompt ?? null,
        data.lastTool ?? null,
        data.lastToolInput ?? null,
        data.currentFile ?? null,
        data.lastMessage ?? null,
        data.startedAt?.toISOString() ?? null,
        data.lastActiveAt?.toISOString() ?? null,
        data.completedAt?.toISOString() ?? null,
        hasPid ? 1 : 0,
        data.pid ?? null,
        hasTty ? 1 : 0,
        data.tty ?? null,
        data.toolCount ?? null,
        data.messageCount ?? null,
        now,
        data.sessionId
      );

      if (updateResult.changes > 0) {
        return this.findBySessionId(data.sessionId)!;
      }

      {
        // Insert new session
        const id = randomUUID();
        db.prepare(`
          INSERT INTO sessions (
            id, session_id, directory, status, title, initial_prompt,
            last_tool, last_tool_input, current_file, last_message,
            started_at, last_active_at, pid, tty,
            tool_count, message_count, created_at, updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `).run(
          id,
          data.sessionId,
          data.directory || '',
          data.status || 'idle',
          data.title || '',
          data.initialPrompt || '',
          data.lastTool ?? null,
          data.lastToolInput ?? null,
          data.currentFile ?? null,
          data.lastMessage ?? null,
          data.startedAt?.toISOString() ?? null,
          data.lastActiveAt?.toISOString() || now,
          data.pid ?? null,
          data.tty ?? null,
          data.toolCount || 0,
          data.messageCount || 0,
          now,
          now
        );

        return this.findBySessionId(data.sessionId)!;
      }
    });
  }

  deleteOldSessions(retentionDays: number): number {
    const db = getDatabase();
    const cutoff = new Date();
    cutoff.setDate(cutoff.getDate() - retentionDays);

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

  countByStatus(): Record<SessionStatus, number> {
    const db = getDatabase();
    const rows = db.prepare(`
      SELECT status, COUNT(*) as count FROM sessions GROUP BY status
    `).all() as Array<{ status: string; count: number }>;

    const result: Record<SessionStatus, number> = {
      running: 0,
      waiting: 0,
      idle: 0,
      lost: 0,
      completed: 0,
    };

    for (const row of rows) {
      result[row.status as SessionStatus] = row.count;
    }

    return result;
  }
}

/** Singleton instance */
export const sessionRepository = new SessionRepository();
