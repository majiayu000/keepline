/**
 * Recovery service for restoring lost sessions
 */

import { existsSync } from 'fs';
import type { Session } from '../domain/session/index.js';
import type { ISessionRepository } from '../domain/session/repository.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { RecoveryError } from '../lib/errors.js';
import { emit } from '../lib/events.js';
import { getProjectFolder } from '../lib/paths.js';
import { logger } from '../lib/logger.js';
import { isValidSessionId, assertValidSessionId } from '../lib/session-id.js';
import { renderShellCommand } from '../lib/shell-quote.js';
import { openTerminalWithCommand, printRecoveryCommand } from './terminal.js';
import type { RecoveryMethod, RecoveryOptions, RecoveryResult } from './recovery.types.js';

/** Build claude argv for recovery. */
export function buildClaudeCommandArgs(
  method: RecoveryMethod,
  sessionId: string,
  initialPrompt?: string,
  skipPermissions = false
): string[] {
  assertValidSessionId(sessionId);

  const args = ['claude'];
  if (skipPermissions) {
    args.push('--dangerously-skip-permissions');
  }

  switch (method) {
    case 'resume':
      return [...args, '--resume', sessionId];
    case 'continue':
      return [...args, '--continue'];
    case 'new':
      if (initialPrompt) {
        return [...args, initialPrompt];
      }
      return args;
  }
}

/** Build shell-safe claude command for recovery terminal apps. */
export function buildClaudeCommand(
  method: RecoveryMethod,
  sessionId: string,
  initialPrompt?: string,
  skipPermissions = false
): string {
  return renderShellCommand(buildClaudeCommandArgs(method, sessionId, initialPrompt, skipPermissions));
}

export class RecoveryService {
  constructor(private readonly repository: ISessionRepository) {}

  /** Check if session can be recovered */
  canRecover(session: Session): {
    canRecover: boolean;
    reason?: string;
    availableMethods: RecoveryMethod[];
  } {
    const availableMethods: RecoveryMethod[] = [];

    if (!isValidSessionId(session.sessionId)) {
      return {
        canRecover: false,
        reason: 'Invalid session ID format',
        availableMethods: [],
      };
    }

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
  getRecommendedMethod(session: Session): RecoveryMethod {
    const { canRecover: canRecoverSession, reason, availableMethods } = this.canRecover(session);

    if (!canRecoverSession) {
      throw new RecoveryError(session.sessionId, reason || 'Cannot recover session');
    }

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
  async recoverSession(options: RecoveryOptions): Promise<RecoveryResult> {
    if (!isValidSessionId(options.sessionId)) {
      throw new RecoveryError(options.sessionId, 'Invalid session ID format');
    }

    const session = this.repository.findBySessionId(options.sessionId);

    if (!session) {
      throw new RecoveryError(options.sessionId, 'Session not found');
    }

    const { canRecover: canRecoverSession, reason, availableMethods } = this.canRecover(session);

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
        this.repository.upsert({
          sessionId: options.sessionId,
          status: 'running',
        });

        emit('session:recovered', { session: this.repository.findBySessionId(options.sessionId)! });

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
  getRecoveryInfo(sessionId: string): {
    session: Session | null;
    canRecover: boolean;
    reason?: string;
    availableMethods: RecoveryMethod[];
    recommendedMethod?: RecoveryMethod;
    command?: string;
  } {
    if (!isValidSessionId(sessionId)) {
      return {
        session: null,
        canRecover: false,
        reason: 'Invalid session ID format',
        availableMethods: [],
      };
    }

    const session = this.repository.findBySessionId(sessionId);

    if (!session) {
      return {
        session: null,
        canRecover: false,
        reason: 'Session not found',
        availableMethods: [],
      };
    }

    const recoveryStatus = this.canRecover(session);

    if (!recoveryStatus.canRecover) {
      return {
        session,
        ...recoveryStatus,
      };
    }

    const recommendedMethod = this.getRecommendedMethod(session);
    const command = buildClaudeCommand(recommendedMethod, session.sessionId, session.initialPrompt);

    return {
      session,
      canRecover: true,
      availableMethods: recoveryStatus.availableMethods,
      recommendedMethod,
      command,
    };
  }
}

export const recoveryService = new RecoveryService(sessionRepository);

export const canRecover = recoveryService.canRecover.bind(recoveryService);
export const getRecommendedMethod = recoveryService.getRecommendedMethod.bind(recoveryService);
export const recoverSession = recoveryService.recoverSession.bind(recoveryService);
export const getRecoveryInfo = recoveryService.getRecoveryInfo.bind(recoveryService);
