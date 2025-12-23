/**
 * Recovery domain events
 */

import type { DomainEvent } from '../shared/types.js';
import type { RecoveryMethod } from './types.js';
import type { RecoveryAssessment } from './entity.js';

/** Recovery started event */
export interface RecoveryStarted extends DomainEvent {
  type: 'recovery.started';
  sessionId: string;
  method: RecoveryMethod;
  policyId?: string;
  attemptNumber: number;
}

/** Recovery completed event */
export interface RecoveryCompleted extends DomainEvent {
  type: 'recovery.completed';
  sessionId: string;
  method: RecoveryMethod;
  success: boolean;
  newPid?: number;
  error?: string;
  duration: number;
}

/** Recovery assessment completed */
export interface RecoveryAssessed extends DomainEvent {
  type: 'recovery.assessed';
  sessionId: string;
  assessment: RecoveryAssessment;
}

/** Recovery policy triggered */
export interface RecoveryPolicyTriggered extends DomainEvent {
  type: 'recovery.policy.triggered';
  sessionId: string;
  policyId: string;
  policyName: string;
}

/** All recovery events */
export type RecoveryDomainEvent =
  | RecoveryStarted
  | RecoveryCompleted
  | RecoveryAssessed
  | RecoveryPolicyTriggered;

/** Event factory functions */
export function createRecoveryStartedEvent(
  sessionId: string,
  method: RecoveryMethod,
  attemptNumber: number,
  policyId?: string
): RecoveryStarted {
  return {
    type: 'recovery.started',
    timestamp: new Date(),
    aggregateId: sessionId,
    sessionId,
    method,
    policyId,
    attemptNumber,
  };
}

export function createRecoveryCompletedEvent(
  sessionId: string,
  method: RecoveryMethod,
  success: boolean,
  duration: number,
  newPid?: number,
  error?: string
): RecoveryCompleted {
  return {
    type: 'recovery.completed',
    timestamp: new Date(),
    aggregateId: sessionId,
    sessionId,
    method,
    success,
    newPid,
    error,
    duration,
  };
}

export function createRecoveryAssessedEvent(
  sessionId: string,
  assessment: RecoveryAssessment
): RecoveryAssessed {
  return {
    type: 'recovery.assessed',
    timestamp: new Date(),
    aggregateId: sessionId,
    sessionId,
    assessment,
  };
}

export function createRecoveryPolicyTriggeredEvent(
  sessionId: string,
  policyId: string,
  policyName: string
): RecoveryPolicyTriggered {
  return {
    type: 'recovery.policy.triggered',
    timestamp: new Date(),
    aggregateId: sessionId,
    sessionId,
    policyId,
    policyName,
  };
}
