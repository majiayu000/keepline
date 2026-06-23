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
  sessionsDir?: string;
}

export interface CodexSessionScanFailure {
  filePath: string;
  message: string;
}

export interface CodexSessionScanResult {
  sessions: CodexParsedSessionData[];
  failures: CodexSessionScanFailure[];
}

interface CodexSessionCache {
  data: CodexParsedSessionData | null;
  modifiedAt: number;
  includeToolCalls: boolean;
  failureMessage?: string;
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
  sessions: CodexSessionFile[],
  readFailures: string[]
): void {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    readFailures.push(dir);
    return;
  }

  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      scanDirectoryRecursive(path, cutoffTime, sessions, readFailures);
      continue;
    }

    if (!entry.isFile() || !entry.name.endsWith('.jsonl') || !entry.name.startsWith('rollout-')) {
      continue;
    }

    let fileStat;
    try {
      fileStat = statSync(path);
    } catch {
      readFailures.push(path);
      continue;
    }
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
  const sessionsDir = options.sessionsDir ?? CODEX_SESSIONS;
  if (!existsSync(sessionsDir)) {
    logger.debug('Codex sessions directory not found, skipping scan', { path: sessionsDir });
    return [];
  }

  const cutoffTime = options.maxAgeDays
    ? Date.now() - options.maxAgeDays * 24 * 60 * 60 * 1000
    : 0;
  const sessions: CodexSessionFile[] = [];
  const readFailures: string[] = [];
  scanDirectoryRecursive(sessionsDir, cutoffTime, sessions, readFailures);
  if (readFailures.length > 0) {
    logger.warn('Skipped unreadable Codex session paths during scan', {
      count: readFailures.length,
      sample: readFailures.slice(0, 5),
    });
  }
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

async function scanAllCodexSessionsWithFailures(
  options: CodexScanOptions = {}
): Promise<CodexSessionScanResult> {
  const sessionFiles = scanCodexSessionsDirectory(options);
  const includeToolCalls = options.includeToolCalls ?? false;
  const sessions: CodexParsedSessionData[] = [];
  const failures: CodexSessionScanFailure[] = [];
  const freshFailures: CodexSessionScanFailure[] = [];
  const seenFilePaths = new Set<string>();

  for (const file of sessionFiles) {
    seenFilePaths.add(file.filePath);
    try {
      const cached = sessionSummaryCache.get(file.filePath);
      const fileModTime = file.modifiedAt.getTime();
      if (cached && cached.modifiedAt === fileModTime) {
        if (!includeToolCalls || cached.includeToolCalls) {
          if (cached.failureMessage) {
            failures.push({
              filePath: file.filePath,
              message: cached.failureMessage,
            });
            continue;
          }
        }
      }

      const parsed = await getOrParseCodexSession(file, sessionSummaryCache, includeToolCalls);
      if (parsed) {
        sessions.push(parsed);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const failure = {
        filePath: file.filePath,
        message,
      };
      failures.push(failure);
      freshFailures.push(failure);
      sessionSummaryCache.set(file.filePath, {
        data: null,
        modifiedAt: file.modifiedAt.getTime(),
        includeToolCalls,
        failureMessage: message,
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

  if (freshFailures.length > 0) {
    logger.warn('Skipped invalid Codex session files during scan', {
      count: freshFailures.length,
      sample: freshFailures.slice(0, 5).map((failure) => failure.filePath),
    });
  }

  return { sessions, failures };
}

export async function getAllCodexSessions(
  options: CodexScanOptions = {}
): Promise<CodexParsedSessionData[]> {
  const result = await scanAllCodexSessionsWithFailures(options);
  return result.sessions;
}

export async function getAllCodexSessionsWithFailures(
  options: CodexScanOptions = {}
): Promise<CodexSessionScanResult> {
  const result = await scanAllCodexSessionsWithFailures(options);
  return result;
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
