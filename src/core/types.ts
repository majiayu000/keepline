/**
 * Core type definitions for Tasker
 */

/** Session status enumeration */
export type SessionStatus =
  | 'running'    // Process exists and actively processing
  | 'waiting'    // Process exists but waiting for input
  | 'idle'       // Process exists but inactive
  | 'lost'       // No process but session data exists
  | 'completed'; // Session finished

/** Base entity interface */
export interface Entity {
  id: string;
  createdAt: Date;
  updatedAt: Date;
}

/** Session entity */
export interface Session extends Entity {
  sessionId: string;        // Claude session ID
  directory: string;        // Working directory
  status: SessionStatus;

  // Task information
  title: string;            // Extracted/generated title
  initialPrompt: string;    // Original user prompt

  // Activity tracking
  lastTool?: string;        // Last used tool
  lastToolInput?: string;   // Tool input (JSON string)
  currentFile?: string;     // Currently editing file

  // Timeline
  startedAt?: Date;
  lastActiveAt: Date;
  completedAt?: Date;

  // Process info
  pid?: number;
  tty?: string;

  // Metrics
  toolCount: number;        // Total tools used
  messageCount: number;     // Total messages
}

/** Process info from system */
export interface ProcessInfo {
  pid: number;
  command: string;
  cwd?: string;
  tty?: string;
  cpu?: number;
  memory?: number;
  startTime?: Date;
}

/** Claude session file info */
export interface ClaudeSessionFile {
  sessionId: string;
  directory: string;        // Original directory path
  filePath: string;         // Path to JSONL file
  modifiedAt: Date;
}

/** Tool usage record */
export interface ToolUsage {
  timestamp: Date;
  tool: string;
  input: Record<string, unknown>;
  output?: string;
  duration?: number;
}

/** Event payload types */
export interface SessionEventPayload {
  session: Session;
  previousStatus?: SessionStatus;
}

export interface ToolEventPayload {
  sessionId: string;
  tool: string;
  input: Record<string, unknown>;
  timestamp: Date;
}

/** Result type for operations */
export type Result<T, E = Error> =
  | { success: true; data: T }
  | { success: false; error: E };

/** Parsed session data from JSONL */
export interface ParsedSessionData {
  sessionId: string;
  directory: string;
  firstMessage?: string;
  lastMessage?: string;
  messageCount: number;
  toolCount: number;
  lastTool?: string;
  lastToolInput?: Record<string, unknown>;
  currentFile?: string;
  startedAt?: Date;
  lastActiveAt: Date;
}
