/**
 * Runtime process attribution built on the legacy process matcher.
 */

import type { AgentClient } from '../../domain/session/index.js';
import type {
  RuntimeId,
  RuntimeScanError,
  RuntimeSession,
} from '../../domain/runtime/index.js';
import { detectSessionStatus } from '../process/detector.js';
import { getCachedProcesses } from '../process/scanner.js';
import type { ClaudeProcessInfo } from '../process/types.js';
import { matchProcessesToSessions } from '../../services/session.process-matcher.js';

export interface RuntimeProcessAttributionResult {
  sessions: RuntimeSession[];
  errors: RuntimeScanError[];
}

export type RuntimeProcessAttributor = (
  sessions: RuntimeSession[]
) => RuntimeProcessAttributionResult;

function clientForRuntime(runtimeId: RuntimeId): AgentClient | undefined {
  switch (runtimeId) {
    case 'claude-code':
      return 'claude';
    case 'codex':
      return 'codex';
    default:
      return undefined;
  }
}

function scanError(runtimeId: RuntimeId, error: unknown): RuntimeScanError {
  return {
    runtimeId,
    code: 'unknown',
    message: error instanceof Error ? error.message : String(error),
    recoverable: true,
  };
}

export function attributeRuntimeProcesses(
  runtimeId: RuntimeId,
  sessions: RuntimeSession[],
  processSource: () => ClaudeProcessInfo[] = getCachedProcesses
): RuntimeProcessAttributionResult {
  if (sessions.length === 0) {
    return { sessions, errors: [] };
  }

  const client = clientForRuntime(runtimeId);
  if (!client) {
    return { sessions, errors: [] };
  }

  let processes: ClaudeProcessInfo[];
  try {
    processes = processSource();
  } catch (error) {
    return {
      sessions,
      errors: [scanError(runtimeId, error)],
    };
  }

  const matches = matchProcessesToSessions(
    sessions.map((session) => ({
      sessionId: session.sessionId,
      client,
      directory: session.cwd,
      startedAt: session.startedAt,
      lastActiveAt: session.lastActiveAt,
      pid: session.pid,
    })),
    processes
  );

  return {
    sessions: sessions.map((session) => {
      const process = matches.get(session.sessionId);
      if (!process) {
        return {
          ...session,
          status: session.status === 'completed' ? session.status : 'lost',
          processRunning: false,
        };
      }

      return {
        ...session,
        pid: process.pid,
        tty: process.tty,
        processRunning: true,
        cpuUsage: process.cpu,
        memoryUsage: process.memory,
        status: session.status === 'completed'
          ? session.status
          : detectSessionStatus(process, session.lastActiveAt),
      };
    }),
    errors: [],
  };
}
