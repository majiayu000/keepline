/**
 * Watch command - live monitor sessions
 */

import chalk from 'chalk';
import ora from 'ora';
import { runMigrations } from '../db/migrations.js';
import { syncSessions } from '../services/session.service.js';
import { getAggregatedSessions, getSessionStats } from '../services/session.aggregator.js';
import { createSessionTable } from '../lib/format.js';
import { config } from '../lib/config.js';

interface WatchOptions {
  interval?: string;
}

export async function watchCommand(options: WatchOptions): Promise<void> {
  // Initialize database
  runMigrations();

  const interval = options.interval
    ? parseInt(options.interval, 10) * 1000
    : config.get().scanInterval;

  console.log(chalk.bold('Keepline - Live Session Monitor'));
  console.log(chalk.gray(`Refreshing every ${interval / 1000} seconds. Press Ctrl+C to stop.\n`));

  const refresh = async (): Promise<void> => {
    // Clear screen
    console.clear();
    console.log(chalk.bold('Keepline - Live Session Monitor'));
    console.log(chalk.gray(`Last update: ${new Date().toLocaleTimeString()}\n`));

    const spinner = ora('Scanning...').start();

    try {
      await syncSessions();
      spinner.stop();

      const sessions = getAggregatedSessions()
        .filter((s) => s.status !== 'completed')
        .sort((a, b) => b.lastActiveAt.getTime() - a.lastActiveAt.getTime());

      const stats = getSessionStats(sessions);

      console.log(
        chalk.gray('Sessions:'),
        chalk.green(`${stats.running} running`),
        chalk.yellow(`${stats.waiting} waiting`),
        chalk.blue(`${stats.idle} idle`),
        chalk.red(`${stats.lost} lost`)
      );
      console.log('');

      if (sessions.length === 0) {
        console.log(chalk.gray('No active sessions.'));
      } else {
        console.log(createSessionTable(sessions));
      }

      console.log('');
      console.log(chalk.gray('Press Ctrl+C to stop watching.'));
    } catch (error) {
      spinner.fail('Scan failed');
      console.error(chalk.red((error as Error).message));
    }
  };

  // Initial refresh
  await refresh();

  // Set up interval
  const intervalId = setInterval(refresh, interval);

  // Handle Ctrl+C
  process.on('SIGINT', () => {
    clearInterval(intervalId);
    console.log('\n' + chalk.gray('Stopped watching.'));
    process.exit(0);
  });
}
