/**
 * Core type definitions for Tasker
 *
 * Re-exports from domain layer for backward compatibility.
 * New code should import directly from '../domain/index.js'
 */

// Re-export from domain layer
export type {
  SessionStatus,
  ToolCallInfo,
  ProcessInfo,
  ClaudeSessionFile,
} from '../domain/session/value-objects.js';

export type {
  Session,
  SessionListItem,
  ParsedSessionData,
  AggregatedSession,
  AggregatedSessionListItem,
  CreateSessionInput,
  UpdateSessionInput,
  SessionStats,
} from '../domain/session/entity.js';

export type {
  Entity,
  Result,
  DomainEvent,
} from '../domain/shared/types.js';

/** Tool usage record */
export interface ToolUsage {
  timestamp: Date;
  tool: string;
  input: Record<string, unknown>;
  output?: string;
  duration?: number;
}

/** Event payload types (for backward compatibility with old event system) */
export interface SessionEventPayload {
  session: import('../domain/session/entity.js').Session;
  previousStatus?: import('../domain/session/value-objects.js').SessionStatus;
}

export interface ToolEventPayload {
  sessionId: string;
  tool: string;
  input: Record<string, unknown>;
  output?: string;
  timestamp: Date;
}

export interface SessionEndEventPayload {
  sessionId: string;
  timestamp: Date;
  reason?: string;
}
