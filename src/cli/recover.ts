/**
 * Recover command - restore lost sessions
 */

import chalk from 'chalk';
import { runMigrations } from '../db/migrations.js';
import { syncSessions } from '../services/session.service.js';
import { getAggregatedSessions } from '../services/session.aggregator.js';
import { recoverSession, getRecoveryInfo } from '../services/recovery.service.js';
import { formatSessionDetail } from '../lib/format.js';
import type { TerminalApp } from '../services/recovery.types.js';

interface RecoverOptions {
  method?: string;
  terminal?: boolean;
  terminalApp?: string;
  skipPermissions?: boolean;
}

export async function recoverCommand(
  sessionIndex: string,
  options: RecoverOptions
): Promise<void> {
  // Initialize database
  runMigrations();

  // Sync first
  console.log(chalk.gray('Syncing sessions...'));
  await syncSessions();

  // Get sessions
  const sessions = getAggregatedSessions()
    .filter((s) => s.status !== 'completed')
    .sort((a, b) => b.lastActiveAt.getTime() - a.lastActiveAt.getTime());

  // Parse session index
  const index = parseInt(sessionIndex, 10) - 1;

  if (isNaN(index) || index < 0 || index >= sessions.length) {
    console.log(chalk.red(`Invalid session number. Use 1-${sessions.length}`));
    process.exit(1);
  }

  const session = sessions[index];

  // Get recovery info
  const info = getRecoveryInfo(session.sessionId);

  if (!info.canRecover) {
    console.log(chalk.red(`Cannot recover: ${info.reason}`));
    process.exit(1);
  }

  // Show session details
  console.log('');
  console.log(formatSessionDetail(session));
  console.log('');

  // Show available methods
  console.log(chalk.bold('Recovery Options:'));
  console.log(chalk.gray(`  Available methods: ${info.availableMethods.join(', ')}`));
  console.log(chalk.gray(`  Recommended: ${info.recommendedMethod}`));
  console.log('');

  // Determine method to use
  const method = (options.method || info.recommendedMethod) as 'resume' | 'continue' | 'new';

  if (!info.availableMethods.includes(method)) {
    console.log(chalk.red(`Method '${method}' not available for this session.`));
    process.exit(1);
  }

  // Validate terminal app
  const validTerminalApps = ['Terminal', 'iTerm', 'Warp', 'auto'];
  const terminalApp = (options.terminalApp || 'auto') as TerminalApp;

  if (!validTerminalApps.includes(terminalApp)) {
    console.log(chalk.red(`Invalid terminal app '${options.terminalApp}'. Valid options: ${validTerminalApps.join(', ')}`));
    process.exit(1);
  }

  console.log(chalk.cyan(`Recovering with method: ${method}`));
  if (options.terminal) {
    console.log(chalk.gray(`Opening in: ${terminalApp === 'auto' ? 'auto-detected terminal' : terminalApp}`));
  }

  // Perform recovery
  const result = await recoverSession({
    method,
    sessionId: session.sessionId,
    directory: session.directory,
    openTerminal: options.terminal ?? false,
    skipPermissions: options.skipPermissions ?? false,
    terminalApp,
  });

  if (result.success) {
    console.log(chalk.green('\nRecovery initiated successfully!'));
    if (!options.terminal) {
      console.log(chalk.gray('Run the command above in your terminal to continue.'));
    }
  } else {
    console.log(chalk.red(`\nRecovery failed: ${result.error}`));
    process.exit(1);
  }
}

/** List recoverable sessions */
export async function recoverListCommand(): Promise<void> {
  runMigrations();
  await syncSessions();

  const sessions = getAggregatedSessions().filter((s) => s.status === 'lost');

  if (sessions.length === 0) {
    console.log(chalk.green('No lost sessions to recover.'));
    return;
  }

  console.log(chalk.bold('\nRecoverable Sessions:\n'));

  sessions.forEach((session, index) => {
    const info = getRecoveryInfo(session.sessionId);
    console.log(chalk.cyan(`[${index + 1}]`), session.directory);
    console.log(chalk.gray(`    Task: ${session.title || session.initialPrompt.slice(0, 60)}...`));
    console.log(chalk.gray(`    Methods: ${info.availableMethods.join(', ')}`));
    console.log('');
  });

  console.log(chalk.gray('Use: tasker recover <number> to recover a session'));
}
