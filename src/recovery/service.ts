/**
 * Recovery service for restoring lost sessions
 */

import { existsSync } from 'fs';
import type { Session } from '../core/types.js';
import { RecoveryError } from '../core/errors.js';
import { emit } from '../core/events.js';
import { sessionRepo } from '../storage/index.js';
import { getProjectFolder } from '../utils/paths.js';
import { logger } from '../utils/logger.js';
import { openTerminalWithCommand, printRecoveryCommand } from './terminal.js';
import type { RecoveryMethod, RecoveryOptions, RecoveryResult } from './types.js';

/** Build claude command for recovery */
function buildClaudeCommand(
  method: RecoveryMethod,
  sessionId: string,
  initialPrompt?: string,
  skipPermissions = false
): string {
  const baseCmd = skipPermissions ? 'claude --dangerously-skip-permissions' : 'claude';

  switch (method) {
    case 'resume':
      return `${baseCmd} --resume ${sessionId}`;
    case 'continue':
      return `${baseCmd} --continue`;
    case 'new':
      if (initialPrompt) {
        // Escape the prompt for shell
        const escapedPrompt = initialPrompt.replace(/'/g, "'\\''");
        return `${baseCmd} '${escapedPrompt}'`;
      }
      return baseCmd;
  }
}

/** Check if session can be recovered */
export function canRecover(session: Session): {
  canRecover: boolean;
  reason?: string;
  availableMethods: RecoveryMethod[];
} {
  const availableMethods: RecoveryMethod[] = [];

  // Check if session file exists
  const projectFolder = getProjectFolder(session.directory);
  const sessionFile = `${projectFolder}/${session.sessionId}.jsonl`;

  if (existsSync(sessionFile)) {
    availableMethods.push('resume');
  }

  // Continue is always available if directory exists
  if (existsSync(session.directory)) {
    availableMethods.push('continue');
    availableMethods.push('new');
  }

  if (availableMethods.length === 0) {
    return {
      canRecover: false,
      reason: 'Session directory no longer exists',
      availableMethods: [],
    };
  }

  return {
    canRecover: true,
    availableMethods,
  };
}

/** Get recommended recovery method */
export function getRecommendedMethod(session: Session): RecoveryMethod {
  const { availableMethods } = canRecover(session);

  // Prefer resume if available
  if (availableMethods.includes('resume')) {
    return 'resume';
  }

  // Fall back to continue
  if (availableMethods.includes('continue')) {
    return 'continue';
  }

  // Last resort: new session
  return 'new';
}

/** Recover a lost session */
export async function recoverSession(options: RecoveryOptions): Promise<RecoveryResult> {
  const session = sessionRepo.findBySessionId(options.sessionId);

  if (!session) {
    throw new RecoveryError(options.sessionId, 'Session not found');
  }

  const { canRecover: canRecoverSession, reason, availableMethods } = canRecover(session);

  if (!canRecoverSession) {
    throw new RecoveryError(options.sessionId, reason || 'Cannot recover session');
  }

  if (!availableMethods.includes(options.method)) {
    throw new RecoveryError(
      options.sessionId,
      `Method ${options.method} not available. Available: ${availableMethods.join(', ')}`
    );
  }

  const command = buildClaudeCommand(
    options.method,
    options.sessionId,
    session.initialPrompt,
    options.skipPermissions
  );

  logger.info(`Recovering session ${options.sessionId} using ${options.method}`);

  try {
    if (options.openTerminal) {
      // Open new terminal window
      openTerminalWithCommand(command, options.directory, options.terminalApp ?? 'auto');

      // Update session status
      sessionRepo.upsert({
        sessionId: options.sessionId,
        status: 'running',
      });

      emit('session:recovered', { session: sessionRepo.findBySessionId(options.sessionId)! });

      return {
        success: true,
        method: options.method,
        sessionId: options.sessionId,
      };
    } else {
      // Just print command for user
      printRecoveryCommand(command, options.directory);

      return {
        success: true,
        method: options.method,
        sessionId: options.sessionId,
      };
    }
  } catch (error) {
    const errorMessage = (error as Error).message;
    logger.error(`Failed to recover session: ${errorMessage}`);

    return {
      success: false,
      method: options.method,
      sessionId: options.sessionId,
      error: errorMessage,
    };
  }
}

/** Get recovery info for a session */
export function getRecoveryInfo(sessionId: string): {
  session: Session | null;
  canRecover: boolean;
  reason?: string;
  availableMethods: RecoveryMethod[];
  recommendedMethod?: RecoveryMethod;
  command?: string;
} {
  const session = sessionRepo.findBySessionId(sessionId);

  if (!session) {
    return {
      session: null,
      canRecover: false,
      reason: 'Session not found',
      availableMethods: [],
    };
  }

  const recoveryStatus = canRecover(session);

  if (!recoveryStatus.canRecover) {
    return {
      session,
      ...recoveryStatus,
    };
  }

  const recommendedMethod = getRecommendedMethod(session);
  const command = buildClaudeCommand(recommendedMethod, session.sessionId, session.initialPrompt);

  return {
    session,
    canRecover: true,
    availableMethods: recoveryStatus.availableMethods,
    recommendedMethod,
    command,
  };
}
