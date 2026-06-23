/**
 * Helpers shared by runtime adapter wrappers.
 */

import type { ParsedSessionData, ToolCallInfo } from '../../domain/session/index.js';
import type {
  JsonValue,
  RuntimeCommand,
  RuntimeId,
  RuntimeSession,
  RuntimeUsageStats,
} from '../../domain/runtime/index.js';
import type { CodexParsedSessionData } from '../codex/types.js';

type ParsedRuntimeSource = ParsedSessionData | CodexParsedSessionData;

export function structuredCommand(argv: string[], cwd?: string): RuntimeCommand {
  const [executable, ...args] = argv;
  if (!executable) {
    throw new Error('Runtime command executable is required');
  }
  return { executable, args, cwd };
}

export function titleFromPrompt(prompt: string | undefined, fallback: string): string {
  if (!prompt) return fallback;
  if (prompt.length <= 80) return prompt;

  const truncated = prompt.slice(0, 80);
  const lastSpace = truncated.lastIndexOf(' ');
  return lastSpace > 40 ? `${truncated.slice(0, lastSpace)}...` : `${truncated}...`;
}

function stringFromInput(input: Record<string, unknown>, keys: string[]): string | undefined {
  for (const key of keys) {
    const value = input[key];
    if (typeof value === 'string' && value.length > 0) {
      return value;
    }
  }
  return undefined;
}

export function filesTouchedFromSession(
  session: Pick<ParsedSessionData, 'currentFile' | 'toolCalls'>
): string[] {
  const files = new Set<string>();
  if (session.currentFile) {
    files.add(session.currentFile);
  }

  for (const call of session.toolCalls ?? []) {
    const filePath = stringFromInput(call.input, [
      'path',
      'file_path',
      'filePath',
      'notebook_path',
    ]);
    if (filePath) {
      files.add(filePath);
    }
  }

  return [...files];
}

function toRuntimeUsageStats(
  usageStats: ParsedRuntimeSource['usageStats']
): RuntimeUsageStats | undefined {
  if (!usageStats) return undefined;
  return {
    totalInputTokens: usageStats.totalInputTokens,
    totalOutputTokens: usageStats.totalOutputTokens,
    totalTokens: usageStats.totalTokens,
    totalCost: usageStats.totalCost,
    apiCalls: usageStats.apiCalls,
  };
}

export function parsedSessionToRuntimeSession(
  runtimeId: RuntimeId,
  session: ParsedRuntimeSource,
  metadata: Record<string, JsonValue> = {}
): RuntimeSession {
  return {
    runtimeId,
    sessionId: 'rawSessionId' in session ? session.rawSessionId : session.sessionId,
    cwd: session.directory,
    agentId: session.agentId,
    parentSessionId: session.parentSessionId,
    parentRuntimeSessionId: session.parentSessionId,
    isSubAgent: session.isSubAgent,
    status: 'unknown',
    title: titleFromPrompt(session.firstMessage, session.sessionId),
    initialPrompt: session.firstMessage,
    lastMessage: session.lastMessage,
    lastTool: session.lastTool,
    lastToolInput: session.lastToolInput,
    currentFile: session.currentFile,
    filesTouched: filesTouchedFromSession(session),
    toolCalls: session.toolCalls as ToolCallInfo[] | undefined,
    toolCount: session.toolCount,
    messageCount: session.messageCount,
    startedAt: session.startedAt,
    lastActiveAt: session.lastActiveAt,
    usageStats: toRuntimeUsageStats(session.usageStats),
    runtimeMetadata: metadata,
  };
}
