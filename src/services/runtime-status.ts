import {
  REGISTERED_RUNTIME_IDS,
  type RegisteredRuntimeId,
  type RuntimeId,
  type RuntimeScanError,
} from '../domain/runtime/index.js';
import type { AgentClient } from '../domain/session/index.js';

export type SessionRuntimeId = RegisteredRuntimeId;
export type RuntimeFilter = SessionRuntimeId | 'all';

export const SESSION_RUNTIME_IDS: readonly SessionRuntimeId[] = REGISTERED_RUNTIME_IDS;

const CLIENT_RUNTIME_IDS = {
  claude: 'claude-code',
  codex: 'codex',
} satisfies Record<AgentClient, SessionRuntimeId>;

const RUNTIME_CLIENTS = {
  'claude-code': 'claude',
  codex: 'codex',
} satisfies Record<SessionRuntimeId, AgentClient>;

export interface RuntimeScanSummary {
  runtimeId: SessionRuntimeId;
  degraded: boolean;
  errorCount: number;
  errors: RuntimeScanError[];
  lastScanAt?: string;
}

export type RuntimeScanFailure = {
  filePath?: string;
  message: string;
  code?: RuntimeScanError['code'];
  recoverable?: boolean;
};

const latestRuntimeScan = new Map<SessionRuntimeId, RuntimeScanSummary>();

export function isSessionRuntimeId(runtimeId: string): runtimeId is SessionRuntimeId {
  return (SESSION_RUNTIME_IDS as readonly string[]).includes(runtimeId);
}

export function runtimeIdForClient(client: AgentClient): SessionRuntimeId {
  const runtimeId = CLIENT_RUNTIME_IDS[client];
  if (!runtimeId) {
    throw new Error(`Unsupported agent client: ${String(client)}`);
  }
  return runtimeId;
}

export function clientForRuntimeId(runtimeId: RuntimeId): AgentClient {
  if (!isSessionRuntimeId(runtimeId)) {
    throw new Error(`Unsupported runtime id: ${String(runtimeId)}`);
  }
  return RUNTIME_CLIENTS[runtimeId];
}

export function parseRuntimeFilter(raw: string | undefined): {
  runtimeId?: SessionRuntimeId;
  invalid?: string;
} {
  const value = raw?.trim();
  if (!value || value === 'all') return {};
  if (isSessionRuntimeId(value)) {
    return { runtimeId: value };
  }
  return { invalid: value };
}

export function recordRuntimeScanFailures(
  runtimeId: SessionRuntimeId,
  failures: RuntimeScanFailure[]
): void {
  latestRuntimeScan.set(runtimeId, {
    runtimeId,
    degraded: failures.length > 0,
    errorCount: failures.length,
    errors: failures.slice(0, 10).map((failure) => ({
      runtimeId: runtimeId as RuntimeId,
      code: failure.code ?? 'parse-failed',
      message: failure.message,
      sourcePath: failure.filePath,
      recoverable: failure.recoverable ?? true,
    })),
    lastScanAt: new Date().toISOString(),
  });
}

export function getRuntimeScanStatus(): RuntimeScanSummary[] {
  return SESSION_RUNTIME_IDS.map((runtimeId) =>
    latestRuntimeScan.get(runtimeId) ?? {
      runtimeId,
      degraded: false,
      errorCount: 0,
      errors: [],
    }
  );
}

export function clearRuntimeScanStatus(): void {
  latestRuntimeScan.clear();
}
