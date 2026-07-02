/**
 * Runtime-neutral agent session contracts.
 */

import type { SessionStatus, ToolCallInfo } from '../session/index.js';

export const REGISTERED_RUNTIME_IDS = ['claude-code', 'codex'] as const;

export type RegisteredRuntimeId = typeof REGISTERED_RUNTIME_IDS[number];

export type RuntimeId = RegisteredRuntimeId | (string & {});

export type RuntimeKind = 'cli' | 'ide' | 'cloud' | 'unknown';

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export type RuntimeCapability =
  | 'session-history'
  | 'process-scan'
  | 'resume'
  | 'quota'
  | 'plans'
  | 'hooks';

export type RuntimeFeatureCapability = Extract<
  RuntimeCapability,
  'quota' | 'plans' | 'hooks'
>;

export interface RuntimeQuotaProvider {
  getQuota(): Promise<JsonValue>;
}

export interface RuntimePlansProvider {
  scanPlans(options?: Record<string, JsonValue>): Promise<JsonValue[]>;
}

export interface RuntimeHooksProvider {
  installHooks?(): Promise<JsonValue>;
  getHookStatus?(): Promise<JsonValue>;
}

export interface RuntimeFeatureProviders {
  quota: RuntimeQuotaProvider;
  plans: RuntimePlansProvider;
  hooks: RuntimeHooksProvider;
}

export type RuntimeCompatibilityRoutes = Partial<
  Record<RuntimeFeatureCapability, readonly string[]>
>;

export interface RuntimeDescriptor {
  id: RuntimeId;
  displayName: string;
  kind: RuntimeKind;
  executableNames: string[];
  sessionPathHints: string[];
  capabilities: RuntimeCapability[];
  featureProviders?: Partial<RuntimeFeatureProviders>;
  compatibilityRoutes?: RuntimeCompatibilityRoutes;
}

export interface RuntimeUsageStats {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  totalCost: number;
  apiCalls: number;
  cacheReadTokens?: number;
  cacheWriteTokens?: number;
  model?: string;
  modelBreakdown?: JsonValue[];
}

export interface RuntimeSession {
  runtimeId: RuntimeId;
  sessionId: string;
  sourcePath?: string;
  cwd: string;
  projectRoot?: string;

  agentId?: string;
  parentSessionId?: string;
  parentRuntimeSessionId?: string;
  isSubAgent?: boolean;

  pid?: number;
  tty?: string;
  processRunning?: boolean;
  cpuUsage?: number;
  memoryUsage?: number;

  status: SessionStatus | 'unknown';
  title: string;
  initialPrompt?: string;
  lastMessage?: string;
  lastTool?: string;
  lastToolInput?: Record<string, unknown>;
  currentFile?: string;
  filesTouched: string[];
  toolCalls?: ToolCallInfo[];
  toolCount: number;
  messageCount: number;

  startedAt?: Date;
  lastActiveAt: Date;
  completedAt?: Date;

  usageStats?: RuntimeUsageStats;
  runtimeMetadata?: Record<string, JsonValue>;
}

export interface RuntimeCommand {
  executable: string;
  args: string[];
  cwd?: string;
}

export type RuntimeRecoveryMethod = 'resume' | 'continue' | 'new';

export interface RuntimeRecoveryOptions {
  method: RuntimeRecoveryMethod;
  initialPrompt?: string;
  skipPermissions?: boolean;
}

export interface RuntimeScanOptions {
  maxAgeDays?: number;
  mode?: 'basic' | 'full';
  includeSubAgents?: boolean;
  projectRoot?: string;
}

export type RuntimeScanErrorCode =
  | 'missing-root'
  | 'read-failed'
  | 'parse-failed'
  | 'unsupported-schema'
  | 'unknown';

export interface RuntimeScanError {
  runtimeId: RuntimeId;
  code: RuntimeScanErrorCode;
  message: string;
  sourcePath?: string;
  recoverable: boolean;
}

export interface RuntimeScanResult {
  runtime: RuntimeDescriptor;
  sessions: RuntimeSession[];
  errors: RuntimeScanError[];
}

export interface AgentRuntimeAdapter {
  descriptor: RuntimeDescriptor;
  scanSessions(options?: RuntimeScanOptions): Promise<RuntimeScanResult>;
  buildRecoveryCommand?: (
    session: RuntimeSession,
    options: RuntimeRecoveryOptions
  ) => RuntimeCommand | undefined;
}
