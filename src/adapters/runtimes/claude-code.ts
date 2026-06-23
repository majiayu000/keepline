/**
 * Claude Code runtime adapter.
 */

import type { ParsedSessionData } from '../../domain/session/index.js';
import type {
  AgentRuntimeAdapter,
  RuntimeDescriptor,
  RuntimeRecoveryOptions,
  RuntimeScanError,
  RuntimeScanOptions,
  RuntimeScanResult,
  RuntimeSession,
} from '../../domain/runtime/index.js';
import { buildClaudeCommandArgs } from '../../services/recovery.service.js';
import {
  getAllSessionsWithFailures as getAllClaudeSessionsWithFailures,
  type ClaudeSessionScanFailure,
  type ScanOptions,
} from '../claude/scanner.js';
import { parsedSessionToRuntimeSession, structuredCommand } from './session-mapper.js';

interface ClaudeRuntimeScanSourceResult {
  sessions: ParsedSessionData[];
  failures?: ClaudeSessionScanFailure[];
}

export type ClaudeCodeSessionScanner = (
  options?: ScanOptions
) => Promise<ParsedSessionData[] | ClaudeRuntimeScanSourceResult>;

export const CLAUDE_CODE_DESCRIPTOR: RuntimeDescriptor = {
  id: 'claude-code',
  displayName: 'Claude Code',
  kind: 'cli',
  executableNames: ['claude', 'claude-code'],
  sessionPathHints: [
    '~/.claude/projects/<project>/<session>.jsonl',
    '~/.claude-work/projects/<project>/<session>.jsonl',
  ],
  capabilities: ['session-history', 'process-scan', 'resume', 'quota', 'plans', 'hooks'],
  compatibilityRoutes: {
    quota: ['/api/quota'],
    plans: ['/api/plans'],
    hooks: ['/api/hooks', 'keepline hooks install', 'keepline hooks status'],
  },
};

export class ClaudeCodeRuntimeAdapter implements AgentRuntimeAdapter {
  readonly descriptor = CLAUDE_CODE_DESCRIPTOR;

  constructor(
    private readonly scanClaudeSessions: ClaudeCodeSessionScanner = getAllClaudeSessionsWithFailures
  ) {}

  async scanSessions(options: RuntimeScanOptions = {}): Promise<RuntimeScanResult> {
    try {
      const scanResult = await this.scanClaudeSessions({
        includeSubAgents: options.includeSubAgents,
        includeToolCalls: options.mode === 'full',
        maxAgeDays: options.maxAgeDays,
      });
      const sessions = Array.isArray(scanResult) ? scanResult : scanResult.sessions;
      const failures = Array.isArray(scanResult) ? [] : scanResult.failures ?? [];

      return {
        runtime: this.descriptor,
        sessions: sessions.map((session) =>
          parsedSessionToRuntimeSession(this.descriptor.id, session, {
            legacyClient: 'claude',
          })
        ),
        errors: failures.map((failure) => ({
          runtimeId: this.descriptor.id,
          code: 'parse-failed',
          message: failure.message,
          sourcePath: failure.filePath,
          recoverable: true,
        })),
      };
    } catch (error) {
      const scanError: RuntimeScanError = {
        runtimeId: this.descriptor.id,
        code: 'unknown',
        message: error instanceof Error ? error.message : String(error),
        recoverable: true,
      };
      return {
        runtime: this.descriptor,
        sessions: [],
        errors: [scanError],
      };
    }
  }

  buildRecoveryCommand(
    session: RuntimeSession,
    options: RuntimeRecoveryOptions
  ) {
    return structuredCommand(
      buildClaudeCommandArgs(
        options.method,
        session.sessionId,
        options.initialPrompt ?? session.initialPrompt,
        options.skipPermissions ?? false
      ),
      session.cwd
    );
  }
}
