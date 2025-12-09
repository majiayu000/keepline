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
} from '../types.js';

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

  // Find first user message
  let firstMessage: string | undefined;
  let messageCount = 0;
  let toolCount = 0;
  let lastTool: string | undefined;
  let lastToolInput: Record<string, unknown> | undefined;
  let currentFile: string | undefined;
  let startedAt: Date | undefined;
  let lastActiveAt: Date = new Date(firstEntry.timestamp);

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

    if (isAssistantEntry(entry)) {
      const toolUses = extractToolUses(entry);
      toolCount += toolUses.length;

      if (toolUses.length > 0) {
        const lastToolUse = toolUses[toolUses.length - 1];
        lastTool = lastToolUse.name;
        lastToolInput = lastToolUse.input;

        const file = extractCurrentFile(lastToolUse.input);
        if (file) {
          currentFile = file;
        }
      }
    }
  }

  return {
    sessionId,
    directory,
    firstMessage,
    messageCount,
    toolCount,
    lastTool,
    lastToolInput,
    currentFile,
    startedAt,
    lastActiveAt,
  };
}
