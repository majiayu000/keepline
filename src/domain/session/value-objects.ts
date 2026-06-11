/**
 * Session domain value objects
 */

/** Session status enumeration */
export type SessionStatus =
  | 'running'    // Process exists and actively processing
  | 'waiting'    // Process exists but waiting for input
  | 'idle'       // Process exists but inactive
  | 'lost'       // No process but session data exists
  | 'completed'; // Session finished

/** All possible session statuses */
export const SESSION_STATUSES: readonly SessionStatus[] = [
  'running',
  'waiting',
  'idle',
  'lost',
  'completed',
] as const;

/** Check if a status is active (has or should have a running process) */
export function isActiveStatus(status: SessionStatus): boolean {
  return status === 'running' || status === 'waiting' || status === 'idle';
}

/** Check if a status indicates the session needs attention */
export function needsAttention(status: SessionStatus): boolean {
  return status === 'lost' || status === 'waiting';
}

/** Tool call information */
export interface ToolCallInfo {
  name: string;
  input: Record<string, unknown>;
  timestamp: string;
}

/** Session usage statistics (tokens / cost / API calls) */
export interface SessionUsageStats {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  totalCost: number;
  apiCalls: number;
}

/** Process information from system */
export interface ProcessInfo {
  pid: number;
  command: string;
  cwd?: string;
  tty?: string;
  cpu?: number;
  memory?: number;
  startTime?: Date;
}

/** Claude session file information */
export interface ClaudeSessionFile {
  sessionId: string;
  directory: string;
  filePath: string;
  modifiedAt: Date;
}
