#!/usr/bin/env bun
/**
 * Codex Hub - The command center for agent coding sessions
 *
 * Never lose your Codex or Claude Code work again.
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
  console: !process.env.CODEX_HUB_DAEMON && !process.env.CLAUDE_HUB_DAEMON, // Disable console in daemon mode
});

// Create CLI program
const program = new Command();

program
  .name('codex-hub')
  .version('1.0.0')
  .description('The command center for Codex and Claude Code sessions - Never lose your work again');

// Register all commands
registerCommands(program);

// Default action: show list
program.action(async () => {
  const { listCommand } = await import('./cli/list.js');
  await listCommand({});
});

// Parse arguments
program.parse();
