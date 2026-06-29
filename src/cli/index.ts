/**
 * Commands module - register all CLI commands
 */

import { Command } from 'commander';
import { listCommand } from './list.js'; // Note: actually list.tsx
import { watchCommand } from './watch.js';
import { recoverCommand, recoverListCommand } from './recover.js';
import { daemonCommand } from './daemon.js';
import { statusCommand } from './status.js';
import { webCommand } from './web.js';
import { overviewCommand } from './overview.js';
import {
  memoryListCommand,
  memoryShowCommand,
  memoryEditCommand,
  memoryDeleteCommand,
  memoryExportCommand,
} from './memory.js';

export function registerCommands(program: Command): void {
  // List sessions
  program
    .command('list')
    .alias('ls')
    .description('List all agent runtime sessions')
    .option('-s, --status <status>', 'Filter by status (running,waiting,idle,lost,completed)')
    .option('-d, --directory <dir>', 'Filter by directory')
    .option('-l, --limit <n>', 'Limit results')
    .option('-a, --all', 'Include completed sessions')
    .option('--style <style>', 'UI style: cyber (default), minimal, dashboard, neon, macos')
    .action(listCommand);

  // Watch sessions (live)
  program
    .command('watch')
    .alias('w')
    .description('Live monitor agent runtime sessions')
    .option('-i, --interval <seconds>', 'Refresh interval in seconds')
    .action(watchCommand);

  // Recover session
  program
    .command('recover [session]')
    .alias('r')
    .description('Recover a lost session')
    .option('-m, --method <method>', 'Recovery method (resume, continue, new)')
    .option('-t, --terminal', 'Open in new terminal window')
    .option('-a, --terminal-app <app>', 'Terminal app to use (Terminal, iTerm, Warp, auto)')
    .option('--skip-permissions', 'Use the owning CLI unsafe permission bypass flag')
    .action(async (session, options) => {
      if (!session) {
        await recoverListCommand();
      } else {
        await recoverCommand(session, options);
      }
    });

  // Daemon management
  program
    .command('daemon <action>')
    .alias('d')
    .description('Manage background daemon (start, stop, restart, status)')
    .option('--hooks', 'Install/uninstall Claude-compatible hooks with daemon')
    .action(daemonCommand);

  // Status
  program
    .command('status')
    .description('Show system status')
    .action(statusCommand);

  // Orchestrator overview
  program
    .command('overview')
    .description('Show the global agent attention queue')
    .option('-a, --all', 'Include completed sessions')
    .option('-l, --limit <n>', 'Limit results (default: 20)')
    .option('--json', 'Output JSON')
    .option('--high-cost-threshold <amount>', 'Cost threshold for high-cost reason')
    .option('--stale-hours <hours>', 'Hours without activity before stale reason')
    .action(overviewCommand);

  // Hooks management
  program
    .command('hooks <action>')
    .description('Manage Claude-compatible hooks (install, uninstall, status)')
    .action(async (action: string) => {
      const { installHooks, uninstallHooks, getHookStatus } = await import('../adapters/hook/installer.js');
      const chalk = (await import('chalk')).default;

      switch (action) {
        case 'install':
          installHooks();
          console.log(chalk.green('Hooks installed successfully'));
          break;
        case 'uninstall':
          uninstallHooks();
          console.log(chalk.green('Hooks uninstalled successfully'));
          break;
        case 'status': {
          const status = getHookStatus();
          console.log(`Hooks installed: ${status.installed ? chalk.green('Yes') : chalk.red('No')}`);
          console.log(`Settings path: ${status.settingsPath}`);
          break;
        }
        default:
          console.log(chalk.red(`Unknown action: ${action}`));
          console.log('Available actions: install, uninstall, status');
      }
    });

  // Sync (manual trigger)
  program
    .command('sync')
    .description('Manually sync agent runtime sessions')
    .action(async () => {
      const { runMigrations } = await import('../db/migrations.js');
      const { syncSessions } = await import('../services/session.service.js');
      const chalk = (await import('chalk')).default;

      runMigrations();
      console.log(chalk.gray('Syncing sessions...'));
      const result = await syncSessions();
      console.log(chalk.green(`Done: ${result.discovered} new, ${result.updated} updated, ${result.lost} lost`));
    });

  // Web UI
  program
    .command('web')
    .description('Start the web UI server')
    .option('-p, --port <port>', 'Port to listen on (default: 3377)')
    .action(webCommand);

  // Memory management (relay race pattern)
  const memoryCmd = program
    .command('memory')
    .alias('m')
    .description('Manage session memory (relay race pattern)');

  memoryCmd
    .command('list')
    .alias('ls')
    .description('List all session memories')
    .option('-d, --directory <dir>', 'Filter by directory')
    .option('-l, --limit <n>', 'Limit results (default: 10)')
    .action(memoryListCommand);

  memoryCmd
    .command('show <session>')
    .description('Show memory details for a session')
    .option('-v, --verbose', 'Show all details')
    .option('-c, --context', 'Show recovery context')
    .action(memoryShowCommand);

  memoryCmd
    .command('edit <session>')
    .description('Edit session memory')
    .option('-p, --progress <text>', 'Set last progress')
    .option('-t, --add-task <task>', 'Add a pending task')
    .option('-T, --complete-task <index|text>', 'Complete a task (by index or text match)')
    .option('-i, --add-issue <issue>', 'Add a known issue')
    .option('-I, --resolve-issue <index|text>', 'Resolve an issue')
    .option('-d, --decision <text>', 'Add a decision')
    .option('-H, --handoff <notes>', 'Set handoff notes')
    .option('-P, --priority <item>', 'Add a priority item for handoff')
    .option('-n, --notes <text>', 'Set general notes')
    .option('--clear', 'Clear all memory fields')
    .action(memoryEditCommand);

  memoryCmd
    .command('delete <session>')
    .description('Delete session memory')
    .option('-f, --force', 'Confirm deletion')
    .action(memoryDeleteCommand);

  memoryCmd
    .command('export <session>')
    .description('Export memory as recovery context')
    .option('-o, --output <file>', 'Output file (default: stdout)')
    .action(memoryExportCommand);
}
