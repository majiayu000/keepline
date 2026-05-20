/**
 * Status command - show system status
 */

import chalk from 'chalk';
import { existsSync } from 'fs';
import {
  CLAUDE_HOME,
  CLAUDE_PROJECT_ROOTS,
  CLAUDE_HUB_HOME,
  CLAUDE_HUB_DB,
  LEGACY_TASKER_HOME,
} from '../lib/paths.js';
import { getDaemonStatus } from '../services/daemon.manager.js';
import { getHookStatus } from '../adapters/hook/installer.js';
import { config } from '../lib/config.js';
import { runMigrations } from '../db/migrations.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { scanClaudeProcesses } from '../adapters/process/scanner.js';

export async function statusCommand(): Promise<void> {
  console.log('');
  console.log(chalk.bold('Claude Hub System Status'));
  console.log('');

  // Paths
  console.log(chalk.cyan('Paths:'));
  console.log(`  Claude home:    ${CLAUDE_HOME} ${existsSync(CLAUDE_HOME) ? chalk.green('OK') : chalk.red('Not found')}`);
  for (const root of CLAUDE_PROJECT_ROOTS) {
    console.log(`  Claude projects: ${root} ${existsSync(root) ? chalk.green('OK') : chalk.gray('Not found')}`);
  }
  console.log(`  Claude Hub home: ${CLAUDE_HUB_HOME} ${existsSync(CLAUDE_HUB_HOME) ? chalk.green('OK') : chalk.gray('Will create')}`);
  console.log(`  Claude Hub DB:   ${CLAUDE_HUB_DB} ${existsSync(CLAUDE_HUB_DB) ? chalk.green('OK') : chalk.gray('Will create')}`);
  if (LEGACY_TASKER_HOME !== CLAUDE_HUB_HOME && existsSync(LEGACY_TASKER_HOME)) {
    console.log(`  Legacy Tasker home: ${LEGACY_TASKER_HOME} ${chalk.yellow('Pending migration')}`);
  }
  console.log('');

  // Daemon
  const daemon = getDaemonStatus();
  console.log(chalk.cyan('Daemon:'));
  if (daemon.running) {
    console.log(`  Status: ${chalk.green('Running')}`);
    console.log(`  PID:    ${daemon.pid}`);
    if (daemon.uptime) {
      const hours = Math.floor(daemon.uptime / 3600);
      const minutes = Math.floor((daemon.uptime % 3600) / 60);
      console.log(`  Uptime: ${hours}h ${minutes}m`);
    }
  } else {
    console.log(`  Status: ${chalk.yellow('Not running')}`);
  }
  console.log('');

  // Hooks
  const hooks = getHookStatus();
  console.log(chalk.cyan('Hooks:'));
  console.log(`  Installed: ${hooks.installed ? chalk.green('Yes') : chalk.yellow('No')}`);
  console.log('');

  // Configuration
  const cfg = config.get();
  console.log(chalk.cyan('Configuration:'));
  console.log(`  Scan interval:  ${cfg.scanInterval}ms`);
  console.log(`  Hook port:      ${cfg.hookPort}`);
  console.log(`  Log level:      ${cfg.logLevel}`);
  console.log('');

  // Database stats
  try {
    runMigrations();
    const allSessions = sessionRepository.findAll();
    const activeSessions = sessionRepository.findActive();

    console.log(chalk.cyan('Database:'));
    console.log(`  Total sessions:  ${allSessions.length}`);
    console.log(`  Active sessions: ${activeSessions.length}`);
  } catch {
    console.log(chalk.cyan('Database:'));
    console.log(`  ${chalk.gray('Not initialized')}`);
  }
  console.log('');

  // Running processes
  const processes = scanClaudeProcesses();
  console.log(chalk.cyan('Claude Processes:'));
  if (processes.length === 0) {
    console.log(`  ${chalk.gray('No Claude processes running')}`);
  } else {
    console.log(`  Found: ${chalk.green(processes.length)} running`);
    processes.slice(0, 5).forEach((p) => {
      console.log(`    PID ${p.pid}: ${p.cwd}`);
    });
    if (processes.length > 5) {
      console.log(chalk.gray(`    ... and ${processes.length - 5} more`));
    }
  }
  console.log('');
}
