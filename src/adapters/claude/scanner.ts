/**
 * Claude projects directory scanner
 *
 * Optimized with file modification time caching:
 * - Tracks file modification times
 * - Only re-parses files that have changed
 * - Returns cached data for unchanged files
 */

import { existsSync, readdirSync, statSync } from 'fs';
import { join } from 'path';
import { CLAUDE_PROJECTS, projectNameToDir, extractSessionId } from '../../lib/paths.js';
import { parseSessionFile } from './parser/jsonl.js';
import type { ClaudeSessionFile } from '../../lib/types.js';
import type { ParsedSessionData } from './types.js';
import { logger } from '../../lib/logger.js';

// Cache for parsed session data with modification times
interface SessionCache {
  data: ParsedSessionData | null;
  modifiedAt: number; // File modification time in ms
}
const sessionSummaryCache = new Map<string, SessionCache>();
const sessionDetailCache = new Map<string, SessionCache>();

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
  maxAgeDays?: number; // Only include files modified within this many days (default: all)
}

/** Scan projects directory for session files */
export function scanProjectsDirectory(options: ScanOptions = {}): ClaudeSessionFile[] {
  const { includeSubAgents = false, maxAgeDays } = options;

  if (!existsSync(CLAUDE_PROJECTS)) {
    logger.warn('Claude projects directory not found');
    return [];
  }

  const sessions: ClaudeSessionFile[] = [];
  const projectDirs = readdirSync(CLAUDE_PROJECTS, { withFileTypes: true });

  // Calculate cutoff date if maxAgeDays is specified
  const cutoffTime = maxAgeDays
    ? Date.now() - (maxAgeDays * 24 * 60 * 60 * 1000)
    : 0;

  for (const projectEntry of projectDirs) {
    if (!projectEntry.isDirectory()) continue;

    const projectName = projectEntry.name;
    const projectPath = join(CLAUDE_PROJECTS, projectName);

    // Skip directories that haven't been modified recently (optimization)
    if (maxAgeDays) {
      const stat = statSync(projectPath);
      if (stat.mtime.getTime() < cutoffTime) {
        continue;
      }
    }

    // Filter files based on options
    const files = readdirSync(projectPath, { withFileTypes: true });

    for (const fileEntry of files) {
      if (!fileEntry.isFile()) continue;

      const file = fileEntry.name;
      if (!file.endsWith('.jsonl')) continue;

      const isAgentFile = file.startsWith('agent-');
      if (!includeSubAgents && isAgentFile) continue;

      const filePath = join(projectPath, file);
      const fileStat = statSync(filePath);

      // Skip files older than cutoff
      if (maxAgeDays && fileStat.mtime.getTime() < cutoffTime) {
        continue;
      }

      // For agent files, use the agent ID as sessionId
      const sessionId = isAgentFile
        ? file.replace('.jsonl', '') // Keep as "agent-xxxx"
        : extractSessionId(file);

      sessions.push({
        sessionId,
        directory: projectNameToDir(projectName),
        filePath,
        modifiedAt: fileStat.mtime,
      });
    }
  }

  return sessions;
}

/** Get all sessions with parsed data (optimized with caching) */
export async function getAllSessions(options: ScanOptions = {}): Promise<ParsedSessionData[]> {
  const sessionFiles = scanProjectsDirectory(options);
  const sessions: ParsedSessionData[] = [];
  let cacheHits = 0;
  let cacheMisses = 0;

  // Track which files we've seen to clean up stale cache entries
  const seenFilePaths = new Set<string>();

  for (const file of sessionFiles) {
    seenFilePaths.add(file.filePath);

    try {
      const cached = sessionSummaryCache.get(file.filePath);
      const fileModTime = file.modifiedAt.getTime();

      // Use cache if file hasn't been modified
      if (cached && cached.modifiedAt === fileModTime) {
        if (cached.data) {
          sessions.push(cached.data);
        }
        cacheHits++;
        continue;
      }

      // Parse file and update cache
      const parsed = await parseSessionFile(file.filePath, { includeToolCalls: false });
      sessionSummaryCache.set(file.filePath, {
        data: parsed,
        modifiedAt: fileModTime,
      });
      if (parsed) {
        sessions.push(parsed);
      }
      cacheMisses++;
    } catch (error) {
      sessionSummaryCache.set(file.filePath, {
        data: null,
        modifiedAt: file.modifiedAt.getTime(),
      });
      logger.error(`Failed to parse session file: ${file.filePath}`, error);
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

  try {
    const parsed = await parseSessionFile(file.filePath, { includeToolCalls: true });
    sessionDetailCache.set(file.filePath, {
      data: parsed,
      modifiedAt: fileModTime,
    });
    return parsed;
  } catch (error) {
    sessionDetailCache.set(file.filePath, {
      data: null,
      modifiedAt: fileModTime,
    });
    throw error;
  }
}

async function getOrParseSessionSummary(file: ClaudeSessionFile): Promise<ParsedSessionData | null> {
  const cached = sessionSummaryCache.get(file.filePath);
  const fileModTime = file.modifiedAt.getTime();

  if (cached && cached.modifiedAt === fileModTime) {
    return cached.data;
  }

  try {
    const parsed = await parseSessionFile(file.filePath, { includeToolCalls: false });
    sessionSummaryCache.set(file.filePath, {
      data: parsed,
      modifiedAt: fileModTime,
    });
    return parsed;
  } catch (error) {
    sessionSummaryCache.set(file.filePath, {
      data: null,
      modifiedAt: fileModTime,
    });
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
