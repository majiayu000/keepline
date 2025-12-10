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
import { CLAUDE_PROJECTS, projectNameToDir, extractSessionId } from '../utils/paths.js';
import { parseSessionFile } from './parser/jsonl.js';
import type { ClaudeSessionFile } from '../core/types.js';
import type { ParsedSessionData } from './types.js';
import { logger } from '../utils/logger.js';

// Cache for parsed session data with modification times
interface SessionCache {
  data: ParsedSessionData;
  modifiedAt: number; // File modification time in ms
}
const sessionCache = new Map<string, SessionCache>();

/** Clear the session cache */
export function clearSessionCache(): void {
  sessionCache.clear();
  logger.debug('Session cache cleared');
}

/** Get cache stats for debugging */
export function getSessionCacheStats(): { size: number; keys: string[] } {
  return {
    size: sessionCache.size,
    keys: Array.from(sessionCache.keys()),
  };
}

/** Scan projects directory for session files */
export function scanProjectsDirectory(): ClaudeSessionFile[] {
  if (!existsSync(CLAUDE_PROJECTS)) {
    logger.warn('Claude projects directory not found');
    return [];
  }

  const sessions: ClaudeSessionFile[] = [];
  const projectDirs = readdirSync(CLAUDE_PROJECTS);

  for (const projectName of projectDirs) {
    const projectPath = join(CLAUDE_PROJECTS, projectName);
    const stat = statSync(projectPath);

    if (!stat.isDirectory()) continue;

    // Skip agent- prefixed directories (subagent data)
    const files = readdirSync(projectPath).filter(
      (f) => f.endsWith('.jsonl') && !f.startsWith('agent-')
    );

    for (const file of files) {
      const filePath = join(projectPath, file);
      const fileStat = statSync(filePath);

      sessions.push({
        sessionId: extractSessionId(file),
        directory: projectNameToDir(projectName),
        filePath,
        modifiedAt: fileStat.mtime,
      });
    }
  }

  return sessions;
}

/** Get all sessions with parsed data (optimized with caching) */
export async function getAllSessions(): Promise<ParsedSessionData[]> {
  const sessionFiles = scanProjectsDirectory();
  const sessions: ParsedSessionData[] = [];
  let cacheHits = 0;
  let cacheMisses = 0;

  // Track which files we've seen to clean up stale cache entries
  const seenFilePaths = new Set<string>();

  for (const file of sessionFiles) {
    seenFilePaths.add(file.filePath);

    try {
      const cached = sessionCache.get(file.filePath);
      const fileModTime = file.modifiedAt.getTime();

      // Use cache if file hasn't been modified
      if (cached && cached.modifiedAt === fileModTime) {
        sessions.push(cached.data);
        cacheHits++;
        continue;
      }

      // Parse file and update cache
      const parsed = await parseSessionFile(file.filePath);
      if (parsed) {
        sessionCache.set(file.filePath, {
          data: parsed,
          modifiedAt: fileModTime,
        });
        sessions.push(parsed);
        cacheMisses++;
      }
    } catch (error) {
      logger.error(`Failed to parse session file: ${file.filePath}`, error);
    }
  }

  // Clean up cache entries for files that no longer exist
  for (const cachedPath of sessionCache.keys()) {
    if (!seenFilePaths.has(cachedPath)) {
      sessionCache.delete(cachedPath);
      logger.debug(`Removed stale cache entry: ${cachedPath}`);
    }
  }

  logger.debug(`Session scan: ${cacheHits} cache hits, ${cacheMisses} misses`);
  return sessions;
}

/** Helper to get or parse session with caching */
async function getOrParseSession(file: ClaudeSessionFile): Promise<ParsedSessionData | null> {
  const cached = sessionCache.get(file.filePath);
  const fileModTime = file.modifiedAt.getTime();

  if (cached && cached.modifiedAt === fileModTime) {
    return cached.data;
  }

  const parsed = await parseSessionFile(file.filePath);
  if (parsed) {
    sessionCache.set(file.filePath, {
      data: parsed,
      modifiedAt: fileModTime,
    });
  }
  return parsed;
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
      const parsed = await getOrParseSession(file);
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
