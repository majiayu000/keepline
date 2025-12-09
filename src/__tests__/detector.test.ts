/**
 * Tests for process detector
 */

import { describe, test, expect } from 'bun:test';
import {
  detectSessionStatus,
  isSessionCompleted,
  needsAttention,
  getStatusDescription,
} from '../process/detector.js';
import type { ClaudeProcessInfo } from '../process/types.js';

// Helper to create a mock process
function createMockProcess(overrides: Partial<ClaudeProcessInfo> = {}): ClaudeProcessInfo {
  return {
    pid: 12345,
    cwd: '/test/path',
    cpu: 0.5,
    memory: 1.0,
    startTime: new Date(),
    args: [],
    ...overrides,
  };
}

describe('detectSessionStatus', () => {
  test('returns "lost" when process is null', () => {
    const status = detectSessionStatus(null);
    expect(status).toBe('lost');
  });

  test('returns "running" when CPU is above threshold', () => {
    const process = createMockProcess({ cpu: 5.0 });
    const status = detectSessionStatus(process);
    expect(status).toBe('running');
  });

  test('returns "running" for very recent activity', () => {
    const process = createMockProcess({ cpu: 0.1 });
    const recentActivity = new Date(Date.now() - 2000); // 2 seconds ago
    const status = detectSessionStatus(process, recentActivity);
    expect(status).toBe('running');
  });

  test('returns "waiting" for moderate recent activity', () => {
    const process = createMockProcess({ cpu: 0.1 });
    const moderateActivity = new Date(Date.now() - 15000); // 15 seconds ago
    const status = detectSessionStatus(process, moderateActivity);
    expect(status).toBe('waiting');
  });

  test('returns "idle" for old activity', () => {
    const process = createMockProcess({ cpu: 0.1 });
    const oldActivity = new Date(Date.now() - 60000); // 60 seconds ago
    const status = detectSessionStatus(process, oldActivity);
    expect(status).toBe('idle');
  });

  test('returns "idle" when no activity date provided and CPU is low', () => {
    const process = createMockProcess({ cpu: 0.1 });
    const status = detectSessionStatus(process);
    expect(status).toBe('idle');
  });
});

describe('isSessionCompleted', () => {
  test('returns false when process is running', () => {
    const process = createMockProcess();
    const result = isSessionCompleted(process, 'Write');
    expect(result).toBe(false);
  });

  test('returns true when no process and last tool is completion tool', () => {
    const result = isSessionCompleted(null, 'Write');
    expect(result).toBe(true);
  });

  test('returns true for TodoWrite completion tool', () => {
    const result = isSessionCompleted(null, 'TodoWrite');
    expect(result).toBe(true);
  });

  test('returns false when no process and last tool is not completion tool', () => {
    const result = isSessionCompleted(null, 'Read');
    expect(result).toBe(false);
  });

  test('returns false when no process and no last tool', () => {
    const result = isSessionCompleted(null);
    expect(result).toBe(false);
  });
});

describe('needsAttention', () => {
  test('returns true for "lost" status', () => {
    expect(needsAttention('lost')).toBe(true);
  });

  test('returns true for "waiting" status', () => {
    expect(needsAttention('waiting')).toBe(true);
  });

  test('returns false for "running" status', () => {
    expect(needsAttention('running')).toBe(false);
  });

  test('returns false for "idle" status', () => {
    expect(needsAttention('idle')).toBe(false);
  });

  test('returns false for "completed" status', () => {
    expect(needsAttention('completed')).toBe(false);
  });
});

describe('getStatusDescription', () => {
  test('returns correct description for all statuses', () => {
    expect(getStatusDescription('running')).toBe('Actively processing');
    expect(getStatusDescription('waiting')).toBe('Waiting for user input');
    expect(getStatusDescription('idle')).toBe('Idle but running');
    expect(getStatusDescription('lost')).toBe('Process terminated unexpectedly');
    expect(getStatusDescription('completed')).toBe('Session completed');
  });
});
