/**
 * JSONL file parser for Claude session files
 */

import { createReadStream } from 'fs';
import { createInterface } from 'readline';
import { ParseError } from '../../../lib/errors.js';
import type {
  ClaudeEntry,
  ClaudeUserEntry,
  ClaudeAssistantEntry,
  ClaudeToolUseBlock,
  ParsedSessionData,
  ToolCallInfo,
  SessionUsageStats,
} from '../types.js';
import {
  addUsageToAccumulator,
  createUsageAccumulator,
  usageStatsFromAccumulator,
} from '../../../services/usage.extractor.js';

/** Extended entry type with agent info */
export type ClaudeEntryWithAgent = ClaudeEntry & {
  agentId?: string;
  isSidechain?: boolean;
}

interface SessionSummaryAccumulator {
  sessionId: string;
  directory: string;
  agentId?: string;
  isSubAgent: boolean;
  parentSessionId?: string;
  firstMessage?: string;
  lastMessage?: string;
  firstSystemCommand?: string;
  messageCount: number;
  toolCount: number;
  lastTool?: string;
  lastToolInput?: Record<string, unknown>;
  currentFile?: string;
  startedAtMs?: number;
  lastActiveAtMs: number;
  toolCalls: ToolCallInfo[];
  usageAccumulator: ReturnType<typeof createUsageAccumulator>;
}

/** Parse a single JSONL line */
function parseLine(line: string, lineNumber: number, filePath: string): ClaudeEntryWithAgent | null {
  if (!line.trim()) return null;

  try {
    return JSON.parse(line) as ClaudeEntryWithAgent;
  } catch {
    throw new ParseError(filePath, lineNumber);
  }
}

/** Check if entry is a user entry */
function isUserEntry(entry: ClaudeEntry): entry is ClaudeUserEntry {
  return entry.type === 'user';
}

/** Check if entry is a system entry (slash commands like /resume, /usage) */
function isSystemEntry(entry: ClaudeEntry): boolean {
  return (entry as { type: string }).type === 'system';
}

/** Extract command name from system entry content */
function extractSystemCommand(content: string): string | undefined {
  const match = content.match(/<command-name>\/(\w+)<\/command-name>/);
  return match ? `/${match[1]}` : undefined;
}

/** Check if entry is an assistant entry */
function isAssistantEntry(entry: ClaudeEntry): entry is ClaudeAssistantEntry {
  return entry.type === 'assistant';
}

/** Extract first user prompt from entry */
function extractUserPrompt(entry: ClaudeUserEntry): string | undefined {
  const content = entry.message.content;
  if (typeof content === 'string') {
    return content;
  }
  return undefined;
}

/** Extract current file from tool input */
function extractCurrentFile(toolInput: Record<string, unknown>): string | undefined {
  // Common file path keys in Claude tools
  const fileKeys = ['file_path', 'path', 'filePath', 'notebook_path'];
  for (const key of fileKeys) {
    const value = toolInput[key];
    if (typeof value === 'string') {
      return value;
    }
  }
  return undefined;
}

function isFileHistorySnapshot(entry: ClaudeEntryWithAgent): boolean {
  return (entry as { type?: string }).type === 'file-history-snapshot';
}

function createSessionAccumulator(entry: ClaudeEntryWithAgent): SessionSummaryAccumulator | null {
  const sessionId = entry.sessionId;
  const directory = entry.cwd;

  if (!sessionId || !directory) {
    return null;
  }

  const agentId = entry.agentId;
  const isSubAgent = !!agentId || entry.isSidechain === true;
  let lastActiveAtMs = Date.parse(entry.timestamp);
  if (Number.isNaN(lastActiveAtMs)) {
    lastActiveAtMs = Date.now();
  }

  return {
    sessionId,
    directory,
    agentId,
    isSubAgent,
    parentSessionId: isSubAgent ? sessionId : undefined,
    messageCount: 0,
    toolCount: 0,
    lastActiveAtMs,
    toolCalls: [],
    usageAccumulator: createUsageAccumulator(),
  };
}

function accumulateSessionEntry(
  accumulator: SessionSummaryAccumulator,
  entry: ClaudeEntryWithAgent
): void {
  const entryTimeMs = Date.parse(entry.timestamp);
  if (!Number.isNaN(entryTimeMs) && entryTimeMs > accumulator.lastActiveAtMs) {
    accumulator.lastActiveAtMs = entryTimeMs;
  }
  if (!Number.isNaN(entryTimeMs) && (
    accumulator.startedAtMs === undefined || entryTimeMs < accumulator.startedAtMs
  )) {
    accumulator.startedAtMs = entryTimeMs;
  }

  if (isUserEntry(entry) && entry.userType === 'external') {
    accumulator.messageCount++;
    if (!accumulator.firstMessage) {
      accumulator.firstMessage = extractUserPrompt(entry);
    }
  }

  if (isSystemEntry(entry as ClaudeEntry) && !accumulator.firstSystemCommand) {
    const content = (entry as { content?: string }).content;
    if (content) {
      accumulator.firstSystemCommand = extractSystemCommand(content);
    }
  }

  if (!isAssistantEntry(entry)) {
    return;
  }

  let entryToolCount = 0;
  let entryLastToolUse: ClaudeToolUseBlock | undefined;
  let entryText = '';
  let hasText = false;

  for (const block of entry.message.content) {
    if (block.type === 'tool_use') {
      entryToolCount++;
      entryLastToolUse = block;
      accumulator.toolCalls.push({
        name: block.name,
        input: block.input,
        timestamp: entry.timestamp,
      });
      continue;
    }

    if (block.type === 'text') {
      if (hasText) {
        entryText += `\n${block.text}`;
      } else {
        entryText = block.text;
        hasText = true;
      }
    }
  }

  accumulator.toolCount += entryToolCount;

  if (entryLastToolUse) {
    accumulator.lastTool = entryLastToolUse.name;
    accumulator.lastToolInput = entryLastToolUse.input;

    const file = extractCurrentFile(entryLastToolUse.input);
    if (file) {
      accumulator.currentFile = file;
    }
  }

  if (hasText) {
    const text = entryText.trim();
    if (text) {
      accumulator.lastMessage = text;
    }
  }

  const usage = entry.message.usage as
    | {
        input_tokens?: number;
        output_tokens?: number;
        cache_creation_input_tokens?: number;
        cache_read_input_tokens?: number;
      }
    | undefined;
  if (
    usage &&
    typeof usage.input_tokens === 'number' &&
    typeof usage.output_tokens === 'number'
  ) {
    addUsageToAccumulator(accumulator.usageAccumulator, entry.message.model, {
      input_tokens: usage.input_tokens,
      output_tokens: usage.output_tokens,
      cache_creation_input_tokens: usage.cache_creation_input_tokens,
      cache_read_input_tokens: usage.cache_read_input_tokens,
    });
  }
}

function finalizeSessionAccumulator(
  accumulator: SessionSummaryAccumulator
): ParsedSessionData {
  if (!accumulator.firstMessage && accumulator.firstSystemCommand) {
    accumulator.firstMessage = `System: ${accumulator.firstSystemCommand}`;
  }

  const rawUsageStats = usageStatsFromAccumulator(accumulator.usageAccumulator);
  const usageStats: SessionUsageStats | undefined = rawUsageStats.apiCalls > 0
    ? {
        totalInputTokens: rawUsageStats.totalInputTokens,
        totalOutputTokens: rawUsageStats.totalOutputTokens,
        totalTokens: rawUsageStats.totalTokens,
        totalCost: rawUsageStats.totalCost,
        apiCalls: rawUsageStats.apiCalls,
      }
    : undefined;

  return {
    sessionId: accumulator.isSubAgent && accumulator.agentId
      ? `agent-${accumulator.agentId}`
      : accumulator.sessionId,
    directory: accumulator.directory,
    firstMessage: accumulator.firstMessage,
    lastMessage: accumulator.lastMessage,
    messageCount: accumulator.messageCount,
    toolCount: accumulator.toolCount,
    lastTool: accumulator.lastTool,
    lastToolInput: accumulator.lastToolInput,
    currentFile: accumulator.currentFile,
    startedAt: accumulator.startedAtMs === undefined
      ? undefined
      : new Date(accumulator.startedAtMs),
    lastActiveAt: new Date(accumulator.lastActiveAtMs),
    toolCalls: accumulator.toolCalls.length > 0 ? accumulator.toolCalls : undefined,
    usageStats,
    agentId: accumulator.agentId,
    parentSessionId: accumulator.parentSessionId,
    isSubAgent: accumulator.isSubAgent,
  };
}

/** Parse a session JSONL file */
export function summarizeSessionEntries(entries: ClaudeEntryWithAgent[]): ParsedSessionData | null {
  let accumulator: SessionSummaryAccumulator | null = null;

  for (const entry of entries) {
    if (isFileHistorySnapshot(entry)) {
      continue;
    }
    if (!accumulator) {
      accumulator = createSessionAccumulator(entry);
      if (!accumulator) {
        return null;
      }
    }
    accumulateSessionEntry(accumulator, entry);
  }

  return accumulator ? finalizeSessionAccumulator(accumulator) : null;
}

/** Parse a session JSONL file */
export async function parseSessionFile(filePath: string): Promise<ParsedSessionData | null> {
  let lineNumber = 0;
  let accumulator: SessionSummaryAccumulator | null = null;

  const fileStream = createReadStream(filePath);
  const rl = createInterface({
    input: fileStream,
    crlfDelay: Infinity,
  });

  for await (const line of rl) {
    lineNumber++;
    const entry = parseLine(line, lineNumber, filePath);
    if (!entry || isFileHistorySnapshot(entry)) {
      continue;
    }

    if (!accumulator) {
      accumulator = createSessionAccumulator(entry);
      if (!accumulator) {
        return null;
      }
    }

    accumulateSessionEntry(accumulator, entry);
  }

  return accumulator ? finalizeSessionAccumulator(accumulator) : null;
}
