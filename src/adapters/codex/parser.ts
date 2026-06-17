/**
 * JSONL parser for Codex saved session files.
 */

import { createReadStream } from 'fs';
import { createInterface } from 'readline';
import { ParseError } from '../../lib/errors.js';
import type { ToolCallInfo } from '../../domain/session/index.js';
import type { CodexJsonlEntry, CodexParsedSessionData } from './types.js';

export interface ParseCodexSessionOptions {
  includeToolCalls?: boolean;
}

interface CodexAccumulator {
  rawSessionId: string;
  sessionId: string;
  directory: string;
  firstMessage?: string;
  lastMessage?: string;
  lastTool?: string;
  lastToolInput?: Record<string, unknown>;
  currentFile?: string;
  messageCount: number;
  toolCount: number;
  startedAtMs?: number;
  lastActiveAtMs: number;
  toolCalls: ToolCallInfo[];
  includeToolCalls: boolean;
}

export function scopeCodexSessionId(rawSessionId: string): string {
  return rawSessionId.startsWith('codex_') ? rawSessionId : `codex_${rawSessionId}`;
}

export function unscopeCodexSessionId(sessionId: string): string {
  return sessionId.startsWith('codex_') ? sessionId.slice('codex_'.length) : sessionId;
}

function parseCodexTimestampMs(timestamp: unknown): number | undefined {
  if (typeof timestamp !== 'string') return undefined;
  const timestampMs = Date.parse(timestamp);
  return Number.isNaN(timestampMs) ? undefined : timestampMs;
}

function firstTextContent(content: unknown): string | undefined {
  if (!Array.isArray(content)) return undefined;

  for (const block of content) {
    if (!block || typeof block !== 'object') continue;
    const record = block as Record<string, unknown>;
    if (
      (record.type === 'input_text' || record.type === 'output_text') &&
      typeof record.text === 'string'
    ) {
      return record.text;
    }
  }

  return undefined;
}

function parseFunctionArguments(argumentsValue: unknown): Record<string, unknown> | undefined {
  if (!argumentsValue) return undefined;
  if (typeof argumentsValue === 'object' && !Array.isArray(argumentsValue)) {
    return argumentsValue as Record<string, unknown>;
  }
  if (typeof argumentsValue !== 'string') return undefined;

  try {
    const parsed = JSON.parse(argumentsValue);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    return { arguments: argumentsValue };
  }

  return undefined;
}

function extractCodexCurrentFile(toolInput: Record<string, unknown> | undefined): string | undefined {
  if (!toolInput) return undefined;

  for (const key of ['path', 'file_path', 'filePath', 'notebook_path']) {
    const value = toolInput[key];
    if (typeof value === 'string') return value;
  }

  return undefined;
}

function createAccumulator(entry: CodexJsonlEntry, includeToolCalls: boolean): CodexAccumulator | null {
  if (entry.type !== 'session_meta' || !entry.payload) return null;

  const rawSessionId = entry.payload.id;
  const directory = entry.payload.cwd;
  if (typeof rawSessionId !== 'string' || typeof directory !== 'string') {
    return null;
  }

  const startedAtMs = parseCodexTimestampMs(entry.timestamp);
  return {
    rawSessionId,
    sessionId: scopeCodexSessionId(rawSessionId),
    directory,
    messageCount: 0,
    toolCount: 0,
    startedAtMs,
    lastActiveAtMs: startedAtMs ?? Date.now(),
    toolCalls: [],
    includeToolCalls,
  };
}

function updateTimestamp(accumulator: CodexAccumulator, timestamp: unknown): void {
  const timestampMs = parseCodexTimestampMs(timestamp);
  if (timestampMs === undefined) return;
  if (accumulator.startedAtMs === undefined || timestampMs < accumulator.startedAtMs) {
    accumulator.startedAtMs = timestampMs;
  }
  if (timestampMs > accumulator.lastActiveAtMs) {
    accumulator.lastActiveAtMs = timestampMs;
  }
}

function accumulateResponseItem(accumulator: CodexAccumulator, entry: CodexJsonlEntry): void {
  const payload = entry.payload;
  if (!payload) return;

  updateTimestamp(accumulator, entry.timestamp);

  if (payload.type === 'message') {
    accumulator.messageCount++;
    const text = firstTextContent(payload.content);

    if (payload.role === 'user' && !accumulator.firstMessage && text) {
      accumulator.firstMessage = text;
    }
    if (payload.role === 'assistant' && text) {
      accumulator.lastMessage = text;
    }
    return;
  }

  if (payload.type === 'function_call') {
    const name = typeof payload.name === 'string' ? payload.name : undefined;
    if (!name) return;

    const input = parseFunctionArguments(payload.arguments) ?? {};
    accumulator.toolCount++;
    accumulator.lastTool = name;
    accumulator.lastToolInput = input;
    accumulator.currentFile = extractCodexCurrentFile(input) ?? accumulator.currentFile;

    if (accumulator.includeToolCalls) {
      accumulator.toolCalls.push({
        name,
        input,
        timestamp: typeof entry.timestamp === 'string' ? entry.timestamp : new Date().toISOString(),
      });
    }
  }
}

function finalizeAccumulator(accumulator: CodexAccumulator): CodexParsedSessionData {
  return {
    client: 'codex',
    rawSessionId: accumulator.rawSessionId,
    sessionId: accumulator.sessionId,
    directory: accumulator.directory,
    firstMessage: accumulator.firstMessage,
    lastMessage: accumulator.lastMessage,
    messageCount: accumulator.messageCount,
    toolCount: accumulator.toolCount,
    lastTool: accumulator.lastTool,
    lastToolInput: accumulator.lastToolInput,
    currentFile: accumulator.currentFile,
    startedAt: accumulator.startedAtMs === undefined ? undefined : new Date(accumulator.startedAtMs),
    lastActiveAt: new Date(accumulator.lastActiveAtMs),
    toolCalls: accumulator.includeToolCalls ? accumulator.toolCalls : undefined,
  };
}

export async function parseCodexSessionFile(
  filePath: string,
  options: ParseCodexSessionOptions = {}
): Promise<CodexParsedSessionData | null> {
  const fileStream = createReadStream(filePath);
  const rl = createInterface({
    input: fileStream,
    crlfDelay: Infinity,
  });

  let lineNumber = 0;
  let accumulator: CodexAccumulator | null = null;
  const includeToolCalls = options.includeToolCalls ?? true;

  for await (const line of rl) {
    lineNumber++;
    if (!line.trim()) continue;

    let entry: CodexJsonlEntry;
    try {
      entry = JSON.parse(line) as CodexJsonlEntry;
    } catch {
      throw new ParseError(filePath, lineNumber);
    }

    if (!accumulator) {
      accumulator = createAccumulator(entry, includeToolCalls);
      if (accumulator) {
        continue;
      }
    }

    if (!accumulator) continue;
    if (entry.type === 'response_item') {
      accumulateResponseItem(accumulator, entry);
    } else {
      updateTimestamp(accumulator, entry.timestamp);
    }
  }

  return accumulator ? finalizeAccumulator(accumulator) : null;
}
