/**
 * Session repository interface
 *
 * Defines the contract for session persistence.
 * Implementation is in infrastructure layer.
 */

import type { Session } from './entity.js';
import type { SessionStatus } from './value-objects.js';

export interface ActiveSessionRecord {
  sessionId: string;
  status: SessionStatus;
  pid?: number;
}

/** Session upsert data (for create or update) */
export interface SessionUpsertData {
  sessionId: string;
  directory?: string;
  status?: SessionStatus;
  title?: string;
  initialPrompt?: string;
  lastTool?: string;
  lastToolInput?: string;
  currentFile?: string;
  lastMessage?: string;
  startedAt?: Date;
  lastActiveAt?: Date;
  completedAt?: Date;
  pid?: number;
  tty?: string;
  toolCount?: number;
  messageCount?: number;
}

/** Session repository interface */
export interface ISessionRepository {
  /** Find session by internal ID */
  findById(id: string): Session | null;

  /** Find session by Claude session ID */
  findBySessionId(sessionId: string): Session | null;

  /** Find multiple sessions by Claude session ID */
  findBySessionIds(sessionIds: string[]): Session[];

  /** Find all sessions */
  findAll(): Session[];

  /** Find active sessions (not completed or lost) */
  findActive(): Session[];

  /** Find active sessions with only the fields needed for process reconciliation */
  findActiveLightweight(): ActiveSessionRecord[];

  /** Find sessions by status */
  findByStatus(status: SessionStatus): Session[];

  /** Find sessions by directory */
  findByDirectory(directory: string): Session[];

  /** Create or update a session */
  upsert(data: SessionUpsertData): Session;

  /** Delete old sessions based on retention policy */
  deleteOldSessions(retentionDays: number): number;

  /** Get session count by status */
  countByStatus(): Record<SessionStatus, number>;
}
