/**
 * Tests for configuration management
 */

import { describe, test, expect, beforeEach, afterEach } from 'bun:test';
import { existsSync, mkdirSync, rmSync, writeFileSync, readFileSync } from 'fs';
import { join } from 'path';

// We need to test the config module, but it has side effects (file I/O)
// So we'll test the default values and structure

describe('TaskerConfig interface', () => {
  // Test that default config has all expected properties
  const expectedProperties = [
    'scanInterval',
    'hookPort',
    'logLevel',
    'fileLogging',
    'autoDaemon',
    'retentionDays',
    'activeCpuThreshold',
    'idleThresholdSeconds',
    'runningThresholdSeconds',
    'processCacheTtl',
  ];

  test('config module exports config manager', async () => {
    const { config } = await import('../utils/config.js');
    expect(config).toBeDefined();
    expect(typeof config.get).toBe('function');
    expect(typeof config.set).toBe('function');
    expect(typeof config.reset).toBe('function');
  });

  test('config.get() returns object with all expected properties', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();

    for (const prop of expectedProperties) {
      expect(cfg).toHaveProperty(prop);
    }
  });

  test('default scanInterval is 5000ms', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.scanInterval).toBe(5000);
  });

  test('default hookPort is 7890', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.hookPort).toBe(7890);
  });

  test('default logLevel is "info"', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.logLevel).toBe('info');
  });

  test('default activeCpuThreshold is 1.0', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.activeCpuThreshold).toBe(1.0);
  });

  test('default idleThresholdSeconds is 30', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.idleThresholdSeconds).toBe(30);
  });

  test('default runningThresholdSeconds is 5', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.runningThresholdSeconds).toBe(5);
  });

  test('default processCacheTtl is 3000ms', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    expect(cfg.processCacheTtl).toBe(3000);
  });

  test('logLevel accepts valid values', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();
    const validLevels = ['debug', 'info', 'warn', 'error'];
    expect(validLevels).toContain(cfg.logLevel);
  });
});

describe('config value types', () => {
  test('numeric configs are numbers', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();

    expect(typeof cfg.scanInterval).toBe('number');
    expect(typeof cfg.hookPort).toBe('number');
    expect(typeof cfg.retentionDays).toBe('number');
    expect(typeof cfg.activeCpuThreshold).toBe('number');
    expect(typeof cfg.idleThresholdSeconds).toBe('number');
    expect(typeof cfg.runningThresholdSeconds).toBe('number');
    expect(typeof cfg.processCacheTtl).toBe('number');
  });

  test('boolean configs are booleans', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();

    expect(typeof cfg.fileLogging).toBe('boolean');
    expect(typeof cfg.autoDaemon).toBe('boolean');
  });

  test('string configs are strings', async () => {
    const { config } = await import('../utils/config.js');
    const cfg = config.get();

    expect(typeof cfg.logLevel).toBe('string');
  });
});
