/**
 * Claude projects directory scanner
 */

import { existsSync, readdirSync, statSync } from 'fs';
import { join } from 'path';
import { CLAUDE_PROJECTS, projectNameToDir, extractSessionId } from '../utils/paths.js';
import { parseSessionFile } from './parser/jsonl.js';
import type { ClaudeSessionFile } from '../core/types.js';
import type { ParsedSessionData } from './types.js';
import { logger } from '../utils/logger.js';

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

/** Get all sessions with parsed data */
export async function getAllSessions(): Promise<ParsedSessionData[]> {
  const sessionFiles = scanProjectsDirectory();
  const sessions: ParsedSessionData[] = [];

  for (const file of sessionFiles) {
    try {
      const parsed = await parseSessionFile(file.filePath);
      if (parsed) {
        sessions.push(parsed);
      }
    } catch (error) {
      logger.error(`Failed to parse session file: ${file.filePath}`, error);
    }
  }

  return sessions;
}

/** Get session by ID */
export async function getSessionById(
  sessionId: string
): Promise<ParsedSessionData | null> {
  const sessionFiles = scanProjectsDirectory();
  const file = sessionFiles.find((f) => f.sessionId === sessionId);

  if (!file) return null;

  return parseSessionFile(file.filePath);
}

/** Get sessions for a specific directory */
export async function getSessionsByDirectory(
  directory: string
): Promise<ParsedSessionData[]> {
  const sessionFiles = scanProjectsDirectory();
  const filtered = sessionFiles.filter((f) => f.directory === directory);
  const sessions: ParsedSessionData[] = [];

  for (const file of filtered) {
    try {
      const parsed = await parseSessionFile(file.filePath);
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
