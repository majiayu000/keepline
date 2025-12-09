/**
 * Tests for process scanner caching functionality
 */

import { describe, test, expect, beforeEach } from 'bun:test';
import {
  clearProcessCache,
  getCachedProcesses,
} from '../process/scanner.js';

describe('Process Scanner Cache', () => {
  beforeEach(() => {
    // Clear cache before each test
    clearProcessCache();
  });

  test('clearProcessCache clears the cache', () => {
    // First call populates cache
    getCachedProcesses();

    // Clear it
    clearProcessCache();

    // This test mainly verifies no errors are thrown
    expect(true).toBe(true);
  });

  test('getCachedProcesses returns an array', () => {
    const processes = getCachedProcesses();
    expect(Array.isArray(processes)).toBe(true);
  });

  test('getCachedProcesses returns same result on repeated calls (cache)', () => {
    const first = getCachedProcesses();
    const second = getCachedProcesses();

    // Both calls should return arrays
    expect(Array.isArray(first)).toBe(true);
    expect(Array.isArray(second)).toBe(true);

    // Length should be the same (cached)
    expect(first.length).toBe(second.length);
  });

  test('processes have expected shape when found', () => {
    const processes = getCachedProcesses();

    // If there are any claude processes, verify shape
    for (const proc of processes) {
      expect(proc).toHaveProperty('pid');
      expect(proc).toHaveProperty('cwd');
      expect(proc).toHaveProperty('cpu');
      expect(proc).toHaveProperty('memory');
      expect(proc).toHaveProperty('startTime');
      expect(proc).toHaveProperty('args');

      expect(typeof proc.pid).toBe('number');
      expect(typeof proc.cwd).toBe('string');
      expect(typeof proc.cpu).toBe('number');
      expect(typeof proc.memory).toBe('number');
      expect(proc.startTime).toBeInstanceOf(Date);
      expect(Array.isArray(proc.args)).toBe(true);
    }
  });
});

describe('isProcessRunning', () => {
  test('returns false for non-existent PID', async () => {
    const { isProcessRunning } = await import('../process/scanner.js');
    // Use an impossibly high PID that won't exist
    const result = isProcessRunning(999999999);
    expect(result).toBe(false);
  });

  test('returns true for current process PID', async () => {
    const { isProcessRunning } = await import('../process/scanner.js');
    // Current process should be running
    const result = isProcessRunning(process.pid);
    expect(result).toBe(true);
  });
});
