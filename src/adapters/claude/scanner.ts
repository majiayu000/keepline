/**
 * Claude projects directory scanner
 *
 * Optimized with file modification time caching:
 * - Tracks file modification times
 * - Only re-parses files that have changed
 * - Returns cached data for unchanged files
 */

import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'fs';
import { join } from 'path';
import {
  CLAUDE_HUB_PARSE_FAILURE_CACHE,
  CLAUDE_PROJECT_ROOTS,
  ensureClaudeHubParent,
  extractSessionId,
  projectNameToDir,
} from '../../lib/paths.js';
import { parseSessionFile } from './parser/jsonl.js';
import type { ClaudeSessionFile } from '../../domain/session/index.js';
import type { ParsedSessionData } from './types.js';
import { logger } from '../../lib/logger.js';
import { isValidSessionId } from '../../lib/session-id.js';

// Cache for parsed session data with modification times
interface SessionCache {
  data: ParsedSessionData | null;
  modifiedAt: number; // File modification time in ms
  includeToolCalls: boolean;
}
const sessionSummaryCache = new Map<string, SessionCache>();
const sessionDetailCache = new Map<string, SessionCache>();
const persistedParseFailures = new Map<string, number>();
let persistedParseFailuresLoaded = false;
let persistedParseFailuresDirty = false;

function loadPersistedParseFailures(): void {
  if (persistedParseFailuresLoaded) {
    return;
  }

  persistedParseFailuresLoaded = true;
  if (!existsSync(CLAUDE_HUB_PARSE_FAILURE_CACHE)) {
    return;
  }

  try {
    const raw = readFileSync(CLAUDE_HUB_PARSE_FAILURE_CACHE, 'utf-8');
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    for (const [filePath, modifiedAt] of Object.entries(parsed)) {
      if (typeof modifiedAt === 'number') {
        persistedParseFailures.set(filePath, modifiedAt);
      }
    }
  } catch (error) {
    logger.warn('Failed to load persisted parse failure cache', {
      error: (error as Error).message,
    });
  }
}

function flushPersistedParseFailures(): void {
  if (!persistedParseFailuresDirty) {
    return;
  }

  ensureClaudeHubParent(CLAUDE_HUB_PARSE_FAILURE_CACHE);
  writeFileSync(
    CLAUDE_HUB_PARSE_FAILURE_CACHE,
    JSON.stringify(Object.fromEntries(persistedParseFailures), null, 2)
  );
  persistedParseFailuresDirty = false;
}

function isPersistedParseFailure(filePath: string, modifiedAt: number): boolean {
  loadPersistedParseFailures();
  return persistedParseFailures.get(filePath) === modifiedAt;
}

function recordPersistedParseFailure(filePath: string, modifiedAt: number): void {
  loadPersistedParseFailures();
  if (persistedParseFailures.get(filePath) === modifiedAt) {
    return;
  }
  persistedParseFailures.set(filePath, modifiedAt);
  persistedParseFailuresDirty = true;
}

function clearPersistedParseFailure(filePath: string): void {
  loadPersistedParseFailures();
  if (persistedParseFailures.delete(filePath)) {
    persistedParseFailuresDirty = true;
  }
}

/** Clear the session cache */
export function clearSessionCache(): void {
  sessionSummaryCache.clear();
  sessionDetailCache.clear();
  logger.debug('Session cache cleared');
}

/** Get cache stats for debugging */
export function getSessionCacheStats(): { size: number; keys: string[] } {
  return {
    size: sessionSummaryCache.size + sessionDetailCache.size,
    keys: [
      ...Array.from(sessionSummaryCache.keys()).map((key) => `summary:${key}`),
      ...Array.from(sessionDetailCache.keys()).map((key) => `detail:${key}`),
    ],
  };
}

/** Options for scanning session files */
export interface ScanOptions {
  includeSubAgents?: boolean; // Include agent- prefixed files (default: false for backward compatibility)
  includeToolCalls?: boolean; // Include full tool call details in bulk parsed results (default: false)
  maxAgeDays?: number; // Only include files modified within this many days (default: all)
}

/** Scan projects directory for session files */
export function scanProjectsDirectory(options: ScanOptions = {}): ClaudeSessionFile[] {
  const { includeSubAgents = false, maxAgeDays } = options;

  // Map keyed by `${projectName}:${sessionId}` for O(1) cross-root dedup
  const byKey = new Map<string, ClaudeSessionFile>();
  const invalidSessionFiles: string[] = [];
  const cutoffTime = maxAgeDays ? Date.now() - maxAgeDays * 24 * 60 * 60 * 1000 : 0;
  let scannedRoots = 0;

  for (const root of CLAUDE_PROJECT_ROOTS) {
    if (!existsSync(root)) continue;
    scannedRoots++;

    const projectDirs = readdirSync(root, { withFileTypes: true });

    for (const projectEntry of projectDirs) {
      if (!projectEntry.isDirectory()) continue;

      const projectName = projectEntry.name;
      const projectPath = join(root, projectName);

      // Skip directories that haven't been modified recently (optimization)
      if (maxAgeDays) {
        const stat = statSync(projectPath);
        if (stat.mtime.getTime() < cutoffTime) {
          continue;
        }
      }

      const files = readdirSync(projectPath, { withFileTypes: true });

      for (const fileEntry of files) {
        if (!fileEntry.isFile()) continue;

        const file = fileEntry.name;
        if (!file.endsWith('.jsonl')) continue;

        const isAgentFile = file.startsWith('agent-');
        if (!includeSubAgents && isAgentFile) continue;

        const filePath = join(projectPath, file);
        const fileStat = statSync(filePath);

        if (maxAgeDays && fileStat.mtime.getTime() < cutoffTime) {
          continue;
        }

        const sessionId = isAgentFile
          ? file.replace('.jsonl', '')
          : extractSessionId(file);

        if (!isValidSessionId(sessionId)) {
          invalidSessionFiles.push(filePath);
          continue;
        }

        // De-duplicate when the same sessionId exists in multiple roots
        // (e.g. .claude and .claude-work); prefer the newer file.
        const dedupeKey = `${projectName}:${sessionId}`;
        const existing = byKey.get(dedupeKey);
        if (existing && existing.modifiedAt.getTime() >= fileStat.mtime.getTime()) {
          continue;
        }

        byKey.set(dedupeKey, {
          sessionId,
          directory: projectNameToDir(projectName),
          filePath,
          modifiedAt: fileStat.mtime,
        });
      }
    }
  }

  if (scannedRoots === 0) {
    logger.warn('No Claude projects directories found', {
      roots: CLAUDE_PROJECT_ROOTS,
    });
  }

  if (invalidSessionFiles.length > 0) {
    logger.warn('Skipped session files with invalid session IDs', {
      count: invalidSessionFiles.length,
      sample: invalidSessionFiles.slice(0, 5),
    });
  }

  return Array.from(byKey.values());
}

function requireValidParsedSession(
  parsed: ParsedSessionData | null,
  filePath: string
): ParsedSessionData | null {
  if (parsed && !isValidSessionId(parsed.sessionId)) {
    throw new Error(`Invalid session ID in session file: ${filePath}`);
  }

  return parsed;
}

/** Get all sessions with parsed data (optimized with caching) */
export async function getAllSessions(options: ScanOptions = {}): Promise<ParsedSessionData[]> {
  const sessionFiles = scanProjectsDirectory(options);
  const includeToolCalls = options.includeToolCalls ?? false;
  const sessions: ParsedSessionData[] = [];
  let cacheHits = 0;
  let cacheMisses = 0;
  const parseFailures: string[] = [];

  // Track which files we've seen to clean up stale cache entries
  const seenFilePaths = new Set<string>();

  for (const file of sessionFiles) {
    seenFilePaths.add(file.filePath);

    try {
      const cached = sessionSummaryCache.get(file.filePath);
      const fileModTime = file.modifiedAt.getTime();

      // Use cache if file hasn't been modified
      if (cached && cached.modifiedAt === fileModTime) {
        if (!includeToolCalls || cached.includeToolCalls) {
          if (cached.data) {
            sessions.push(cached.data);
          }
          cacheHits++;
          continue;
        }
      }

      if (isPersistedParseFailure(file.filePath, fileModTime)) {
        sessionSummaryCache.set(file.filePath, {
          data: null,
          modifiedAt: fileModTime,
          includeToolCalls,
        });
        cacheHits++;
        continue;
      }

      // Parse file and update cache
      const parsed = requireValidParsedSession(
        await parseSessionFile(file.filePath, { includeToolCalls }),
        file.filePath
      );
      sessionSummaryCache.set(file.filePath, {
        data: parsed,
        modifiedAt: fileModTime,
        includeToolCalls,
      });
      clearPersistedParseFailure(file.filePath);
      if (parsed) {
        sessions.push(parsed);
      }
      cacheMisses++;
    } catch (error) {
      sessionSummaryCache.set(file.filePath, {
        data: null,
        modifiedAt: file.modifiedAt.getTime(),
        includeToolCalls,
      });
      recordPersistedParseFailure(file.filePath, file.modifiedAt.getTime());
      parseFailures.push(file.filePath);
    }
  }

  // Clean up cache entries for files that no longer exist
  for (const [cacheName, cache] of [['summary', sessionSummaryCache], ['detail', sessionDetailCache]] as const) {
    for (const cachedPath of cache.keys()) {
      if (!seenFilePaths.has(cachedPath)) {
        cache.delete(cachedPath);
        logger.debug(`Removed stale ${cacheName} cache entry: ${cachedPath}`);
      }
    }
  }

  loadPersistedParseFailures();
  for (const cachedPath of [...persistedParseFailures.keys()]) {
    if (!seenFilePaths.has(cachedPath)) {
      persistedParseFailures.delete(cachedPath);
      persistedParseFailuresDirty = true;
    }
  }
  flushPersistedParseFailures();

  if (parseFailures.length > 0) {
    logger.warn('Skipped invalid session files during scan', {
      count: parseFailures.length,
      sample: parseFailures.slice(0, 5),
    });
  }

  logger.debug(`Session scan: ${cacheHits} cache hits, ${cacheMisses} misses`);
  return sessions;
}

/** Helper to get or parse session with caching */
async function getOrParseSession(file: ClaudeSessionFile): Promise<ParsedSessionData | null> {
  const cached = sessionDetailCache.get(file.filePath);
  const fileModTime = file.modifiedAt.getTime();

  if (cached && cached.modifiedAt === fileModTime) {
    return cached.data;
  }

  if (isPersistedParseFailure(file.filePath, fileModTime)) {
    sessionDetailCache.set(file.filePath, {
      data: null,
      modifiedAt: fileModTime,
      includeToolCalls: true,
    });
    return null;
  }

  try {
    const parsed = requireValidParsedSession(
      await parseSessionFile(file.filePath, { includeToolCalls: true }),
      file.filePath
    );
    sessionDetailCache.set(file.filePath, {
      data: parsed,
      modifiedAt: fileModTime,
      includeToolCalls: true,
    });
    clearPersistedParseFailure(file.filePath);
    flushPersistedParseFailures();
    return parsed;
  } catch (error) {
    sessionDetailCache.set(file.filePath, {
      data: null,
      modifiedAt: fileModTime,
      includeToolCalls: true,
    });
    recordPersistedParseFailure(file.filePath, fileModTime);
    flushPersistedParseFailures();
    throw error;
  }
}

async function getOrParseSessionSummary(file: ClaudeSessionFile): Promise<ParsedSessionData | null> {
  const cached = sessionSummaryCache.get(file.filePath);
  const fileModTime = file.modifiedAt.getTime();

  if (cached && cached.modifiedAt === fileModTime) {
    return cached.data;
  }

  if (isPersistedParseFailure(file.filePath, fileModTime)) {
    sessionSummaryCache.set(file.filePath, {
      data: null,
      modifiedAt: fileModTime,
      includeToolCalls: false,
    });
    return null;
  }

  try {
    const parsed = requireValidParsedSession(
      await parseSessionFile(file.filePath, { includeToolCalls: false }),
      file.filePath
    );
    sessionSummaryCache.set(file.filePath, {
      data: parsed,
      modifiedAt: fileModTime,
      includeToolCalls: false,
    });
    clearPersistedParseFailure(file.filePath);
    flushPersistedParseFailures();
    return parsed;
  } catch (error) {
    sessionSummaryCache.set(file.filePath, {
      data: null,
      modifiedAt: fileModTime,
      includeToolCalls: false,
    });
    recordPersistedParseFailure(file.filePath, fileModTime);
    flushPersistedParseFailures();
    throw error;
  }
}

/** Get session by ID (uses cache) */
export async function getSessionById(
  sessionId: string
): Promise<ParsedSessionData | null> {
  const sessionFiles = scanProjectsDirectory();
  const file = sessionFiles.find((f) => f.sessionId === sessionId);

  if (!file) return null;

  return getOrParseSession(file);
}

/** Get sessions for a specific directory (uses cache) */
export async function getSessionsByDirectory(
  directory: string
): Promise<ParsedSessionData[]> {
  const sessionFiles = scanProjectsDirectory();
  const filtered = sessionFiles.filter((f) => f.directory === directory);
  const sessions: ParsedSessionData[] = [];

  for (const file of filtered) {
    try {
      const parsed = await getOrParseSessionSummary(file);
      if (parsed) {
        sessions.push(parsed);
      }
    } catch (error) {
      logger.error(`Failed to parse session file: ${file.filePath}`, error);
    }
  }

  return sessions;
}

/** Get most recently modified sessions */
export function getRecentSessionFiles(limit = 20): ClaudeSessionFile[] {
  const sessions = scanProjectsDirectory();
  return sessions
    .sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime())
    .slice(0, limit);
}
