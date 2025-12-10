/**
 * Process scanner for detecting Claude Code instances
 */

import { execSync } from 'child_process';
import { logger } from '../utils/logger.js';
import { ProcessScanError } from '../core/errors.js';
import type { ClaudeProcessInfo } from './types.js';

// Maximum valid PID (varies by OS, but 2^22 is common max)
const MAX_PID = 4194304;

/** Validate PID is a safe positive integer */
function validatePid(pid: number): boolean {
  return Number.isInteger(pid) && pid > 0 && pid <= MAX_PID;
}

/** Get working directory for a process using lsof */
function getProcessCwd(pid: number): string | undefined {
  if (!validatePid(pid)) {
    logger.warn('Invalid PID for getProcessCwd', { pid });
    return undefined;
  }

  try {
    const output = execSync(`lsof -p ${pid} 2>/dev/null | grep cwd`, {
      encoding: 'utf-8',
      timeout: 5000,
    });
    // Format: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
    const parts = output.trim().split(/\s+/);
    if (parts.length >= 9) {
      return parts.slice(8).join(' ');
    }
  } catch {
    // Process may have ended or lsof failed
  }
  return undefined;
}

/** Get command line args for a process */
function getProcessArgs(pid: number): string[] {
  if (!validatePid(pid)) {
    return [];
  }

  try {
    const output = execSync(`ps -p ${pid} -o args=`, {
      encoding: 'utf-8',
      timeout: 5000,
    });
    return output.trim().split(/\s+/).slice(1);
  } catch {
    return [];
  }
}

/** Get process start time */
function getProcessStartTime(pid: number): Date | undefined {
  if (!validatePid(pid)) {
    return undefined;
  }

  try {
    const output = execSync(`ps -p ${pid} -o lstart=`, {
      encoding: 'utf-8',
      timeout: 5000,
    });
    const timeStr = output.trim();
    if (timeStr) {
      return new Date(timeStr);
    }
  } catch {
    // Ignore
  }
  return undefined;
}

/** Scan for all Claude Code processes */
export function scanClaudeProcesses(): ClaudeProcessInfo[] {
  try {
    // Find all claude processes
    const output = execSync(
      `ps aux | grep -E '[c]laude' | grep -v grep`,
      { encoding: 'utf-8', timeout: 10000 }
    );

    const lines = output.trim().split('\n').filter(Boolean);
    const processes: ClaudeProcessInfo[] = [];

    for (const line of lines) {
      // Parse: USER PID %CPU %MEM VSZ RSS TTY STAT START TIME COMMAND
      const parts = line.trim().split(/\s+/);
      if (parts.length < 11) continue;

      const pid = parseInt(parts[1], 10);
      if (isNaN(pid)) continue;

      const cpu = parseFloat(parts[2]) || 0;
      const mem = parseFloat(parts[3]) || 0;
      const tty = parts[6];
      const command = parts.slice(10).join(' ');

      // Skip non-main claude processes (like shell wrappers)
      if (!command.includes('claude')) continue;
      // Skip if it's a background shell process
      if (command.includes('/bin/zsh') || command.includes('/bin/bash')) continue;

      const cwd = getProcessCwd(pid);
      if (!cwd) continue; // Skip if we can't get working directory

      const startTime = getProcessStartTime(pid);
      const args = getProcessArgs(pid);

      processes.push({
        pid,
        cwd,
        tty: tty !== '??' ? tty : undefined,
        cpu,
        memory: mem,
        startTime: startTime || new Date(),
        args,
      });
    }

    logger.debug(`Found ${processes.length} Claude processes`);
    return processes;
  } catch (error) {
    // No claude processes found (grep returns exit code 1)
    if ((error as { status?: number }).status === 1) {
      return [];
    }
    throw new ProcessScanError('Failed to scan processes', {
      error: (error as Error).message,
    });
  }
}

/** Check if a specific process is still running */
export function isProcessRunning(pid: number): boolean {
  if (!validatePid(pid)) {
    return false;
  }

  try {
    // Use process.kill with signal 0 to check if process exists (safer than execSync)
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** Get process info by PID */
export function getProcessInfo(pid: number): ClaudeProcessInfo | null {
  if (!validatePid(pid)) {
    return null;
  }

  if (!isProcessRunning(pid)) return null;

  const cwd = getProcessCwd(pid);
  if (!cwd) return null;

  try {
    const output = execSync(`ps -p ${pid} -o %cpu,%mem,tty=`, {
      encoding: 'utf-8',
      timeout: 5000,
    });
    const parts = output.trim().split(/\s+/);

    return {
      pid,
      cwd,
      cpu: parseFloat(parts[0]) || 0,
      memory: parseFloat(parts[1]) || 0,
      tty: parts[2] !== '??' ? parts[2] : undefined,
      startTime: getProcessStartTime(pid) || new Date(),
      args: getProcessArgs(pid),
    };
  } catch {
    return null;
  }
}

/** Stop a process gracefully (SIGTERM) or forcefully (SIGKILL) */
export function stopProcess(pid: number, force: boolean = false): { success: boolean; error?: string } {
  if (!validatePid(pid)) {
    return { success: false, error: 'Invalid PID' };
  }

  if (!isProcessRunning(pid)) {
    return { success: false, error: 'Process not running' };
  }

  try {
    const signal = force ? 'SIGKILL' : 'SIGTERM';
    process.kill(pid, signal);
    logger.info(`Sent ${signal} to process ${pid}`);

    // If graceful stop, set up a timeout to force kill if still running
    if (!force) {
      setTimeout(() => {
        if (isProcessRunning(pid)) {
          logger.warn(`Process ${pid} still running after SIGTERM, sending SIGKILL`);
          try {
            process.kill(pid, 'SIGKILL');
          } catch {
            // Process may have died in the meantime
          }
        }
      }, 5000);
    }

    return { success: true };
  } catch (error) {
    const err = error as NodeJS.ErrnoException;
    if (err.code === 'ESRCH') {
      return { success: false, error: 'Process not found' };
    }
    if (err.code === 'EPERM') {
      return { success: false, error: 'Permission denied' };
    }
    return { success: false, error: err.message };
  }
}
