/**
 * API Input Validation
 *
 * Exported validation functions for request body validation.
 * These are used by the API server and can be tested independently.
 */

/** Valid recovery methods */
export const VALID_RECOVERY_METHODS = ['resume', 'continue', 'new'] as const;
export type RecoveryMethod = (typeof VALID_RECOVERY_METHODS)[number];

/** Valid terminal apps */
export const VALID_TERMINAL_APPS = ['Terminal', 'iTerm', 'Warp', 'auto'] as const;
export type TerminalAppOption = (typeof VALID_TERMINAL_APPS)[number];

export { isValidSessionId } from '../../../lib/session-id.js';

/** Recovery request body */
export interface RecoverRequestBody {
  method?: RecoveryMethod;
  openTerminal?: boolean;
  skipPermissions?: boolean;
  terminalApp?: TerminalAppOption;
}

/** Validation result type */
export type ValidationResult<T> =
  | { valid: true; data: T }
  | { valid: false; error: string };

/** Validate recovery request body */
export function validateRecoverRequest(
  body: unknown
): ValidationResult<RecoverRequestBody> {
  if (body === null || typeof body !== 'object') {
    return { valid: true, data: {} }; // Empty body is OK, use defaults
  }

  const obj = body as Record<string, unknown>;

  // Validate method if provided
  if (obj.method !== undefined) {
    if (
      typeof obj.method !== 'string' ||
      !VALID_RECOVERY_METHODS.includes(obj.method as RecoveryMethod)
    ) {
      return {
        valid: false,
        error: `Invalid method. Must be one of: ${VALID_RECOVERY_METHODS.join(', ')}`,
      };
    }
  }

  // Validate booleans if provided
  if (obj.openTerminal !== undefined && typeof obj.openTerminal !== 'boolean') {
    return { valid: false, error: 'openTerminal must be a boolean' };
  }

  if (obj.skipPermissions !== undefined && typeof obj.skipPermissions !== 'boolean') {
    return { valid: false, error: 'skipPermissions must be a boolean' };
  }

  // Validate terminalApp if provided
  if (obj.terminalApp !== undefined) {
    if (
      typeof obj.terminalApp !== 'string' ||
      !VALID_TERMINAL_APPS.includes(obj.terminalApp as TerminalAppOption)
    ) {
      return {
        valid: false,
        error: `Invalid terminalApp. Must be one of: ${VALID_TERMINAL_APPS.join(', ')}`,
      };
    }
  }

  return {
    valid: true,
    data: {
      method: obj.method as RecoveryMethod | undefined,
      openTerminal: obj.openTerminal as boolean | undefined,
      skipPermissions: obj.skipPermissions as boolean | undefined,
      terminalApp: obj.terminalApp as TerminalAppOption | undefined,
    },
  };
}

/** Stop request body */
export interface StopRequestBody {
  force?: boolean;
}

/** Validate stop request body */
export function validateStopRequest(
  body: unknown
): ValidationResult<StopRequestBody> {
  if (body === null || typeof body !== 'object') {
    return { valid: true, data: {} };
  }

  const obj = body as Record<string, unknown>;

  if (obj.force !== undefined && typeof obj.force !== 'boolean') {
    return { valid: false, error: 'force must be a boolean' };
  }

  return { valid: true, data: { force: obj.force as boolean | undefined } };
}
