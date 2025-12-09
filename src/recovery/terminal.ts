/**
 * Terminal operations for recovery
 */

import { execSync, spawn } from 'child_process';
import { logger } from '../utils/logger.js';
import type { TerminalApp } from './types.js';

/** Detect available terminal app */
export function detectTerminalApp(): TerminalApp {
  try {
    // Check if iTerm is running
    const result = execSync(
      `osascript -e 'tell application "System Events" to (name of processes) contains "iTerm2"'`,
      { encoding: 'utf-8' }
    ).trim();

    if (result === 'true') {
      return 'iTerm';
    }
  } catch {
    // Ignore
  }

  return 'Terminal';
}

/** Open a new terminal window with command */
export function openTerminalWithCommand(
  command: string,
  directory: string,
  terminalApp: TerminalApp = 'auto'
): void {
  const app = terminalApp === 'auto' ? detectTerminalApp() : terminalApp;

  if (app === 'iTerm') {
    openITerm(command, directory);
  } else {
    openTerminalApp(command, directory);
  }
}

/** Open Terminal.app with command */
function openTerminalApp(command: string, directory: string): void {
  const script = `
    tell application "Terminal"
      activate
      do script "cd ${escapeForAppleScript(directory)} && ${escapeForAppleScript(command)}"
    end tell
  `;

  try {
    execSync(`osascript -e '${script}'`, { encoding: 'utf-8' });
    logger.debug('Opened Terminal.app with command');
  } catch (error) {
    logger.error('Failed to open Terminal.app', error);
    throw error;
  }
}

/** Open iTerm with command */
function openITerm(command: string, directory: string): void {
  const script = `
    tell application "iTerm"
      activate
      create window with default profile
      tell current session of current window
        write text "cd ${escapeForAppleScript(directory)} && ${escapeForAppleScript(command)}"
      end tell
    end tell
  `;

  try {
    execSync(`osascript -e '${script}'`, { encoding: 'utf-8' });
    logger.debug('Opened iTerm with command');
  } catch (error) {
    logger.error('Failed to open iTerm', error);
    throw error;
  }
}

/** Escape string for AppleScript */
function escapeForAppleScript(str: string): string {
  return str
    .replace(/\\/g, '\\\\')
    .replace(/"/g, '\\"')
    .replace(/'/g, "'\"'\"'");
}

/** Execute command in current terminal */
export function executeCommand(command: string, cwd: string): number | null {
  try {
    const child = spawn(command, [], {
      shell: true,
      cwd,
      stdio: 'inherit',
      detached: true,
    });

    child.unref();
    return child.pid ?? null;
  } catch (error) {
    logger.error('Failed to execute command', error);
    return null;
  }
}

/** Print command for user to copy */
export function printRecoveryCommand(command: string, directory: string): void {
  console.log('\nTo recover this session, run:\n');
  console.log(`  cd ${directory}`);
  console.log(`  ${command}`);
  console.log('');
}
