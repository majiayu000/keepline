/**
 * Recovery domain entities
 */

import type { Entity } from '../shared/types.js';
import type { RecoveryMethod } from './types.js';

/** Recovery attempt status */
export type RecoveryAttemptStatus =
  | 'pending'
  | 'running'
  | 'success'
  | 'failed';

/** Recovery attempt entity - tracks recovery history */
export interface RecoveryAttempt extends Entity {
  /** Session being recovered */
  sessionId: string;

  /** Recovery policy ID (if auto-recovery) */
  policyId?: string;

  /** Method used */
  method: RecoveryMethod;

  /** Current status */
  status: RecoveryAttemptStatus;

  /** When recovery started */
  startedAt: Date;

  /** When recovery completed */
  completedAt?: Date;

  /** Error message if failed */
  error?: string;

  /** New PID if successful */
  newPid?: number;

  /** Assessment result (JSON) */
  assessment?: string;
}

/** Recovery policy entity - defines auto-recovery rules */
export interface RecoveryPolicy extends Entity {
  /** Policy name */
  name: string;

  /** Is policy enabled */
  enabled: boolean;

  /** Trigger conditions */
  trigger: {
    status: 'lost';
    minAgeSeconds: number;
    maxAgeSeconds: number;
    directoryPattern?: string;
    priority?: number;
  };

  /** Action configuration */
  action: {
    method: RecoveryMethod | 'evaluate';
    injectContext: boolean;
    openTerminal: boolean;
    terminalApp?: 'Terminal' | 'iTerm' | 'Warp' | 'auto';
    skipPermissions: boolean;
  };

  /** Retry configuration */
  retry: {
    maxAttempts: number;
    backoff: 'linear' | 'exponential';
    initialDelayMs: number;
    maxDelayMs: number;
  };

  /** Notification configuration */
  notify: {
    onStart: boolean;
    onSuccess: boolean;
    onFailure: boolean;
    channels: string[];
  };

  /** Priority (higher = evaluated first) */
  priority: number;
}

/** Recovery assessment result */
export interface RecoveryAssessment {
  sessionId: string;
  isRecoverable: boolean;
  reason: string;
  riskLevel: 'low' | 'medium' | 'high';

  /** Recommendation */
  recommendedMethod: RecoveryMethod;
  contextAvailable: boolean;
  estimatedSuccessRate: number;

  /** Checks performed */
  checks: {
    sessionFileExists: boolean;
    directoryExists: boolean;
    lastActivityAge: number;
    previousAttempts: number;
  };
}

/** Input for creating a recovery policy */
export interface CreateRecoveryPolicyInput {
  name: string;
  enabled?: boolean;
  trigger: RecoveryPolicy['trigger'];
  action: RecoveryPolicy['action'];
  retry?: RecoveryPolicy['retry'];
  notify?: RecoveryPolicy['notify'];
  priority?: number;
}
