/**
 * History file parser for Claude
 */

import { createReadStream, existsSync } from 'fs';
import { createInterface } from 'readline';
import { CLAUDE_HISTORY } from '../../utils/paths.js';
import type { ClaudeHistoryEntry } from '../types.js';

/** Parse history.jsonl file */
export async function parseHistoryFile(): Promise<ClaudeHistoryEntry[]> {
  if (!existsSync(CLAUDE_HISTORY)) {
    return [];
  }

  const entries: ClaudeHistoryEntry[] = [];

  const fileStream = createReadStream(CLAUDE_HISTORY);
  const rl = createInterface({
    input: fileStream,
    crlfDelay: Infinity,
  });

  for await (const line of rl) {
    if (!line.trim()) continue;

    try {
      const entry = JSON.parse(line) as ClaudeHistoryEntry;
      entries.push(entry);
    } catch {
      // Skip invalid lines
      continue;
    }
  }

  return entries;
}

/** Get recent history entries for a project */
export async function getProjectHistory(
  projectDir: string,
  limit = 10
): Promise<ClaudeHistoryEntry[]> {
  const allEntries = await parseHistoryFile();

  return allEntries
    .filter((entry) => entry.project === projectDir)
    .sort((a, b) => b.timestamp - a.timestamp)
    .slice(0, limit);
}

/** Get most recent session ID for a project */
export async function getMostRecentSessionId(
  projectDir: string
): Promise<string | undefined> {
  const history = await getProjectHistory(projectDir, 1);
  return history[0]?.sessionId;
}

/** Group history by session */
export async function groupHistoryBySession(): Promise<Map<string, ClaudeHistoryEntry[]>> {
  const allEntries = await parseHistoryFile();
  const grouped = new Map<string, ClaudeHistoryEntry[]>();

  for (const entry of allEntries) {
    const existing = grouped.get(entry.sessionId) || [];
    existing.push(entry);
    grouped.set(entry.sessionId, existing);
  }

  return grouped;
}
