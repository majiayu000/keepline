/**
 * Memory repository implementation
 *
 * Persists session memory for the "relay race" pattern.
 */

import { randomUUID } from 'crypto';
import { getDatabase, transaction } from '../sqlite.js';
import type {
  SessionMemory,
  MemoryUpsertData,
  MemorySummary,
} from '../../../domain/memory/index.js';

/** Database row type */
interface MemoryRow {
  id: string;
  session_id: string;
  directory: string;
  last_progress: string;
  pending_tasks: string;
  completed_tasks: string;
  known_issues: string;
  decisions: string;
  notes: string;
  handoff_notes: string;
  handoff_priority: string;
  iteration_count: number;
  total_tokens_used: number;
  created_at: string;
  updated_at: string;
}

/** Convert database row to SessionMemory entity */
function rowToMemory(row: MemoryRow): SessionMemory {
  return {
    id: row.id,
    sessionId: row.session_id,
    directory: row.directory,
    lastProgress: row.last_progress,
    pendingTasks: JSON.parse(row.pending_tasks || '[]'),
    completedTasks: JSON.parse(row.completed_tasks || '[]'),
    knownIssues: JSON.parse(row.known_issues || '[]'),
    decisions: JSON.parse(row.decisions || '[]'),
    notes: row.notes,
    handoffNotes: row.handoff_notes,
    handoffPriority: JSON.parse(row.handoff_priority || '[]'),
    iterationCount: row.iteration_count,
    totalTokensUsed: row.total_tokens_used,
    createdAt: new Date(row.created_at),
    updatedAt: new Date(row.updated_at),
  };
}

/** Memory repository interface */
export interface IMemoryRepository {
  findById(id: string): SessionMemory | null;
  findBySessionId(sessionId: string): SessionMemory | null;
  findByDirectory(directory: string): SessionMemory[];
  findAll(): SessionMemory[];
  findRecent(limit?: number): SessionMemory[];
  upsert(data: MemoryUpsertData): SessionMemory;
  delete(sessionId: string): boolean;
  getSummaries(): MemorySummary[];
}

/** Memory repository implementation */
class MemoryRepository implements IMemoryRepository {
  findById(id: string): SessionMemory | null {
    const db = getDatabase();
    const row = db
      .prepare('SELECT * FROM session_memories WHERE id = ?')
      .get(id) as MemoryRow | undefined;

    return row ? rowToMemory(row) : null;
  }

  findBySessionId(sessionId: string): SessionMemory | null {
    const db = getDatabase();
    const row = db
      .prepare('SELECT * FROM session_memories WHERE session_id = ?')
      .get(sessionId) as MemoryRow | undefined;

    return row ? rowToMemory(row) : null;
  }

  findByDirectory(directory: string): SessionMemory[] {
    const db = getDatabase();
    const rows = db
      .prepare('SELECT * FROM session_memories WHERE directory = ? ORDER BY updated_at DESC')
      .all(directory) as MemoryRow[];

    return rows.map(rowToMemory);
  }

  findAll(): SessionMemory[] {
    const db = getDatabase();
    const rows = db
      .prepare('SELECT * FROM session_memories ORDER BY updated_at DESC')
      .all() as MemoryRow[];

    return rows.map(rowToMemory);
  }

  findRecent(limit: number = 10): SessionMemory[] {
    const db = getDatabase();
    const rows = db
      .prepare('SELECT * FROM session_memories ORDER BY updated_at DESC LIMIT ?')
      .all(limit) as MemoryRow[];

    return rows.map(rowToMemory);
  }

  upsert(data: MemoryUpsertData): SessionMemory {
    const db = getDatabase();
    const now = new Date().toISOString();

    return transaction(() => {
      const existing = this.findBySessionId(data.sessionId);

      if (existing) {
        // Update existing memory
        db.prepare(`
          UPDATE session_memories SET
            directory = COALESCE(?, directory),
            last_progress = COALESCE(?, last_progress),
            pending_tasks = COALESCE(?, pending_tasks),
            completed_tasks = COALESCE(?, completed_tasks),
            known_issues = COALESCE(?, known_issues),
            decisions = COALESCE(?, decisions),
            notes = COALESCE(?, notes),
            handoff_notes = COALESCE(?, handoff_notes),
            handoff_priority = COALESCE(?, handoff_priority),
            iteration_count = COALESCE(?, iteration_count),
            total_tokens_used = COALESCE(?, total_tokens_used),
            updated_at = ?
          WHERE session_id = ?
        `).run(
          data.directory ?? null,
          data.lastProgress ?? null,
          data.pendingTasks ? JSON.stringify(data.pendingTasks) : null,
          data.completedTasks ? JSON.stringify(data.completedTasks) : null,
          data.knownIssues ? JSON.stringify(data.knownIssues) : null,
          data.decisions ? JSON.stringify(data.decisions) : null,
          data.notes ?? null,
          data.handoffNotes ?? null,
          data.handoffPriority ? JSON.stringify(data.handoffPriority) : null,
          data.iterationCount ?? null,
          data.totalTokensUsed ?? null,
          now,
          data.sessionId
        );

        return this.findBySessionId(data.sessionId)!;
      } else {
        // Insert new memory
        const id = randomUUID();
        db.prepare(`
          INSERT INTO session_memories (
            id, session_id, directory, last_progress,
            pending_tasks, completed_tasks, known_issues, decisions,
            notes, handoff_notes, handoff_priority,
            iteration_count, total_tokens_used,
            created_at, updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        `).run(
          id,
          data.sessionId,
          data.directory || '',
          data.lastProgress || '',
          JSON.stringify(data.pendingTasks || []),
          JSON.stringify(data.completedTasks || []),
          JSON.stringify(data.knownIssues || []),
          JSON.stringify(data.decisions || []),
          data.notes || '',
          data.handoffNotes || '',
          JSON.stringify(data.handoffPriority || []),
          data.iterationCount || 0,
          data.totalTokensUsed || 0,
          now,
          now
        );

        return this.findBySessionId(data.sessionId)!;
      }
    });
  }

  delete(sessionId: string): boolean {
    const db = getDatabase();
    const result = db
      .prepare('DELETE FROM session_memories WHERE session_id = ?')
      .run(sessionId);

    return result.changes > 0;
  }

  getSummaries(): MemorySummary[] {
    const db = getDatabase();
    const rows = db.prepare(`
      SELECT
        session_id,
        directory,
        last_progress,
        pending_tasks,
        completed_tasks,
        iteration_count,
        updated_at
      FROM session_memories
      ORDER BY updated_at DESC
    `).all() as Array<{
      session_id: string;
      directory: string;
      last_progress: string;
      pending_tasks: string;
      completed_tasks: string;
      iteration_count: number;
      updated_at: string;
    }>;

    return rows.map(row => ({
      sessionId: row.session_id,
      directory: row.directory,
      lastProgress: row.last_progress,
      pendingTaskCount: JSON.parse(row.pending_tasks || '[]').length,
      completedTaskCount: JSON.parse(row.completed_tasks || '[]').length,
      iterationCount: row.iteration_count,
      updatedAt: new Date(row.updated_at),
    }));
  }

  /**
   * Increment iteration count for a session
   */
  incrementIteration(sessionId: string, tokensUsed: number = 0): void {
    const db = getDatabase();
    db.prepare(`
      UPDATE session_memories SET
        iteration_count = iteration_count + 1,
        total_tokens_used = total_tokens_used + ?,
        updated_at = ?
      WHERE session_id = ?
    `).run(tokensUsed, new Date().toISOString(), sessionId);
  }

  /**
   * Clear handoff data after recovery
   */
  clearHandoff(sessionId: string): void {
    const db = getDatabase();
    const now = new Date().toISOString();
    db.prepare(`
      UPDATE session_memories SET
        handoff_notes = '',
        handoff_priority = '[]',
        updated_at = ?
      WHERE session_id = ?
    `).run(now, sessionId);
  }
}

/** Singleton instance */
export const memoryRepository = new MemoryRepository();
