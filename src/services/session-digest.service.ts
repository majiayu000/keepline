import type { AggregatedSession } from './session.types.js';
import type {
  SessionDigest,
  SessionDigestInput,
  SessionDigestSource,
  SessionDigestUpsertInput,
} from '../domain/orchestrator/index.js';
import { sessionDigestRepository } from '../infrastructure/database/repositories/session-digest.repository.js';
import { memoryRepository } from '../infrastructure/database/repositories/memory.repository.js';

const MAX_SUMMARY_LENGTH = 500;
const MAX_ACTIONS = 5;
const MAX_BLOCKERS = 5;

export function getSessionDigest(sessionId: string): SessionDigest | null {
  return sessionDigestRepository.findBySessionId(sessionId);
}

export function getSessionDigestMap(sessionIds: string[]): Map<string, SessionDigest> {
  return new Map(
    sessionDigestRepository
      .findBySessionIds(sessionIds)
      .map((digest) => [digest.sessionId, digest])
  );
}

export function generateDeterministicSessionDigest(
  session: AggregatedSession
): SessionDigest {
  const input = buildSessionDigestInput(session);
  return sessionDigestRepository.upsert(buildDeterministicDigestUpsert(input));
}

export function generateDeterministicSessionDigests(
  sessions: AggregatedSession[]
): SessionDigest[] {
  return sessions.map(generateDeterministicSessionDigest);
}

export function markSessionDigestError(input: {
  session: AggregatedSession;
  source: SessionDigestSource;
  provider?: string;
  errorMessage: string;
}): SessionDigest {
  return sessionDigestRepository.markError({
    sessionId: input.session.sessionId,
    source: input.source,
    sourceUpdatedAt: input.session.updatedAt,
    provider: input.provider,
    errorMessage: input.errorMessage,
  });
}

export function buildSessionDigestInput(session: AggregatedSession): SessionDigestInput {
  const memory = memoryRepository.findBySessionId(session.sessionId);
  return {
    sessionId: session.sessionId,
    client: session.client,
    directory: session.directory,
    status: session.status,
    title: session.title,
    initialPrompt: session.initialPrompt,
    lastMessage: session.lastMessage,
    lastTool: session.lastTool,
    currentFile: session.currentFile,
    lastActiveAt: session.lastActiveAt,
    updatedAt: session.updatedAt,
    memory: memory ? {
      lastProgress: memory.lastProgress,
      pendingTasks: memory.pendingTasks,
      completedTasks: memory.completedTasks,
      knownIssues: memory.knownIssues,
      decisions: memory.decisions,
      notes: memory.notes,
      handoffNotes: memory.handoffNotes,
      handoffPriority: memory.handoffPriority,
      updatedAt: memory.updatedAt,
    } : undefined,
  };
}

export function buildDeterministicDigestUpsert(
  input: SessionDigestInput
): SessionDigestUpsertInput {
  const memory = input.memory;
  const nextActions = dedupeStrings([
    ...(memory?.handoffPriority ?? []),
    ...(memory?.pendingTasks ?? []),
  ]).slice(0, MAX_ACTIONS);
  const blockers = dedupeStrings([
    ...(memory?.knownIssues ?? []),
    ...(input.status === 'lost' ? ['Session is lost and may need recovery'] : []),
  ]).slice(0, MAX_BLOCKERS);

  return {
    sessionId: input.sessionId,
    summary: truncateText(firstNonEmpty([
      memory?.handoffNotes,
      memory?.lastProgress,
      input.lastMessage,
      input.title,
    ]), MAX_SUMMARY_LENGTH),
    nextActions,
    blockers,
    waitingForHuman: input.status === 'waiting',
    source: 'deterministic',
    status: 'fresh',
    sourceUpdatedAt: [
      input.updatedAt,
      input.lastActiveAt,
      memory?.updatedAt,
    ]
      .filter((value): value is Date => !!value)
      .reduce((latest, value) =>
        value.getTime() > latest.getTime() ? value : latest
      ),
  };
}

function firstNonEmpty(values: Array<string | undefined>): string {
  return values.find((value) => value?.trim())?.trim() ?? '';
}

function dedupeStrings(values: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of values) {
    const value = raw.trim();
    if (!value || seen.has(value)) continue;
    seen.add(value);
    result.push(value);
  }
  return result;
}

function truncateText(value: string, maxLength: number): string {
  return value.length <= maxLength ? value : value.slice(0, maxLength - 3) + '...';
}
