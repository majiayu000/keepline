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

/** Parse a session JSONL file */
export function summarizeSessionEntries(entries: ClaudeEntryWithAgent[]): ParsedSessionData | null {
  if (entries.length === 0) return null;

  // Extract session info from first entry
  const firstEntry = entries[0];
  const sessionId = firstEntry.sessionId;
  const directory = firstEntry.cwd;

  // Skip if no valid session ID
  if (!sessionId || !directory) return null;

  // Check for agent info (sub-agent sessions)
  const agentId = firstEntry.agentId;
  const isSubAgent = !!agentId || firstEntry.isSidechain === true;
  // For sub-agents, the sessionId in the file is actually the parent's sessionId
  const parentSessionId = isSubAgent ? sessionId : undefined;

  // Find first user message and last assistant message
  let firstMessage: string | undefined;
  let lastMessage: string | undefined;
  let firstSystemCommand: string | undefined;
  let messageCount = 0;
  let toolCount = 0;
  let lastTool: string | undefined;
  let lastToolInput: Record<string, unknown> | undefined;
  let currentFile: string | undefined;
  let startedAtMs: number | undefined;
  let lastActiveAtMs = Date.parse(firstEntry.timestamp);
  if (Number.isNaN(lastActiveAtMs)) {
    lastActiveAtMs = Date.now();
  }
  const toolCalls: ToolCallInfo[] = [];
  const usageAccumulator = createUsageAccumulator();

  for (const entry of entries) {
    const entryTimeMs = Date.parse(entry.timestamp);
    if (!Number.isNaN(entryTimeMs) && entryTimeMs > lastActiveAtMs) {
      lastActiveAtMs = entryTimeMs;
    }
    if (!Number.isNaN(entryTimeMs) && (startedAtMs === undefined || entryTimeMs < startedAtMs)) {
      startedAtMs = entryTimeMs;
    }

    if (isUserEntry(entry) && entry.userType === 'external') {
      messageCount++;
      if (!firstMessage) {
        firstMessage = extractUserPrompt(entry);
      }
    }

    // Track system commands (like /resume, /usage) as fallback
    if (isSystemEntry(entry) && !firstSystemCommand) {
      const content = (entry as { content?: string }).content;
      if (content) {
        firstSystemCommand = extractSystemCommand(content);
      }
    }

    if (isAssistantEntry(entry)) {
      let entryToolCount = 0;
      let entryLastToolUse: ClaudeToolUseBlock | undefined;
      let entryText = '';
      let hasText = false;

      for (const block of entry.message.content) {
        if (block.type === 'tool_use') {
          entryToolCount++;
          entryLastToolUse = block;
          toolCalls.push({
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

      toolCount += entryToolCount;

      if (entryLastToolUse) {
        lastTool = entryLastToolUse.name;
        lastToolInput = entryLastToolUse.input;

        const file = extractCurrentFile(entryLastToolUse.input);
        if (file) {
          currentFile = file;
        }
      }

      if (hasText) {
        const text = entryText.trim();
        if (text) {
          lastMessage = text;
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
        addUsageToAccumulator(usageAccumulator, entry.message.model, {
          input_tokens: usage.input_tokens,
          output_tokens: usage.output_tokens,
          cache_creation_input_tokens: usage.cache_creation_input_tokens,
          cache_read_input_tokens: usage.cache_read_input_tokens,
        });
      }
    }
  }

  // Use system command as fallback if no user message found
  if (!firstMessage && firstSystemCommand) {
    firstMessage = `System: ${firstSystemCommand}`;
  }

  // Extract usage stats
  const rawUsageStats = usageStatsFromAccumulator(usageAccumulator);
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
    // For sub-agents, use agentId as sessionId to make them unique
    sessionId: isSubAgent && agentId ? `agent-${agentId}` : sessionId,
    directory,
    firstMessage,
    lastMessage,
    messageCount,
    toolCount,
    lastTool,
    lastToolInput,
    currentFile,
    startedAt: startedAtMs === undefined ? undefined : new Date(startedAtMs),
    lastActiveAt: new Date(lastActiveAtMs),
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
    usageStats,
    // Multi-session tracking fields
    agentId,
    parentSessionId,
    isSubAgent,
  };
}

/** Parse a session JSONL file */
export async function parseSessionFile(filePath: string): Promise<ParsedSessionData | null> {
  const entries: ClaudeEntryWithAgent[] = [];
  let lineNumber = 0;

  const fileStream = createReadStream(filePath);
  const rl = createInterface({
    input: fileStream,
    crlfDelay: Infinity,
  });

  for await (const line of rl) {
    lineNumber++;
    const entry = parseLine(line, lineNumber, filePath);
    if (entry && entry.type !== 'file-history-snapshot') {
      entries.push(entry);
    }
  }

  return summarizeSessionEntries(entries);
}
