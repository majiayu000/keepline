/**
 * Integration tests for process scanner caching functionality
 *
 * These tests verify real scanner behavior against real system processes.
 * Following best practices:
 * - Test features, not implementation details
 * - Use real objects, no mocks
 * - Test observable behavior
 */

import { describe, test, expect, beforeEach } from 'bun:test';
import {
  clearProcessCache,
  getCachedProcesses,
  isProcessRunning,
} from '../process/scanner.js';

// ============================================================================
// Feature: Process Cache Management
// ============================================================================

describe('Process Cache Management', () => {
  beforeEach(() => {
    // Clear cache before each test to ensure isolation
    clearProcessCache();
  });

  describe('cache clearing', () => {
    test('clearProcessCache completes without error', () => {
      // First call populates cache
      getCachedProcesses();

      // Clear should not throw
      expect(() => clearProcessCache()).not.toThrow();
    });
  });

  describe('cached process retrieval', () => {
    test('getCachedProcesses returns an array', () => {
      const processes = getCachedProcesses();
      expect(Array.isArray(processes)).toBe(true);
    });

    test('repeated calls return consistent results (cache hit)', () => {
      const first = getCachedProcesses();
      const second = getCachedProcesses();

      // Both calls should return arrays with same length
      expect(Array.isArray(first)).toBe(true);
      expect(Array.isArray(second)).toBe(true);
      expect(first.length).toBe(second.length);
    });
  });

  describe('process info structure', () => {
    test('each process has required properties with correct types', () => {
      const processes = getCachedProcesses();

      // If there are any claude processes, verify their structure
      for (const proc of processes) {
        // Required properties exist
        expect(proc).toHaveProperty('pid');
        expect(proc).toHaveProperty('cwd');
        expect(proc).toHaveProperty('cpu');
        expect(proc).toHaveProperty('memory');
        expect(proc).toHaveProperty('startTime');
        expect(proc).toHaveProperty('args');

        // Correct types
        expect(typeof proc.pid).toBe('number');
        expect(typeof proc.cwd).toBe('string');
        expect(typeof proc.cpu).toBe('number');
        expect(typeof proc.memory).toBe('number');
        expect(proc.startTime).toBeInstanceOf(Date);
        expect(Array.isArray(proc.args)).toBe(true);
      }
    });
  });
});

// ============================================================================
// Feature: Process Running Detection
// ============================================================================

describe('Process Running Detection', () => {
  describe('detecting non-existent processes', () => {
    test('returns false for impossibly high PID', () => {
      // Use an impossibly high PID that won't exist on any system
      const result = isProcessRunning(999999999);
      expect(result).toBe(false);
    });

    test('returns false for negative PID', () => {
      const result = isProcessRunning(-1);
      expect(result).toBe(false);
    });
  });

  describe('detecting running processes', () => {
    test('returns true for current process (self)', () => {
      // The current process should always be running
      const result = isProcessRunning(process.pid);
      expect(result).toBe(true);
    });
  });
});
