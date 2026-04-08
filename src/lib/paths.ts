/**
 * Path utilities for Claude Hub.
 */

import { cpSync, existsSync, mkdirSync, renameSync, rmSync } from 'fs';
import { homedir } from 'os';
import { dirname, join } from 'path';

/** Claude base directory */
export const CLAUDE_HOME = join(homedir(), '.claude');

/** Claude projects directory */
export const CLAUDE_PROJECTS = join(CLAUDE_HOME, 'projects');

/** Claude history file */
export const CLAUDE_HISTORY = join(CLAUDE_HOME, 'history.jsonl');

/** Claude settings file */
export const CLAUDE_SETTINGS = join(CLAUDE_HOME, 'settings.json');

/** Legacy Tasker data directory */
export const LEGACY_TASKER_HOME = join(homedir(), '.tasker');

/** Claude Hub data directory */
export const CLAUDE_HUB_HOME = process.env.CLAUDE_HUB_HOME || join(homedir(), '.claude-hub');

/** Claude Hub database file */
export const CLAUDE_HUB_DB = join(CLAUDE_HUB_HOME, 'claude-hub.db');

/** Claude Hub log file */
export const CLAUDE_HUB_LOG = join(CLAUDE_HUB_HOME, 'claude-hub.log');

/** Claude Hub PID file for daemon */
export const CLAUDE_HUB_PID = join(CLAUDE_HUB_HOME, 'claude-hub.pid');

/**
 * Backward-compatible aliases. New code should prefer CLAUDE_HUB_* names.
 */
export const TASKER_HOME = CLAUDE_HUB_HOME;
export const TASKER_DB = CLAUDE_HUB_DB;
export const TASKER_LOG = CLAUDE_HUB_LOG;
export const TASKER_PID = CLAUDE_HUB_PID;

export function migrateClaudeHubDataHome(
  legacyHome: string = LEGACY_TASKER_HOME,
  nextHome: string = CLAUDE_HUB_HOME
): void {
  if (nextHome === legacyHome) return;
  if (existsSync(nextHome) || !existsSync(legacyHome)) return;

  try {
    renameSync(legacyHome, nextHome);
  } catch {
    mkdirSync(nextHome, { recursive: true });
    cpSync(legacyHome, nextHome, { recursive: true });
    rmSync(legacyHome, { recursive: true, force: true });
  }
}

/** Ensure the Claude Hub data directory exists, migrating legacy Tasker data if needed. */
export function ensureClaudeHubDataHome(): string {
  migrateClaudeHubDataHome();
  if (!existsSync(CLAUDE_HUB_HOME)) {
    mkdirSync(CLAUDE_HUB_HOME, { recursive: true });
  }
  return CLAUDE_HUB_HOME;
}

/** Ensure the parent directory for a Claude Hub file path exists. */
export function ensureClaudeHubParent(path: string): void {
  ensureClaudeHubDataHome();
  const parent = dirname(path);
  if (!existsSync(parent)) {
    mkdirSync(parent, { recursive: true });
  }
}

/**
 * Convert directory path to Claude project folder name
 * /Users/foo/bar -> -Users-foo-bar
 */
export function dirToProjectName(dir: string): string {
  return dir.replace(/\//g, '-');
}

/**
 * Convert Claude project folder name back to directory path
 * -Users-foo-bar -> /Users/foo/bar
 */
export function projectNameToDir(name: string): string {
  return name.replace(/^-/, '/').replace(/-/g, '/');
}

/**
 * Get project folder path for a directory
 */
export function getProjectFolder(dir: string): string {
  return join(CLAUDE_PROJECTS, dirToProjectName(dir));
}

/**
 * Extract session ID from JSONL filename
 */
export function extractSessionId(filename: string): string {
  return filename.replace('.jsonl', '');
}
