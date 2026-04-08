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

const UNMATCHED_PROCESS_PENALTY = 30 * 24 * 60 * 60 * 1000;

export interface SessionProcessCandidate {
  sessionId: string;
  directory: string;
  startedAt?: Date;
  lastActiveAt: Date;
  pid?: number;
}

function referenceTime(session: SessionProcessCandidate): number {
  return (session.startedAt ?? session.lastActiveAt).getTime();
}

function pairingCost(session: SessionProcessCandidate, process: ClaudeProcessInfo): number {
  const startDelta = Math.abs(referenceTime(session) - process.startTime.getTime());
  const activityAge = Math.max(0, Date.now() - session.lastActiveAt.getTime());
  return startDelta * 10 + Math.min(activityAge, 24 * 60 * 60 * 1000);
}

function getBestAssignment(
  sessions: SessionProcessCandidate[],
  processes: ClaudeProcessInfo[]
): Array<number | null> {
  const memo = new Map<string, { cost: number; assignment: Array<number | null> }>();

  const solve = (processIndex: number, usedMask: number): { cost: number; assignment: Array<number | null> } => {
    if (processIndex >= processes.length) {
      return { cost: 0, assignment: [] };
    }

    const memoKey = `${processIndex}:${usedMask}`;
    const cached = memo.get(memoKey);
    if (cached) {
      return cached;
    }

    let best = solve(processIndex + 1, usedMask);
    best = {
      cost: best.cost + UNMATCHED_PROCESS_PENALTY,
      assignment: [null, ...best.assignment],
    };

    for (let sessionIndex = 0; sessionIndex < sessions.length; sessionIndex++) {
      const bit = 1 << sessionIndex;
      if ((usedMask & bit) !== 0) continue;

      const remainder = solve(processIndex + 1, usedMask | bit);
      const totalCost =
        pairingCost(sessions[sessionIndex], processes[processIndex]) + remainder.cost;

      if (totalCost < best.cost) {
        best = {
          cost: totalCost,
          assignment: [sessionIndex, ...remainder.assignment],
        };
      }
    }

    memo.set(memoKey, best);
    return best;
  };

  return solve(0, 0).assignment;
}

export function matchProcessesToSessions<T extends SessionProcessCandidate>(
  sessions: T[],
  processes: ClaudeProcessInfo[]
): Map<string, ClaudeProcessInfo> {
  const matches = new Map<string, ClaudeProcessInfo>();
  const sessionsByDirectory = new Map<string, T[]>();
  const processesByDirectory = new Map<string, ClaudeProcessInfo[]>();

  for (const session of sessions) {
    const existing = sessionsByDirectory.get(session.directory) || [];
    existing.push(session);
    sessionsByDirectory.set(session.directory, existing);
  }

  for (const process of processes) {
    const existing = processesByDirectory.get(process.cwd) || [];
    existing.push(process);
    processesByDirectory.set(process.cwd, existing);
  }

  for (const [directory, directorySessions] of sessionsByDirectory.entries()) {
    const directoryProcesses = [...(processesByDirectory.get(directory) || [])];
    if (directoryProcesses.length === 0) continue;

    const unmatchedSessions = [...directorySessions];
    const unmatchedProcesses = [...directoryProcesses];
    const processByPid = new Map(unmatchedProcesses.map((process) => [process.pid, process]));

    // Prefer stable PID continuity when the previous sync already knew the process.
    for (let index = unmatchedSessions.length - 1; index >= 0; index--) {
      const session = unmatchedSessions[index];
      if (!session.pid) continue;

      const process = processByPid.get(session.pid);
      if (!process) continue;

      matches.set(session.sessionId, process);
      processByPid.delete(session.pid);
      unmatchedSessions.splice(index, 1);
      const processIndex = unmatchedProcesses.findIndex((candidate) => candidate.pid === process.pid);
      if (processIndex >= 0) {
        unmatchedProcesses.splice(processIndex, 1);
      }
    }

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

    const candidateSessions = [...unmatchedSessions]
      .sort((left, right) => {
        const leftBest = Math.min(...unmatchedProcesses.map((process) => pairingCost(left, process)));
        const rightBest = Math.min(...unmatchedProcesses.map((process) => pairingCost(right, process)));
        if (leftBest !== rightBest) return leftBest - rightBest;
        if (left.lastActiveAt.getTime() !== right.lastActiveAt.getTime()) {
          return right.lastActiveAt.getTime() - left.lastActiveAt.getTime();
        }
        return left.sessionId.localeCompare(right.sessionId);
      })
      .slice(0, candidateLimit);

    const orderedProcesses = [...unmatchedProcesses].sort(
      (left, right) => left.startTime.getTime() - right.startTime.getTime()
    );
    const assignment = getBestAssignment(candidateSessions, orderedProcesses);

    assignment.forEach((sessionIndex, processIndex) => {
      if (sessionIndex === null || sessionIndex === undefined) return;
      const session = candidateSessions[sessionIndex];
      const process = orderedProcesses[processIndex];
      matches.set(session.sessionId, process);
    });
  }

  return matches;
}
