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

export function registerCommands(program: Command): void {
  // List sessions
  program
    .command('list')
    .alias('ls')
    .description('List all Claude Code sessions')
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
    .description('Live monitor Claude Code sessions')
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
    .option('--skip-permissions', 'Use --dangerously-skip-permissions')
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
    .option('--hooks', 'Install/uninstall Claude hooks with daemon')
    .action(daemonCommand);

  // Status
  program
    .command('status')
    .description('Show system status')
    .action(statusCommand);

  // Hooks management
  program
    .command('hooks <action>')
    .description('Manage Claude hooks (install, uninstall, status)')
    .action(async (action: string) => {
      const { installHooks, uninstallHooks, getHookStatus } = await import('../hook/installer.js');
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
    .description('Manually sync sessions with Claude data')
    .action(async () => {
      const { runMigrations } = await import('../storage/migrations.js');
      const { syncSessions } = await import('../session/service.js');
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
}
