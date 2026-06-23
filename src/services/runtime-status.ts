import type { RuntimeId, RuntimeScanError } from '../domain/runtime/index.js';
import type { AgentClient } from '../domain/session/index.js';

export type SessionRuntimeId = 'claude-code' | 'codex';
export type RuntimeFilter = SessionRuntimeId | 'all';

export const SESSION_RUNTIME_IDS: readonly SessionRuntimeId[] = ['claude-code', 'codex'] as const;

export interface RuntimeScanSummary {
  runtimeId: SessionRuntimeId;
  degraded: boolean;
  errorCount: number;
  errors: RuntimeScanError[];
  lastScanAt?: string;
}

type ScanFailure = {
  filePath: string;
  message: string;
};

const latestRuntimeScan = new Map<SessionRuntimeId, RuntimeScanSummary>();

export function runtimeIdForClient(client: AgentClient | undefined): SessionRuntimeId {
  return client === 'codex' ? 'codex' : 'claude-code';
}

export function clientForRuntimeId(runtimeId: SessionRuntimeId): AgentClient {
  return runtimeId === 'codex' ? 'codex' : 'claude';
}

export function parseRuntimeFilter(raw: string | undefined): {
  runtimeId?: SessionRuntimeId;
  invalid?: string;
} {
  const value = raw?.trim();
  if (!value || value === 'all') return {};
  if (value === 'claude-code' || value === 'codex') {
    return { runtimeId: value };
  }
  return { invalid: value };
}

export function recordRuntimeScanFailures(
  runtimeId: SessionRuntimeId,
  failures: ScanFailure[]
): void {
  latestRuntimeScan.set(runtimeId, {
    runtimeId,
    degraded: failures.length > 0,
    errorCount: failures.length,
    errors: failures.slice(0, 10).map((failure) => ({
      runtimeId: runtimeId as RuntimeId,
      code: 'parse-failed',
      message: failure.message,
      sourcePath: failure.filePath,
      recoverable: true,
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
