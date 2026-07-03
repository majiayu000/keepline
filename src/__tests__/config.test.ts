/**
 * Integration tests for configuration management
 *
 * These tests verify the config module behavior using real data.
 * Following best practices:
 * - Test features, not implementation details
 * - Use real objects, no mocks
 * - Test observable behavior
 */

import { describe, test, expect } from 'bun:test';

// ============================================================================
// Feature: Configuration Module API
// ============================================================================

describe('Configuration Module API', () => {
  test('exports config manager with get, set, reset methods', async () => {
    const { config } = await import('../lib/config.js');

    expect(config).toBeDefined();
    expect(typeof config.get).toBe('function');
    expect(typeof config.set).toBe('function');
    expect(typeof config.reset).toBe('function');
  });

  test('config.get() returns complete configuration object', async () => {
    const { config } = await import('../lib/config.js');
    const cfg = config.get();

    // All expected properties should exist
    const expectedProperties = [
      'scanInterval',
      'hookPort',
      'webPort',
      'logLevel',
      'fileLogging',
      'autoDaemon',
      'retentionDays',
      'activeCpuThreshold',
      'idleThresholdSeconds',
      'runningThresholdSeconds',
      'processCacheTtl',
      'sessionDigest',
    ];

    for (const prop of expectedProperties) {
      expect(cfg).toHaveProperty(prop);
    }
  });
});

// ============================================================================
// Feature: Default Configuration Values
// ============================================================================

describe('Default Configuration Values', () => {
  describe('timing defaults', () => {
    test('scanInterval defaults to 5000ms (5 seconds)', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().scanInterval).toBe(5000);
    });

    test('processCacheTtl defaults to 3000ms (3 seconds)', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().processCacheTtl).toBe(3000);
    });

    test('idleThresholdSeconds defaults to 30', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().idleThresholdSeconds).toBe(30);
    });

    test('runningThresholdSeconds defaults to 5', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().runningThresholdSeconds).toBe(5);
    });
  });

  describe('server defaults', () => {
    test('hookPort defaults to 7890', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().hookPort).toBe(7890);
    });

    test('webPort defaults to 3377', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().webPort).toBe(3377);
    });

    test('web CLI port override takes precedence over config default', async () => {
      const { resolveWebPort } = await import('../cli/web.js');
      expect(resolveWebPort('4455')).toBe(4455);
    });

    test('web CLI port rejects invalid values', async () => {
      const { resolveWebPort } = await import('../cli/web.js');
      expect(() => resolveWebPort('abc')).toThrow('Invalid port number');
      expect(() => resolveWebPort('70000')).toThrow('Invalid port number');
      expect(() => resolveWebPort('123abc')).toThrow('Invalid port number');
    });

    test('config port validation rejects coerced or fractional values', async () => {
      const { config, isValidPortNumber, validateConfig } = await import('../lib/config.js');
      const cfg = config.get();

      expect(isValidPortNumber(3377)).toBe(true);
      expect(isValidPortNumber('3377')).toBe(false);
      expect(isValidPortNumber(3377.5)).toBe(false);
      expect(isValidPortNumber(0)).toBe(false);

      expect(() =>
        validateConfig({ ...cfg, webPort: '3377' as unknown as number })
      ).toThrow('webPort must be between 1 and 65535');
      expect(() =>
        validateConfig({ ...cfg, hookPort: 7890.5 })
      ).toThrow('hookPort must be between 1 and 65535');
    });
  });

  describe('logging defaults', () => {
    test('logLevel defaults to "info"', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().logLevel).toBe('info');
    });

    test('logLevel is a valid level', async () => {
      const { config } = await import('../lib/config.js');
      const validLevels = ['debug', 'info', 'warn', 'error'];
      expect(validLevels).toContain(config.get().logLevel);
    });
  });

  describe('detection thresholds', () => {
    test('activeCpuThreshold defaults to 1.0%', async () => {
      const { config } = await import('../lib/config.js');
      expect(config.get().activeCpuThreshold).toBe(1.0);
    });
  });

  describe('session digest defaults', () => {
    test('local summarizer is disabled by default', async () => {
      const { config } = await import('../lib/config.js');
      const summarizer = config.get().sessionDigest.summarizer;

      expect(summarizer).toMatchObject({
        provider: 'disabled',
        baseUrl: 'http://127.0.0.1:11434/v1',
        model: '',
        timeoutMs: 30000,
        maxInputChars: 12000,
        maxOutputTokens: 800,
      });
    });
  });
});

// ============================================================================
// Feature: Configuration Type Safety
// ============================================================================

describe('Configuration Type Safety', () => {
  test('numeric configs are numbers', async () => {
    const { config } = await import('../lib/config.js');
    const cfg = config.get();

    expect(typeof cfg.scanInterval).toBe('number');
    expect(typeof cfg.hookPort).toBe('number');
    expect(typeof cfg.webPort).toBe('number');
    expect(typeof cfg.retentionDays).toBe('number');
    expect(typeof cfg.activeCpuThreshold).toBe('number');
    expect(typeof cfg.idleThresholdSeconds).toBe('number');
    expect(typeof cfg.runningThresholdSeconds).toBe('number');
    expect(typeof cfg.processCacheTtl).toBe('number');
  });

  test('boolean configs are booleans', async () => {
    const { config } = await import('../lib/config.js');
    const cfg = config.get();

    expect(typeof cfg.fileLogging).toBe('boolean');
    expect(typeof cfg.autoDaemon).toBe('boolean');
  });

  test('string configs are strings', async () => {
    const { config } = await import('../lib/config.js');
    const cfg = config.get();

    expect(typeof cfg.logLevel).toBe('string');
  });
});
