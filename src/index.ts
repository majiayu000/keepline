#!/usr/bin/env node
/**
 * Tasker - Claude Code Session Monitor and Recovery Tool
 *
 * Monitor multiple Claude Code instances, track their status,
 * and recover lost sessions when terminals crash.
 */

import { Command } from 'commander';
import { registerCommands } from './commands/index.js';
import { logger } from './utils/logger.js';
import { config } from './utils/config.js';

// Configure logger based on config
const cfg = config.get();
logger.configure({
  level: cfg.logLevel,
  file: cfg.fileLogging,
  console: !process.env.TASKER_DAEMON, // Disable console in daemon mode
});

// Create CLI program
const program = new Command();

program
  .name('tasker')
  .version('1.0.0')
  .description('Claude Code session monitor and recovery tool');

// Register all commands
registerCommands(program);

// Default action: show list
program.action(async () => {
  const { listCommand } = await import('./commands/list.js');
  await listCommand({});
});

// Parse arguments
program.parse();
