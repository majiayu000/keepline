import { describe, expect, test } from 'bun:test';
import { matchProcessesToSessions } from '../services/session.process-matcher.js';
import type { ClaudeProcessInfo } from '../adapters/process/types.js';

function sessionCandidate(overrides: Partial<{
  sessionId: string;
  client: 'claude' | 'codex';
  directory: string;
  startedAt: Date;
  lastActiveAt: Date;
  pid: number;
}> = {}) {
  return {
    sessionId: overrides.sessionId || crypto.randomUUID(),
    client: overrides.client || 'claude',
    directory: overrides.directory || '/tmp/project',
    startedAt: overrides.startedAt,
    lastActiveAt: overrides.lastActiveAt || new Date(),
    pid: overrides.pid,
  };
}

function processCandidate(overrides: Partial<ClaudeProcessInfo> = {}): ClaudeProcessInfo {
  return {
    client: overrides.client || 'claude',
    pid: overrides.pid || 1000,
    cwd: overrides.cwd || '/tmp/project',
    tty: overrides.tty,
    cpu: overrides.cpu || 0.1,
    memory: overrides.memory || 10,
    startTime: overrides.startTime || new Date(),
    args: overrides.args || ['claude'],
  };
}

describe('matchProcessesToSessions', () => {
  test('preserves PID continuity when known', () => {
    const sessions = [
      sessionCandidate({ sessionId: 'session-a', pid: 1001 }),
      sessionCandidate({ sessionId: 'session-b', pid: 1002 }),
    ];
    const processes = [
      processCandidate({ pid: 1002 }),
      processCandidate({ pid: 1001 }),
    ];

    const matches = matchProcessesToSessions(sessions, processes);
    expect(matches.get('session-a')?.pid).toBe(1001);
    expect(matches.get('session-b')?.pid).toBe(1002);
  });

  test('does not preserve a reused PID when process start time is incompatible', () => {
    const base = Date.now();
    const sessions = [
      sessionCandidate({
        sessionId: 'stale-known-pid',
        pid: 1001,
        startedAt: new Date(base - 4 * 60 * 60 * 1000),
        lastActiveAt: new Date(base - 3 * 60 * 60 * 1000),
      }),
    ];
    const processes = [
      processCandidate({
        pid: 1001,
        startTime: new Date(base),
      }),
    ];

    const matches = matchProcessesToSessions(sessions, processes);
    expect(matches.has('stale-known-pid')).toBe(false);
  });

  test('matches same-directory sessions by closest start time', () => {
    const base = Date.now();
    const sessions = [
      sessionCandidate({
        sessionId: 'session-early',
        startedAt: new Date(base + 1_000),
        lastActiveAt: new Date(base + 3_000),
      }),
      sessionCandidate({
        sessionId: 'session-late',
        startedAt: new Date(base + 10_000),
        lastActiveAt: new Date(base + 12_000),
      }),
    ];
    const processes = [
      processCandidate({ pid: 2001, startTime: new Date(base + 2_000) }),
      processCandidate({ pid: 2002, startTime: new Date(base + 11_000) }),
    ];

    const matches = matchProcessesToSessions(sessions, processes);
    expect(matches.get('session-early')?.pid).toBe(2001);
    expect(matches.get('session-late')?.pid).toBe(2002);
  });

  test('prefers the most plausible recent session over stale history in the same directory', () => {
    const now = Date.now();
    const sessions = [
      sessionCandidate({
        sessionId: 'stale-session',
        startedAt: new Date(now - 7 * 24 * 60 * 60 * 1000),
        lastActiveAt: new Date(now - 7 * 24 * 60 * 60 * 1000),
      }),
      sessionCandidate({
        sessionId: 'recent-session',
        startedAt: new Date(now - 10_000),
        lastActiveAt: new Date(now - 5_000),
      }),
    ];
    const processes = [
      processCandidate({ pid: 3001, startTime: new Date(now - 9_000) }),
    ];

    const matches = matchProcessesToSessions(sessions, processes);
    expect(matches.get('recent-session')?.pid).toBe(3001);
    expect(matches.has('stale-session')).toBe(false);
  });

  test('does not match sessions to processes from a different agent client', () => {
    const sessions = [
      sessionCandidate({ sessionId: 'claude-session', client: 'claude' }),
      sessionCandidate({ sessionId: 'codex-session', client: 'codex' }),
    ];
    const processes = [
      processCandidate({ pid: 4001, client: 'codex' }),
    ];

    const matches = matchProcessesToSessions(sessions, processes);
    expect(matches.has('claude-session')).toBe(false);
    expect(matches.get('codex-session')?.pid).toBe(4001);
  });
});
