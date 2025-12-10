/**
 * Claude Code data types (from JSONL files)
 */

/** Base JSONL entry */
export interface ClaudeJsonlEntry {
  type: 'user' | 'assistant' | 'file-history-snapshot';
  uuid: string;
  parentUuid?: string | null;
  sessionId: string;
  cwd: string;
  timestamp: string;
  version?: string;
  gitBranch?: string;
  isSidechain?: boolean;
}

/** User message entry */
export interface ClaudeUserEntry extends ClaudeJsonlEntry {
  type: 'user';
  userType: 'external' | 'internal';
  message: {
    role: 'user';
    content: string | ClaudeToolResult[];
  };
}

/** Tool result in user message */
export interface ClaudeToolResult {
  type: 'tool_result';
  tool_use_id: string;
  content: string;
}

/** Assistant message entry */
export interface ClaudeAssistantEntry extends ClaudeJsonlEntry {
  type: 'assistant';
  message: {
    role: 'assistant';
    model: string;
    content: ClaudeContentBlock[];
    stop_reason?: string;
    usage?: {
      input_tokens: number;
      output_tokens: number;
    };
  };
  requestId?: string;
}

/** Content block types */
export type ClaudeContentBlock =
  | ClaudeTextBlock
  | ClaudeToolUseBlock
  | ClaudeThinkingBlock;

export interface ClaudeTextBlock {
  type: 'text';
  text: string;
}

export interface ClaudeToolUseBlock {
  type: 'tool_use';
  id: string;
  name: string;
  input: Record<string, unknown>;
}

export interface ClaudeThinkingBlock {
  type: 'thinking';
  thinking: string;
}

/** File history snapshot entry */
export interface ClaudeFileHistoryEntry extends ClaudeJsonlEntry {
  type: 'file-history-snapshot';
  messageId: string;
  snapshot: {
    messageId: string;
    timestamp: string;
    trackedFileBackups: Record<string, unknown>;
  };
}

/** History entry (from history.jsonl) */
export interface ClaudeHistoryEntry {
  display: string;
  pastedContents: Record<string, unknown>;
  timestamp: number;
  project: string;
  sessionId: string;
}

/** Union type for all entry types */
export type ClaudeEntry =
  | ClaudeUserEntry
  | ClaudeAssistantEntry
  | ClaudeFileHistoryEntry;

/** Tool call info */
export interface ToolCallInfo {
  name: string;
  input: Record<string, unknown>;
  timestamp: string;
}

/** Usage statistics */
export interface SessionUsageStats {
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  totalCost: number;
  apiCalls: number;
}

/** Parsed session data */
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
  toolCalls?: ToolCallInfo[];
  usageStats?: SessionUsageStats;
}
