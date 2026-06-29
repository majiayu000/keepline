import type { AgentClient, SessionStatus } from '../session/index.js';

export const SESSION_DIGEST_SOURCES = ['deterministic', 'local_model'] as const;
export const SESSION_DIGEST_STATUSES = ['fresh', 'stale', 'error'] as const;

export type SessionDigestSource = typeof SESSION_DIGEST_SOURCES[number];
export type SessionDigestStatus = typeof SESSION_DIGEST_STATUSES[number];

export interface SessionDigest {
  id: string;
  sessionId: string;
  summary: string;
  nextActions: string[];
  blockers: string[];
  waitingForHuman: boolean;
  source: SessionDigestSource;
  status: SessionDigestStatus;
  sourceUpdatedAt: Date;
  generatedAt: Date;
  provider?: string;
  errorMessage?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface SessionDigestUpsertInput {
  sessionId: string;
  summary: string;
  nextActions?: string[];
  blockers?: string[];
  waitingForHuman?: boolean;
  source: SessionDigestSource;
  status?: SessionDigestStatus;
  sourceUpdatedAt: Date;
  generatedAt?: Date;
  provider?: string;
  errorMessage?: string;
}

export interface SessionDigestErrorInput {
  sessionId: string;
  source: SessionDigestSource;
  sourceUpdatedAt: Date;
  provider?: string;
  errorMessage: string;
}

export interface SerializableSessionDigest {
  sessionId: string;
  summary: string;
  nextActions: string[];
  blockers: string[];
  waitingForHuman: boolean;
  source: SessionDigestSource;
  status: SessionDigestStatus;
  sourceUpdatedAt: string;
  generatedAt: string;
  provider?: string;
  errorMessage?: string;
}

export interface SessionDigestInput {
  sessionId: string;
  client: AgentClient;
  directory: string;
  status: SessionStatus;
  title: string;
  initialPrompt?: string;
  lastMessage?: string;
  lastTool?: string;
  currentFile?: string;
  lastActiveAt: Date;
  updatedAt: Date;
  memory?: {
    lastProgress?: string;
    pendingTasks?: string[];
    completedTasks?: string[];
    knownIssues?: string[];
    decisions?: string[];
    notes?: string;
    handoffNotes?: string;
    handoffPriority?: string[];
    updatedAt: Date;
  };
}

export function isSessionDigestSource(value: unknown): value is SessionDigestSource {
  return typeof value === 'string' &&
    SESSION_DIGEST_SOURCES.includes(value as SessionDigestSource);
}

export function serializeSessionDigest(digest: SessionDigest): SerializableSessionDigest {
  return {
    sessionId: digest.sessionId,
    summary: digest.summary,
    nextActions: digest.nextActions,
    blockers: digest.blockers,
    waitingForHuman: digest.waitingForHuman,
    source: digest.source,
    status: digest.status,
    sourceUpdatedAt: digest.sourceUpdatedAt.toISOString(),
    generatedAt: digest.generatedAt.toISOString(),
    provider: digest.provider,
    errorMessage: digest.errorMessage,
  };
}
