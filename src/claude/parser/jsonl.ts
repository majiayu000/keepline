/**
 * JSONL file parser for Claude session files
 */

import { createReadStream } from 'fs';
import { createInterface } from 'readline';
import { ParseError } from '../../core/errors.js';
import type {
  ClaudeEntry,
  ClaudeUserEntry,
  ClaudeAssistantEntry,
  ClaudeToolUseBlock,
  ParsedSessionData,
  ToolCallInfo,
  SessionUsageStats,
} from '../types.js';
import { aggregateUsageStats } from '../../usage/extractor.js';

/** Parse a single JSONL line */
function parseLine(line: string, lineNumber: number, filePath: string): ClaudeEntry | null {
  if (!line.trim()) return null;

  try {
    return JSON.parse(line) as ClaudeEntry;
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

/** Extract tool uses from assistant entry */
function extractToolUses(entry: ClaudeAssistantEntry): ClaudeToolUseBlock[] {
  return entry.message.content.filter(
    (block): block is ClaudeToolUseBlock => block.type === 'tool_use'
  );
}

/** Extract text content from assistant entry */
function extractAssistantText(entry: ClaudeAssistantEntry): string | undefined {
  const textBlocks = entry.message.content.filter(
    (block): block is ClaudeTextBlock => block.type === 'text'
  );
  if (textBlocks.length === 0) return undefined;
  // Combine all text blocks
  return textBlocks.map(b => b.text).join('\n').trim();
}

/** Text block type guard */
interface ClaudeTextBlock {
  type: 'text';
  text: string;
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
export async function parseSessionFile(filePath: string): Promise<ParsedSessionData | null> {
  const entries: ClaudeEntry[] = [];
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

  if (entries.length === 0) return null;

  // Extract session info from first entry
  const firstEntry = entries[0];
  const sessionId = firstEntry.sessionId;
  const directory = firstEntry.cwd;

  // Skip if no valid session ID
  if (!sessionId || !directory) return null;

  // Find first user message and last assistant message
  let firstMessage: string | undefined;
  let lastMessage: string | undefined;
  let firstSystemCommand: string | undefined;
  let messageCount = 0;
  let toolCount = 0;
  let lastTool: string | undefined;
  let lastToolInput: Record<string, unknown> | undefined;
  let currentFile: string | undefined;
  let startedAt: Date | undefined;
  let lastActiveAt: Date = new Date(firstEntry.timestamp);
  const toolCalls: ToolCallInfo[] = [];

  for (const entry of entries) {
    const entryTime = new Date(entry.timestamp);
    if (entryTime > lastActiveAt) {
      lastActiveAt = entryTime;
    }
    if (!startedAt || entryTime < startedAt) {
      startedAt = entryTime;
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
      const toolUses = extractToolUses(entry);
      toolCount += toolUses.length;

      // Collect all tool calls
      for (const tool of toolUses) {
        toolCalls.push({
          name: tool.name,
          input: tool.input,
          timestamp: entry.timestamp,
        });
      }

      if (toolUses.length > 0) {
        const lastToolUse = toolUses[toolUses.length - 1];
        lastTool = lastToolUse.name;
        lastToolInput = lastToolUse.input;

        const file = extractCurrentFile(lastToolUse.input);
        if (file) {
          currentFile = file;
        }
      }

      // Extract assistant text content (always update to get the latest)
      const text = extractAssistantText(entry);
      if (text) {
        lastMessage = text;
      }
    }
  }

  // Use system command as fallback if no user message found
  if (!firstMessage && firstSystemCommand) {
    firstMessage = `System: ${firstSystemCommand}`;
  }

  // Extract usage stats
  const rawUsageStats = aggregateUsageStats(entries);
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
    sessionId,
    directory,
    firstMessage,
    lastMessage,
    messageCount,
    toolCount,
    lastTool,
    lastToolInput,
    currentFile,
    startedAt,
    lastActiveAt,
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
    usageStats,
  };
}
