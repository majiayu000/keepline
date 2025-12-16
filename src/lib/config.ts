/**
 * Configuration management for Tasker
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { TASKER_HOME } from './paths.js';
import { ConfigError } from './errors.js';

export interface TaskerConfig {
  /** Scan interval in milliseconds */
  scanInterval: number;

  /** Hook server port */
  hookPort: number;

  /** Log level */
  logLevel: 'debug' | 'info' | 'warn' | 'error';

  /** Enable file logging */
  fileLogging: boolean;

  /** Auto-start daemon on CLI usage */
  autoDaemon: boolean;

  /** Session retention days (0 = forever) */
  retentionDays: number;

  /** CPU threshold for considering a process as active (percentage) */
  activeCpuThreshold: number;

  /** Time threshold for considering a session as idle (seconds) */
  idleThresholdSeconds: number;

  /** Time threshold for considering activity as "running" vs "waiting" (seconds) */
  runningThresholdSeconds: number;

  /** Process cache TTL in milliseconds */
  processCacheTtl: number;
}

const defaultConfig: TaskerConfig = {
  scanInterval: 5000,
  hookPort: 7890,
  logLevel: 'info',
  fileLogging: true,
  autoDaemon: false,
  retentionDays: 30,
  activeCpuThreshold: 1.0,
  idleThresholdSeconds: 30,
  runningThresholdSeconds: 5,
  processCacheTtl: 3000,
};

const CONFIG_FILE = join(TASKER_HOME, 'config.json');

/** Validate config values are within acceptable ranges */
function validateConfig(cfg: TaskerConfig): void {
  const errors: string[] = [];

  if (cfg.scanInterval < 100) {
    errors.push('scanInterval must be at least 100ms');
  }
  if (cfg.hookPort < 1 || cfg.hookPort > 65535) {
    errors.push('hookPort must be between 1 and 65535');
  }
  if (!['debug', 'info', 'warn', 'error'].includes(cfg.logLevel)) {
    errors.push('logLevel must be one of: debug, info, warn, error');
  }
  if (cfg.retentionDays < 0) {
    errors.push('retentionDays must be non-negative');
  }
  if (cfg.activeCpuThreshold < 0 || cfg.activeCpuThreshold > 100) {
    errors.push('activeCpuThreshold must be between 0 and 100');
  }
  if (cfg.idleThresholdSeconds < 1) {
    errors.push('idleThresholdSeconds must be at least 1');
  }
  if (cfg.runningThresholdSeconds < 1) {
    errors.push('runningThresholdSeconds must be at least 1');
  }
  if (cfg.processCacheTtl < 100) {
    errors.push('processCacheTtl must be at least 100ms');
  }

  if (errors.length > 0) {
    throw new ConfigError(`Invalid config: ${errors.join('; ')}`);
  }
}

class ConfigManager {
  private config: TaskerConfig = defaultConfig;
  private loaded = false;

  load(): TaskerConfig {
    if (this.loaded) return this.config;

    if (!existsSync(TASKER_HOME)) {
      mkdirSync(TASKER_HOME, { recursive: true });
    }

    if (existsSync(CONFIG_FILE)) {
      try {
        const content = readFileSync(CONFIG_FILE, 'utf-8');
        const parsed = JSON.parse(content) as Partial<TaskerConfig>;
        this.config = { ...defaultConfig, ...parsed };

        // Validate loaded config
        validateConfig(this.config);
      } catch (e) {
        if (e instanceof ConfigError) {
          throw e;
        }
        throw new ConfigError(`Failed to parse config: ${(e as Error).message}`);
      }
    } else {
      this.save();
    }

    this.loaded = true;
    return this.config;
  }

  save(): void {
    if (!existsSync(TASKER_HOME)) {
      mkdirSync(TASKER_HOME, { recursive: true });
    }
    writeFileSync(CONFIG_FILE, JSON.stringify(this.config, null, 2));
  }

  get(): TaskerConfig {
    return this.load();
  }

  set<K extends keyof TaskerConfig>(key: K, value: TaskerConfig[K]): void {
    this.load();
    this.config[key] = value;
    this.save();
  }

  reset(): void {
    this.config = { ...defaultConfig };
    this.save();
  }
}

export const config = new ConfigManager();
