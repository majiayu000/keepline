/**
 * Process module types
 */

import type { AgentClient } from '../../domain/session/index.js';

/** Raw process info from system */
export interface RawProcessInfo {
  pid: number;
  ppid: number;
  uid: number;
  cpu: number;
  memory: number;
  command: string;
  started: Date;
  state: string;
}

/** Agent process info */
export interface ClaudeProcessInfo {
  client: AgentClient;
  pid: number;
  cwd: string;
  tty?: string;
  cpu: number;
  memory: number;
  startTime: Date;
  args: string[];
}

/** Process state */
export type ProcessState = 'running' | 'sleeping' | 'waiting' | 'zombie' | 'unknown';

/** Map process state string to enum */
export function parseProcessState(state: string): ProcessState {
  const stateMap: Record<string, ProcessState> = {
    R: 'running',
    'R+': 'running',
    S: 'sleeping',
    'S+': 'sleeping',
    D: 'waiting',
    Z: 'zombie',
  };
  return stateMap[state] || 'unknown';
}
