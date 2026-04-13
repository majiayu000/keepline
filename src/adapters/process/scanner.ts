/**
 * Process scanner for detecting Claude Code instances
 *
 * Optimized to minimize system calls:
 * - Single ps command with all needed fields
 * - Batch lsof for working directories
 * - Process list caching during sync cycles
 */

import { execSync } from 'child_process';
import { logger } from '../../lib/logger.js';
import { config } from '../../lib/config.js';
import { ProcessScanError } from '../../lib/errors.js';
import type { ClaudeProcessInfo } from './types.js';

// Maximum valid PID (varies by OS, but 2^22 is common max)
const MAX_PID = 4194304;

/** Validate PID is a safe positive integer */
function validatePid(pid: number): boolean {
  return Number.isInteger(pid) && pid > 0 && pid <= MAX_PID;
}

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

  // Validate all PIDs first
  const validPids = pids.filter(pid => validatePid(pid));
  if (validPids.length === 0) return cwdMap;

  try {
    // Use lsof with multiple PIDs in one call: lsof -a -d cwd -p pid1,pid2,pid3
    const pidList = validPids.join(',');
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
  if (!lstartStr) return undefined;
  const parsed = new Date(lstartStr);
  return Number.isNaN(parsed.getTime()) ? undefined : parsed;
}

function parseLstartTimestamp(lstartStr: string): number | undefined {
  if (!lstartStr) return undefined;
  const timestamp = Date.parse(lstartStr);
  return Number.isNaN(timestamp) ? undefined : timestamp;
}

interface ParsedPsProcessData {
  pid: number;
  cpu: number;
  mem: number;
  tty: string;
  startTimeMs: number | undefined;
  argsRaw: string;
}

function isAsciiWhitespace(code: number): boolean {
  return code === 32 || code === 9 || code === 10 || code === 13;
}

/**
 * Split by ASCII whitespace with a hard part limit.
 * The last part contains the unsplit remainder.
 */
function splitWhitespaceWithLimit(line: string, limit: number): string[] {
  const parts = new Array<string>(limit);
  let partCount = 0;
  const length = line.length;
  let index = 0;

  while (index < length && partCount < limit - 1) {
    while (index < length && isAsciiWhitespace(line.charCodeAt(index))) {
      index++;
    }
    if (index >= length) break;

    const start = index;
    while (index < length && !isAsciiWhitespace(line.charCodeAt(index))) {
      index++;
    }
    parts[partCount++] = line.slice(start, index);
  }

  while (index < length && isAsciiWhitespace(line.charCodeAt(index))) {
    index++;
  }
  if (index < length) {
    parts[partCount++] = line.slice(index);
  }

  parts.length = partCount;
  return parts;
}

/** Parse ps output and keep only main Claude processes */
export function parseClaudePsOutput(output: string): ParsedPsProcessData[] {
  const parsedProcesses: ParsedPsProcessData[] = [];

  const lines = output.split('\n');
  for (const rawLine of lines) {
    if (!rawLine) continue;
    if (!rawLine.includes('claude')) continue;
    if (rawLine.includes('/bin/zsh') || rawLine.includes('/bin/bash')) continue;

    // Parse: PID %CPU %MEM TTY LSTART(5 fields) ARGS
    // Example: 12345  0.0  0.5 ttys001 Mon Dec  9 10:30:00 2024 /usr/bin/claude --flag
    const parts = splitWhitespaceWithLimit(rawLine, 10);
    if (parts.length < 10) continue;

    const pid = parseInt(parts[0], 10);
    if (isNaN(pid) || !validatePid(pid)) continue;

    const cpu = parseFloat(parts[1]) || 0;
    const mem = parseFloat(parts[2]) || 0;
    const tty = parts[3];

    // lstart is 5 fields: "Mon Dec  9 10:30:00 2024"
    const lstartStr = `${parts[4]} ${parts[5]} ${parts[6]} ${parts[7]} ${parts[8]}`;
    const startTimeMs = parseLstartTimestamp(lstartStr);

    // args is command tail after first token (the binary path)
    const command = parts[9];
    const firstSpace = command.indexOf(' ');
    const argsRaw = firstSpace >= 0 ? command.slice(firstSpace + 1) : '';

    parsedProcesses.push({ pid, cpu, mem, tty, startTimeMs, argsRaw });
  }

  return parsedProcesses;
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

    const parsedProcesses = parseClaudePsOutput(output);
    const candidatePids = parsedProcesses.map((process) => process.pid);

    // Batch get working directories (single lsof call)
    const cwdMap = batchGetProcessCwd(candidatePids);

    // Build final process list
    const processes: ClaudeProcessInfo[] = [];
    for (const parsedProcess of parsedProcesses) {
      const cwd = cwdMap.get(parsedProcess.pid);
      if (!cwd) continue; // Skip if we can't get working directory

      processes.push({
        pid: parsedProcess.pid,
        cwd,
        tty: parsedProcess.tty !== '??' ? parsedProcess.tty : undefined,
        cpu: parsedProcess.cpu,
        memory: parsedProcess.mem,
        startTime: parsedProcess.startTimeMs === undefined ? new Date() : new Date(parsedProcess.startTimeMs),
        args: parsedProcess.argsRaw ? parsedProcess.argsRaw.split(/\s+/) : [],
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

/** Get working directory for a single process (fallback when not in batch) */
function getSingleProcessCwd(pid: number): string | undefined {
  const cwdMap = batchGetProcessCwd([pid]);
  return cwdMap.get(pid);
}

/** Get process info by PID - uses cache first, then falls back to direct lookup */
export function getProcessInfo(pid: number): ClaudeProcessInfo | null {
  if (!validatePid(pid)) {
    return null;
  }

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
