/**
 * Daemon process manager
 */

import { spawn, execSync } from 'child_process';
import { existsSync, readFileSync, writeFileSync, unlinkSync } from 'fs';
import { TASKER_PID } from '../utils/paths.js';
import { logger } from '../utils/logger.js';
import { emit } from '../core/events.js';

/** Check if daemon is running */
export function isDaemonRunning(): boolean {
  if (!existsSync(TASKER_PID)) {
    return false;
  }

  try {
    const pid = parseInt(readFileSync(TASKER_PID, 'utf-8').trim(), 10);
    // Check if process exists
    execSync(`ps -p ${pid} -o pid=`, { encoding: 'utf-8' });
    return true;
  } catch {
    // Process not running, clean up stale PID file
    cleanupPidFile();
    return false;
  }
}

/** Get daemon PID if running */
export function getDaemonPid(): number | null {
  if (!existsSync(TASKER_PID)) {
    return null;
  }

  try {
    const pid = parseInt(readFileSync(TASKER_PID, 'utf-8').trim(), 10);
    // Verify process is running
    execSync(`ps -p ${pid} -o pid=`, { encoding: 'utf-8' });
    return pid;
  } catch {
    cleanupPidFile();
    return null;
  }
}

/** Write PID file */
function writePidFile(pid: number): void {
  writeFileSync(TASKER_PID, String(pid));
}

/** Clean up PID file */
function cleanupPidFile(): void {
  if (existsSync(TASKER_PID)) {
    unlinkSync(TASKER_PID);
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
      env: { ...process.env, TASKER_DAEMON: '1' },
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
export function stopDaemon(): { success: boolean; error?: string } {
  const pid = getDaemonPid();

  if (!pid) {
    return { success: true }; // Not running
  }

  try {
    process.kill(pid, 'SIGTERM');

    // Wait for process to exit (max 5 seconds)
    for (let i = 0; i < 50; i++) {
      try {
        execSync(`ps -p ${pid} -o pid=`, { encoding: 'utf-8' });
        // Still running, wait
        execSync('sleep 0.1');
      } catch {
        // Process exited
        break;
      }
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
export function restartDaemon(): { success: boolean; pid?: number; error?: string } {
  const stopResult = stopDaemon();
  if (!stopResult.success) {
    return stopResult;
  }

  // Wait a bit before starting
  execSync('sleep 0.5');

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

  try {
    // Get process start time
    const output = execSync(`ps -p ${pid} -o etime=`, { encoding: 'utf-8' }).trim();

    // Parse etime format: [[DD-]hh:]mm:ss
    let uptime = 0;
    const parts = output.split(/[-:]/).reverse();
    uptime += parseInt(parts[0] || '0', 10);            // seconds
    uptime += parseInt(parts[1] || '0', 10) * 60;       // minutes
    uptime += parseInt(parts[2] || '0', 10) * 3600;     // hours
    uptime += parseInt(parts[3] || '0', 10) * 86400;    // days

    return { running: true, pid, uptime };
  } catch {
    return { running: true, pid };
  }
}
