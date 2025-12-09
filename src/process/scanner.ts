/**
 * Process scanner for detecting Claude Code instances
 *
 * Optimized to minimize system calls:
 * - Single ps command with all needed fields
 * - Batch lsof for working directories
 * - Process list caching during sync cycles
 */

import { execSync } from 'child_process';
import { logger } from '../utils/logger.js';
import { config } from '../utils/config.js';
import { ProcessScanError } from '../core/errors.js';
import type { ClaudeProcessInfo } from './types.js';

// Cache for process list to avoid redundant scans within same sync cycle
let processCache: { processes: ClaudeProcessInfo[]; timestamp: number } | null = null;

/** Clear the process cache (call at start of sync cycle) */
export function clearProcessCache(): void {
  processCache = null;
}

/** Get cached process list or scan if cache expired */
export function getCachedProcesses(): ClaudeProcessInfo[] {
  const cacheTtl = config.get().processCacheTtl;
  if (processCache && Date.now() - processCache.timestamp < cacheTtl) {
    logger.debug('Using cached process list');
    return processCache.processes;
  }
  return scanClaudeProcesses();
}

/** Batch get working directories for multiple PIDs using single lsof call */
function batchGetProcessCwd(pids: number[]): Map<number, string> {
  const cwdMap = new Map<number, string>();
  if (pids.length === 0) return cwdMap;

  try {
    // Use lsof with multiple PIDs in one call: lsof -a -d cwd -p pid1,pid2,pid3
    const pidList = pids.join(',');
    const output = execSync(`lsof -a -d cwd -p ${pidList} 2>/dev/null`, {
      encoding: 'utf-8',
      timeout: 10000,
    });

    // Parse output: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
    for (const line of output.trim().split('\n')) {
      const parts = line.trim().split(/\s+/);
      if (parts.length >= 9 && parts[3] === 'cwd') {
        const pid = parseInt(parts[1], 10);
        const cwd = parts.slice(8).join(' ');
        if (!isNaN(pid) && cwd) {
          cwdMap.set(pid, cwd);
        }
      }
    }
  } catch {
    // lsof may fail if no processes match
    logger.debug('lsof batch cwd lookup failed or returned empty');
  }

  return cwdMap;
}

/** Parse lstart date string from ps output */
function parseLstartDate(lstartStr: string): Date | undefined {
  if (!lstartStr || lstartStr.trim() === '') return undefined;
  try {
    return new Date(lstartStr.trim());
  } catch {
    return undefined;
  }
}

/** Scan for all Claude Code processes (optimized: 2 system calls instead of N+3) */
export function scanClaudeProcesses(): ClaudeProcessInfo[] {
  try {
    // Single ps command with all needed fields
    // Format: PID %CPU %MEM TTY LSTART ARGS
    // Using custom format to get lstart (which spans multiple columns)
    const output = execSync(
      `ps -eo pid,pcpu,pmem,tty,lstart,args | grep -E '[c]laude'`,
      { encoding: 'utf-8', timeout: 10000 }
    );

    const lines = output.trim().split('\n').filter(Boolean);
    const candidatePids: number[] = [];
    const processDataMap = new Map<number, {
      cpu: number;
      mem: number;
      tty: string;
      startTime: Date | undefined;
      args: string[];
      command: string;
    }>();

    for (const line of lines) {
      // Parse: PID %CPU %MEM TTY LSTART(5 fields) ARGS
      // Example: 12345  0.0  0.5 ttys001 Mon Dec  9 10:30:00 2024 /usr/bin/claude --flag
      const parts = line.trim().split(/\s+/);
      if (parts.length < 10) continue;

      const pid = parseInt(parts[0], 10);
      if (isNaN(pid)) continue;

      const cpu = parseFloat(parts[1]) || 0;
      const mem = parseFloat(parts[2]) || 0;
      const tty = parts[3];

      // lstart is 5 fields: "Mon Dec  9 10:30:00 2024"
      const lstartStr = parts.slice(4, 9).join(' ');
      const startTime = parseLstartDate(lstartStr);

      // args is everything after lstart
      const argsStart = 9;
      const command = parts.slice(argsStart).join(' ');
      const args = parts.slice(argsStart + 1); // Skip the command itself

      // Skip non-main claude processes
      if (!command.includes('claude')) continue;
      if (command.includes('/bin/zsh') || command.includes('/bin/bash')) continue;

      candidatePids.push(pid);
      processDataMap.set(pid, { cpu, mem, tty, startTime, args, command });
    }

    // Batch get working directories (single lsof call)
    const cwdMap = batchGetProcessCwd(candidatePids);

    // Build final process list
    const processes: ClaudeProcessInfo[] = [];
    for (const pid of candidatePids) {
      const cwd = cwdMap.get(pid);
      if (!cwd) continue; // Skip if we can't get working directory

      const data = processDataMap.get(pid)!;
      processes.push({
        pid,
        cwd,
        tty: data.tty !== '??' ? data.tty : undefined,
        cpu: data.cpu,
        memory: data.mem,
        startTime: data.startTime || new Date(),
        args: data.args,
      });
    }

    // Update cache
    processCache = { processes, timestamp: Date.now() };

    logger.debug(`Found ${processes.length} Claude processes (2 syscalls)`);
    return processes;
  } catch (error) {
    // No claude processes found (grep returns exit code 1)
    if ((error as { status?: number }).status === 1) {
      processCache = { processes: [], timestamp: Date.now() };
      return [];
    }
    throw new ProcessScanError('Failed to scan processes', {
      error: (error as Error).message,
    });
  }
}

/** Check if a specific process is still running */
export function isProcessRunning(pid: number): boolean {
  try {
    execSync(`ps -p ${pid} -o pid=`, { encoding: 'utf-8', timeout: 5000 });
    return true;
  } catch {
    return false;
  }
}

/** Get working directory for a single process (fallback when not in batch) */
function getSingleProcessCwd(pid: number): string | undefined {
  const cwdMap = batchGetProcessCwd([pid]);
  return cwdMap.get(pid);
}

/** Get process info by PID - uses cache first, then falls back to direct lookup */
export function getProcessInfo(pid: number): ClaudeProcessInfo | null {
  // Try cache first
  const cacheTtl = config.get().processCacheTtl;
  if (processCache && Date.now() - processCache.timestamp < cacheTtl) {
    const cached = processCache.processes.find((p) => p.pid === pid);
    if (cached) return cached;
  }

  // Fall back to direct lookup (single process)
  if (!isProcessRunning(pid)) return null;

  const cwd = getSingleProcessCwd(pid);
  if (!cwd) return null;

  try {
    const output = execSync(`ps -p ${pid} -o pcpu=,pmem=,tty=,lstart=,args=`, {
      encoding: 'utf-8',
      timeout: 5000,
    });
    const lines = output.trim().split('\n');
    if (lines.length === 0) return null;

    const parts = lines[0].trim().split(/\s+/);
    if (parts.length < 9) return null;

    const cpu = parseFloat(parts[0]) || 0;
    const mem = parseFloat(parts[1]) || 0;
    const tty = parts[2];
    const lstartStr = parts.slice(3, 8).join(' ');
    const startTime = parseLstartDate(lstartStr);
    const args = parts.slice(9);

    return {
      pid,
      cwd,
      cpu,
      memory: mem,
      tty: tty !== '??' ? tty : undefined,
      startTime: startTime || new Date(),
      args,
    };
  } catch {
    return null;
  }
}

/** Stop a process gracefully (SIGTERM) or forcefully (SIGKILL) */
export function stopProcess(pid: number, force: boolean = false): { success: boolean; error?: string } {
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
