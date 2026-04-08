/**
 * Daemon process manager
 */

import { spawn } from 'child_process';
import { existsSync, readFileSync, writeFileSync, unlinkSync } from 'fs';
import { CLAUDE_HUB_PID } from '../lib/paths.js';
import { logger } from '../lib/logger.js';
import { emit } from '../lib/events.js';

// Maximum valid PID
const MAX_PID = 4194304;

/** Validate PID is a safe positive integer */
function validatePid(pid: number): boolean {
  return Number.isInteger(pid) && pid > 0 && pid <= MAX_PID;
}

/** Check if process is running using process.kill(pid, 0) */
function isProcessAlive(pid: number): boolean {
  if (!validatePid(pid)) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** Sleep for ms milliseconds */
function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/** Check if daemon is running */
export function isDaemonRunning(): boolean {
  if (!existsSync(CLAUDE_HUB_PID)) {
    return false;
  }

  try {
    const pid = parseInt(readFileSync(CLAUDE_HUB_PID, 'utf-8').trim(), 10);
    if (!validatePid(pid)) {
      cleanupPidFile();
      return false;
    }
    // Check if process exists using process.kill(pid, 0)
    if (isProcessAlive(pid)) {
      return true;
    }
    cleanupPidFile();
    return false;
  } catch {
    // Process not running, clean up stale PID file
    cleanupPidFile();
    return false;
  }
}

/** Get daemon PID if running */
export function getDaemonPid(): number | null {
  if (!existsSync(CLAUDE_HUB_PID)) {
    return null;
  }

  try {
    const pid = parseInt(readFileSync(CLAUDE_HUB_PID, 'utf-8').trim(), 10);
    if (!validatePid(pid)) {
      cleanupPidFile();
      return null;
    }
    // Verify process is running using process.kill(pid, 0)
    if (isProcessAlive(pid)) {
      return pid;
    }
    cleanupPidFile();
    return null;
  } catch {
    cleanupPidFile();
    return null;
  }
}

/** Write PID file */
function writePidFile(pid: number): void {
  writeFileSync(CLAUDE_HUB_PID, String(pid));
}

/** Clean up PID file */
function cleanupPidFile(): void {
  if (existsSync(CLAUDE_HUB_PID)) {
    unlinkSync(CLAUDE_HUB_PID);
  }
}

/** Start daemon process */
export function startDaemon(): { success: boolean; pid?: number; error?: string } {
  if (isDaemonRunning()) {
    const pid = getDaemonPid();
    return { success: true, pid: pid || undefined };
  }

  try {
    // Spawn detached daemon process
    const child = spawn(process.execPath, [process.argv[1], 'daemon', 'run'], {
      detached: true,
      stdio: 'ignore',
      env: { ...process.env, CLAUDE_HUB_DAEMON: '1' },
    });

    child.unref();

    if (child.pid) {
      writePidFile(child.pid);
      emit('daemon:started', { pid: child.pid });
      logger.info(`Daemon started with PID ${child.pid}`);
      return { success: true, pid: child.pid };
    }

    return { success: false, error: 'Failed to get PID' };
  } catch (error) {
    const errorMessage = (error as Error).message;
    logger.error(`Failed to start daemon: ${errorMessage}`);
    return { success: false, error: errorMessage };
  }
}

/** Stop daemon process */
export async function stopDaemon(): Promise<{ success: boolean; error?: string }> {
  const pid = getDaemonPid();

  if (!pid) {
    return { success: true }; // Not running
  }

  try {
    process.kill(pid, 'SIGTERM');

    // Wait for process to exit (max 5 seconds)
    for (let i = 0; i < 50; i++) {
      if (!isProcessAlive(pid)) {
        // Process exited
        break;
      }
      // Still running, wait
      await sleep(100);
    }

    cleanupPidFile();
    emit('daemon:stopped', { reason: 'manual stop' });
    logger.info('Daemon stopped');
    return { success: true };
  } catch (error) {
    const errorMessage = (error as Error).message;
    logger.error(`Failed to stop daemon: ${errorMessage}`);
    return { success: false, error: errorMessage };
  }
}

/** Restart daemon */
export async function restartDaemon(): Promise<{ success: boolean; pid?: number; error?: string }> {
  const stopResult = await stopDaemon();
  if (!stopResult.success) {
    return stopResult;
  }

  // Wait a bit before starting
  await sleep(500);

  return startDaemon();
}

/** Get daemon status */
export function getDaemonStatus(): {
  running: boolean;
  pid?: number;
  uptime?: number;
} {
  const pid = getDaemonPid();

  if (!pid) {
    return { running: false };
  }

  // PID is already validated by getDaemonPid
  // Just return running status - uptime calculation removed to avoid execSync
  return { running: true, pid };
}
