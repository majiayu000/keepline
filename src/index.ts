#!/usr/bin/env bun
/**
 * Keepline - The command center for agent coding sessions
 *
 * Never lose your local agent runtime work again.
 * Real-time monitoring, automatic recovery, cost tracking,
 * and cross-session memory for agent CLI power users.
 */

import { Command } from 'commander';
import { registerCommands } from './cli/index.js';
import { logger } from './lib/logger.js';
import { config } from './lib/config.js';

// Configure logger based on config
const cfg = config.get();
logger.configure({
  level: cfg.logLevel,
  file: cfg.fileLogging,
  console: !process.env.KEEPLINE_DAEMON, // Disable console in daemon mode
});

// Create CLI program
const program = new Command();

program
  .name('keepline')
  .version('1.0.0')
  .description('The command center for local agent runtime sessions - Never lose your work again');

// Register all commands
registerCommands(program);

// Default action: start the web dashboard.
program.action(async () => {
  const { webCommand } = await import('./cli/web.js');
  await webCommand({});
});

// Parse arguments, preserving async command lifetimes such as the web server.
await program.parseAsync();
