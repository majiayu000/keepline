/**
 * Session/process matching helpers.
 *
 * Claude often has multiple sessions in the same repository, so matching
 * purely by cwd collapses distinct sessions onto a single process.
 *
 * This matcher prefers:
 * 1. Existing PID continuity from previous syncs
 * 2. One-to-one matching within a directory based on start-time proximity
 * 3. Recent activity as a tie-breaker
 */

import type { ClaudeProcessInfo } from '../adapters/process/types.js';
import type { AgentClient } from '../domain/session/index.js';

const UNMATCHED_PROCESS_PENALTY = 30 * 24 * 60 * 60 * 1000;
const MAX_ACTIVITY_AGE_PENALTY = 24 * 60 * 60 * 1000;
const START_TIME_WEIGHT = 10;

export interface SessionProcessCandidate {
  sessionId: string;
  client?: AgentClient;
  directory: string;
  startedAt?: Date;
  lastActiveAt: Date;
  pid?: number;
}

interface SessionPairingMeta<T extends SessionProcessCandidate> {
  session: T;
  referenceTimeMs: number;
  activityAgePenaltyMs: number;
  lastActiveAtMs: number;
}

interface CandidateSession<T extends SessionProcessCandidate> {
  meta: SessionPairingMeta<T>;
  bestCost: number;
}

function buildSessionPairingMeta<T extends SessionProcessCandidate>(
  session: T,
  nowMs: number
): SessionPairingMeta<T> {
  const referenceTimeMs = (session.startedAt ?? session.lastActiveAt).getTime();
  const activityAge = Math.max(0, nowMs - session.lastActiveAt.getTime());
  return {
    session,
    referenceTimeMs,
    activityAgePenaltyMs: Math.min(activityAge, MAX_ACTIVITY_AGE_PENALTY),
    lastActiveAtMs: session.lastActiveAt.getTime(),
  };
}

function pairingCost(
  sessionMeta: SessionPairingMeta<SessionProcessCandidate>,
  processStartTimeMs: number
): number {
  const startDelta = Math.abs(sessionMeta.referenceTimeMs - processStartTimeMs);
  return startDelta * START_TIME_WEIGHT + sessionMeta.activityAgePenaltyMs;
}

function compareCandidates<T extends SessionProcessCandidate>(
  left: CandidateSession<T>,
  right: CandidateSession<T>
): number {
  if (left.bestCost !== right.bestCost) return left.bestCost - right.bestCost;
  if (left.meta.lastActiveAtMs !== right.meta.lastActiveAtMs) {
    return right.meta.lastActiveAtMs - left.meta.lastActiveAtMs;
  }
  return left.meta.session.sessionId.localeCompare(right.meta.session.sessionId);
}

function selectCandidateSessions<T extends SessionProcessCandidate>(
  sessions: T[],
  processStartTimesMs: number[],
  nowMs: number,
  candidateLimit: number
): Array<SessionPairingMeta<T>> {
  const selected: CandidateSession<T>[] = [];

  for (const session of sessions) {
    const meta = buildSessionPairingMeta(session, nowMs);
    let bestCost = Number.POSITIVE_INFINITY;
    for (const processStartTimeMs of processStartTimesMs) {
      const cost = pairingCost(meta, processStartTimeMs);
      if (cost < bestCost) bestCost = cost;
    }

    const candidate = { meta, bestCost };
    let insertAt = selected.length;
    for (let i = 0; i < selected.length; i++) {
      if (compareCandidates(candidate, selected[i]) < 0) {
        insertAt = i;
        break;
      }
    }

    if (insertAt >= candidateLimit) {
      continue;
    }

    selected.splice(insertAt, 0, candidate);
    if (selected.length > candidateLimit) {
      selected.pop();
    }
  }

  return selected.map((candidate) => candidate.meta);
}

function getBestAssignment(
  costMatrix: number[][],
  processCount: number
): Array<number | null> {
  const sessionCount = costMatrix.length;
  if (sessionCount === 0 || processCount === 0) {
    return new Array(processCount).fill(null);
  }

  const maskBase = 1 << sessionCount;
  const memoCost = new Map<number, number>();
  const memoChoice = new Map<number, number>();
  const unmatchedChoice = -1;

  const solve = (processIndex: number, usedMask: number): number => {
    if (processIndex >= processCount) {
      return 0;
    }

    const memoKey = processIndex * maskBase + usedMask;
    const cachedCost = memoCost.get(memoKey);
    if (cachedCost !== undefined) {
      return cachedCost;
    }

    let bestCost = solve(processIndex + 1, usedMask) + UNMATCHED_PROCESS_PENALTY;
    let bestChoice = unmatchedChoice;

    for (let sessionIndex = 0; sessionIndex < sessionCount; sessionIndex++) {
      const bit = 1 << sessionIndex;
      if ((usedMask & bit) !== 0) continue;

      const totalCost = costMatrix[sessionIndex][processIndex] + solve(processIndex + 1, usedMask | bit);

      if (totalCost < bestCost) {
        bestCost = totalCost;
        bestChoice = sessionIndex;
      }
    }

    memoCost.set(memoKey, bestCost);
    memoChoice.set(memoKey, bestChoice);
    return bestCost;
  };

  solve(0, 0);

  const assignment: Array<number | null> = new Array(processCount).fill(null);
  let usedMask = 0;
  for (let processIndex = 0; processIndex < processCount; processIndex++) {
    const memoKey = processIndex * maskBase + usedMask;
    const choice = memoChoice.get(memoKey);
    if (choice === undefined || choice === unmatchedChoice) continue;

    assignment[processIndex] = choice;
    usedMask |= 1 << choice;
  }

  return assignment;
}

export function matchProcessesToSessions<T extends SessionProcessCandidate>(
  sessions: T[],
  processes: ClaudeProcessInfo[]
): Map<string, ClaudeProcessInfo> {
  const matches = new Map<string, ClaudeProcessInfo>();
  const sessionsByDirectory = new Map<string, T[]>();
  const processesByDirectory = new Map<string, ClaudeProcessInfo[]>();
  const groupKey = (client: AgentClient | undefined, directory: string) =>
    `${client ?? 'claude'}\u0000${directory}`;

  for (const session of sessions) {
    const key = groupKey(session.client, session.directory);
    const existing = sessionsByDirectory.get(key) || [];
    existing.push(session);
    sessionsByDirectory.set(key, existing);
  }

  for (const process of processes) {
    const key = groupKey(process.client, process.cwd);
    const existing = processesByDirectory.get(key) || [];
    existing.push(process);
    processesByDirectory.set(key, existing);
  }

  const nowMs = Date.now();

  for (const [directoryKey, directorySessions] of sessionsByDirectory.entries()) {
    const directoryProcesses = processesByDirectory.get(directoryKey) || [];
    if (directoryProcesses.length === 0) continue;

    const unmatchedSessions: T[] = [];
    const usedProcessPids = new Set<number>();
    const hasKnownPid = directorySessions.some((session) => session.pid !== undefined);

    if (hasKnownPid) {
      const processByPid = new Map(directoryProcesses.map((process) => [process.pid, process]));

      // Prefer stable PID continuity when the previous sync already knew the process.
      for (const session of directorySessions) {
        if (!session.pid) {
          unmatchedSessions.push(session);
          continue;
        }

        const process = processByPid.get(session.pid);
        if (!process || usedProcessPids.has(process.pid)) {
          unmatchedSessions.push(session);
          continue;
        }

        matches.set(session.sessionId, process);
        usedProcessPids.add(process.pid);
      }
    } else {
      unmatchedSessions.push(...directorySessions);
    }

    const unmatchedProcesses = directoryProcesses.filter(
      (process) => !usedProcessPids.has(process.pid)
    );

    if (unmatchedSessions.length === 0 || unmatchedProcesses.length === 0) {
      continue;
    }

    // Only consider the closest sessions for the remaining processes so the
    // assignment step stays tractable even when a repository has long history.
    const candidateLimit = Math.min(
      unmatchedSessions.length,
      20,
      Math.max(unmatchedProcesses.length * 2, unmatchedProcesses.length + 2)
    );

    const orderedProcesses = [...unmatchedProcesses].sort(
      (left, right) => left.startTime.getTime() - right.startTime.getTime()
    );
    const processStartTimesMs = orderedProcesses.map((process) => process.startTime.getTime());

    const candidateMetas = selectCandidateSessions(
      unmatchedSessions,
      processStartTimesMs,
      nowMs,
      candidateLimit
    );
    const costMatrix = candidateMetas.map((meta) =>
      processStartTimesMs.map((processStartTimeMs) => pairingCost(meta, processStartTimeMs))
    );
    const assignment = getBestAssignment(costMatrix, orderedProcesses.length);

    assignment.forEach((sessionIndex, processIndex) => {
      if (sessionIndex === null || sessionIndex === undefined) return;
      const session = candidateMetas[sessionIndex].session;
      const process = orderedProcesses[processIndex];
      matches.set(session.sessionId, process);
    });
  }

  return matches;
}
