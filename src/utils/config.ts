/**
 * Configuration management for Tasker
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { TASKER_HOME } from './paths.js';
import { ConfigError } from '../core/errors.js';

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
}

const defaultConfig: TaskerConfig = {
  scanInterval: 5000,
  hookPort: 7890,
  logLevel: 'info',
  fileLogging: true,
  autoDaemon: false,
  retentionDays: 30,
};

const CONFIG_FILE = join(TASKER_HOME, 'config.json');

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
      } catch (e) {
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
