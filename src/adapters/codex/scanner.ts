/**
 * Codex sessions directory scanner.
 */

import { existsSync, readdirSync, statSync } from 'fs';
import { join } from 'path';
import { CODEX_SESSIONS } from '../../lib/paths.js';
import { isValidSessionId } from '../../lib/session-id.js';
import { logger } from '../../lib/logger.js';
import { parseCodexSessionFile, scopeCodexSessionId } from './parser.js';
import type { CodexParsedSessionData, CodexSessionFile } from './types.js';

export interface CodexScanOptions {
  includeToolCalls?: boolean;
  maxAgeDays?: number;
}

interface CodexSessionCache {
  data: CodexParsedSessionData | null;
  modifiedAt: number;
  includeToolCalls: boolean;
}

const sessionSummaryCache = new Map<string, CodexSessionCache>();
const sessionDetailCache = new Map<string, CodexSessionCache>();

export function clearCodexSessionCache(): void {
  sessionSummaryCache.clear();
  sessionDetailCache.clear();
}

function scanDirectoryRecursive(
  dir: string,
  cutoffTime: number,
  sessions: CodexSessionFile[]
): void {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      scanDirectoryRecursive(path, cutoffTime, sessions);
      continue;
    }

    if (!entry.isFile() || !entry.name.endsWith('.jsonl') || !entry.name.startsWith('rollout-')) {
      continue;
    }

    const fileStat = statSync(path);
    if (cutoffTime > 0 && fileStat.mtime.getTime() < cutoffTime) {
      continue;
    }

    const match = entry.name.match(/([0-9a-f]{8}-[0-9a-f-]{27})\.jsonl$/i);
    if (!match) {
      continue;
    }

    const rawSessionId = match[1];
    const sessionId = scopeCodexSessionId(rawSessionId);
    if (!isValidSessionId(sessionId)) {
      continue;
    }

    sessions.push({
      sessionId,
      rawSessionId,
      directory: '',
      filePath: path,
      modifiedAt: fileStat.mtime,
    });
  }
}

export function scanCodexSessionsDirectory(options: CodexScanOptions = {}): CodexSessionFile[] {
  if (!existsSync(CODEX_SESSIONS)) {
    logger.warn('Codex sessions directory not found');
    return [];
  }

  const cutoffTime = options.maxAgeDays
    ? Date.now() - options.maxAgeDays * 24 * 60 * 60 * 1000
    : 0;
  const sessions: CodexSessionFile[] = [];
  scanDirectoryRecursive(CODEX_SESSIONS, cutoffTime, sessions);
  return sessions;
}

async function getOrParseCodexSession(
  file: CodexSessionFile,
  cache: Map<string, CodexSessionCache>,
  includeToolCalls: boolean
): Promise<CodexParsedSessionData | null> {
  const fileModTime = file.modifiedAt.getTime();
  const cached = cache.get(file.filePath);
  if (cached && cached.modifiedAt === fileModTime) {
    if (!includeToolCalls || cached.includeToolCalls) {
      return cached.data;
    }
  }

  const parsed = await parseCodexSessionFile(file.filePath, { includeToolCalls });
  cache.set(file.filePath, {
    data: parsed,
    modifiedAt: fileModTime,
    includeToolCalls,
  });
  return parsed;
}

export async function getAllCodexSessions(
  options: CodexScanOptions = {}
): Promise<CodexParsedSessionData[]> {
  const sessionFiles = scanCodexSessionsDirectory(options);
  const includeToolCalls = options.includeToolCalls ?? false;
  const sessions: CodexParsedSessionData[] = [];
  const parseFailures: string[] = [];
  const seenFilePaths = new Set<string>();

  for (const file of sessionFiles) {
    seenFilePaths.add(file.filePath);
    try {
      const parsed = await getOrParseCodexSession(file, sessionSummaryCache, includeToolCalls);
      if (parsed) {
        sessions.push(parsed);
      }
    } catch {
      parseFailures.push(file.filePath);
      sessionSummaryCache.set(file.filePath, {
        data: null,
        modifiedAt: file.modifiedAt.getTime(),
        includeToolCalls,
      });
    }
  }

  for (const cache of [sessionSummaryCache, sessionDetailCache]) {
    for (const cachedPath of cache.keys()) {
      if (!seenFilePaths.has(cachedPath)) {
        cache.delete(cachedPath);
      }
    }
  }

  if (parseFailures.length > 0) {
    logger.warn('Skipped invalid Codex session files during scan', {
      count: parseFailures.length,
      sample: parseFailures.slice(0, 5),
    });
  }

  return sessions;
}

export async function getCodexSessionById(
  sessionId: string
): Promise<CodexParsedSessionData | null> {
  const scopedSessionId = scopeCodexSessionId(sessionId);
  const sessionFile = scanCodexSessionsDirectory()
    .find((file) => file.sessionId === scopedSessionId);
  if (!sessionFile) {
    return null;
  }

  return getOrParseCodexSession(sessionFile, sessionDetailCache, true);
}
