/**
 * Session entity definition
 */

import type { Entity } from '../shared/types.js';
import type { SessionStatus, ToolCallInfo } from './value-objects.js';

/** Session entity - represents a Claude Code session */
export interface Session extends Entity {
  /** Claude's internal session ID */
  sessionId: string;

  /** Working directory path */
  directory: string;

  /** Current status */
  status: SessionStatus;

  /** Task information */
  title: string;
  initialPrompt: string;

  /** Activity tracking */
  lastTool?: string;
  lastToolInput?: string;
  currentFile?: string;
  lastMessage?: string;
  toolCalls?: ToolCallInfo[];

  /** Timeline */
  startedAt?: Date;
  lastActiveAt: Date;
  completedAt?: Date;

  /** Process info */
  pid?: number;
  tty?: string;

  /** Metrics */
  toolCount: number;
  messageCount: number;
}

/** Parsed session data from JSONL files */
export interface ParsedSessionData {
  sessionId: string;
  directory: string;
  firstMessage?: string;
  lastMessage?: string;
  messageCount: number;
  toolCount: number;
  lastTool?: string;
  lastToolInput?: Record<string, unknown>;
  currentFile?: string;
  startedAt?: Date;
  lastActiveAt: Date;
  toolCalls?: ToolCallInfo[];

  /** Multi-session tracking */
  agentId?: string;
  parentSessionId?: string;
  isSubAgent?: boolean;
}

/** Session with live process data */
export interface AggregatedSession extends Session {
  processRunning: boolean;
  cpuUsage?: number;
  memoryUsage?: number;
}

/** Lightweight session row for list/realtime payloads */
export type SessionListItem = Pick<
  Session,
  | 'id'
  | 'sessionId'
  | 'directory'
  | 'status'
  | 'title'
  | 'startedAt'
  | 'lastActiveAt'
  | 'completedAt'
  | 'pid'
  | 'tty'
  | 'toolCount'
  | 'messageCount'
  | 'createdAt'
  | 'updatedAt'
>;

/** Lightweight session with live process data */
export interface AggregatedSessionListItem extends SessionListItem {
  processRunning: boolean;
  cpuUsage?: number;
  memoryUsage?: number;
}

/** Input for creating a new session */
export interface CreateSessionInput {
  sessionId: string;
  directory: string;
  initialPrompt: string;
  title?: string;
  pid?: number;
  tty?: string;
}

/** Input for updating an existing session */
export interface UpdateSessionInput {
  directory?: string;
  status?: SessionStatus;
  title?: string;
  initialPrompt?: string;
  lastTool?: string;
  lastToolInput?: string;
  currentFile?: string;
  lastMessage?: string;
  lastActiveAt?: Date;
  completedAt?: Date;
  pid?: number;
  tty?: string;
  toolCount?: number;
  messageCount?: number;
}

/** Session statistics */
export interface SessionStats {
  running: number;
  waiting: number;
  idle: number;
  lost: number;
  completed: number;
  total: number;
}

/** Generate title from prompt */
export function generateTitle(prompt: string): string {
  if (prompt.length <= 80) return prompt;

  const truncated = prompt.slice(0, 80);
  const lastSpace = truncated.lastIndexOf(' ');
  if (lastSpace > 40) {
    return truncated.slice(0, lastSpace) + '...';
  }
  return truncated + '...';
}
