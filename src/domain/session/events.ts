/**
 * Session domain events
 */

import type { DomainEvent } from '../shared/types.js';
import type { Session } from './entity.js';
import type { SessionStatus } from './value-objects.js';

/** Base session event */
export interface SessionEvent extends DomainEvent {
  session: Session;
}

/** Session discovered - new session found */
export interface SessionDiscovered extends SessionEvent {
  type: 'session.discovered';
}

/** Session updated - status or data changed */
export interface SessionUpdated extends SessionEvent {
  type: 'session.updated';
  previousStatus?: SessionStatus;
}

/** Session lost - process terminated unexpectedly */
export interface SessionLost extends SessionEvent {
  type: 'session.lost';
  previousStatus: SessionStatus;
  reason: 'process_terminated' | 'timeout' | 'unknown';
}

/** Session recovered - session was restored */
export interface SessionRecovered extends SessionEvent {
  type: 'session.recovered';
  method: 'resume' | 'continue' | 'new';
  newPid?: number;
}

/** Session completed - session finished normally */
export interface SessionCompleted extends SessionEvent {
  type: 'session.completed';
}

/** Tool usage event from hooks */
export interface ToolUsageEvent extends DomainEvent {
  type: 'tool.pre' | 'tool.post';
  sessionId: string;
  tool: string;
  input: Record<string, unknown>;
}

/** Scan complete event */
export interface ScanComplete extends DomainEvent {
  type: 'scan.complete';
  sessionCount: number;
  duration: number;
  discovered: number;
  updated: number;
  lost: number;
}

/** All session-related events */
export type SessionDomainEvent =
  | SessionDiscovered
  | SessionUpdated
  | SessionLost
  | SessionRecovered
  | SessionCompleted
  | ToolUsageEvent
  | ScanComplete;

/** Event factory functions */
export function createSessionDiscoveredEvent(session: Session): SessionDiscovered {
  return {
    type: 'session.discovered',
    timestamp: new Date(),
    aggregateId: session.sessionId,
    session,
  };
}

export function createSessionUpdatedEvent(
  session: Session,
  previousStatus?: SessionStatus
): SessionUpdated {
  return {
    type: 'session.updated',
    timestamp: new Date(),
    aggregateId: session.sessionId,
    session,
    previousStatus,
  };
}

export function createSessionLostEvent(
  session: Session,
  previousStatus: SessionStatus,
  reason: 'process_terminated' | 'timeout' | 'unknown' = 'process_terminated'
): SessionLost {
  return {
    type: 'session.lost',
    timestamp: new Date(),
    aggregateId: session.sessionId,
    session,
    previousStatus,
    reason,
  };
}

export function createSessionRecoveredEvent(
  session: Session,
  method: 'resume' | 'continue' | 'new',
  newPid?: number
): SessionRecovered {
  return {
    type: 'session.recovered',
    timestamp: new Date(),
    aggregateId: session.sessionId,
    session,
    method,
    newPid,
  };
}

export function createSessionCompletedEvent(session: Session): SessionCompleted {
  return {
    type: 'session.completed',
    timestamp: new Date(),
    aggregateId: session.sessionId,
    session,
  };
}

export function createScanCompleteEvent(
  sessionCount: number,
  duration: number,
  discovered: number,
  updated: number,
  lost: number
): ScanComplete {
  return {
    type: 'scan.complete',
    timestamp: new Date(),
    sessionCount,
    duration,
    discovered,
    updated,
    lost,
  };
}
