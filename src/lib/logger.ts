/**
 * Logger utility for Keepline.
 */

import chalk from 'chalk';
import { appendFileSync, existsSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { KEEPLINE_LOG, ensureKeeplineDataHome } from './paths.js';

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

interface LogOptions {
  level: LogLevel;
  file: boolean;
  console: boolean;
}

const defaultOptions: LogOptions = {
  level: 'info',
  file: true,
  console: true,
};

const levelPriority: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

const levelColors: Record<LogLevel, (s: string) => string> = {
  debug: chalk.gray,
  info: chalk.blue,
  warn: chalk.yellow,
  error: chalk.red,
};

class Logger {
  private options: LogOptions = defaultOptions;

  configure(options: Partial<LogOptions>): void {
    this.options = { ...this.options, ...options };
  }

  private shouldLog(level: LogLevel): boolean {
    return levelPriority[level] >= levelPriority[this.options.level];
  }

  private formatMessage(level: LogLevel, message: string): string {
    const timestamp = new Date().toISOString();
    return `[${timestamp}] [${level.toUpperCase()}] ${message}`;
  }

  private writeToFile(message: string): void {
    if (!this.options.file) return;

    ensureKeeplineDataHome();
    const dir = dirname(KEEPLINE_LOG);
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
    appendFileSync(KEEPLINE_LOG, message + '\n');
  }

  private log(level: LogLevel, message: string, data?: unknown): void {
    if (!this.shouldLog(level)) return;

    const formattedMessage = this.formatMessage(level, message);
    const fullMessage = data
      ? `${formattedMessage} ${JSON.stringify(data)}`
      : formattedMessage;

    this.writeToFile(fullMessage);

    if (this.options.console) {
      const colorFn = levelColors[level];
      console.log(colorFn(fullMessage));
    }
  }

  debug(message: string, data?: unknown): void {
    this.log('debug', message, data);
  }

  info(message: string, data?: unknown): void {
    this.log('info', message, data);
  }

  warn(message: string, data?: unknown): void {
    this.log('warn', message, data);
  }

  error(message: string, data?: unknown): void {
    this.log('error', message, data);
  }
}

export const logger = new Logger();
