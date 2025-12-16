/**
 * Path utilities for Tasker
 */

import { homedir } from 'os';
import { join } from 'path';

/** Claude base directory */
export const CLAUDE_HOME = join(homedir(), '.claude');

/** Claude projects directory */
export const CLAUDE_PROJECTS = join(CLAUDE_HOME, 'projects');

/** Claude history file */
export const CLAUDE_HISTORY = join(CLAUDE_HOME, 'history.jsonl');

/** Claude settings file */
export const CLAUDE_SETTINGS = join(CLAUDE_HOME, 'settings.json');

/** Tasker data directory */
export const TASKER_HOME = join(homedir(), '.tasker');

/** Tasker database file */
export const TASKER_DB = join(TASKER_HOME, 'tasker.db');

/** Tasker log file */
export const TASKER_LOG = join(TASKER_HOME, 'tasker.log');

/** Tasker PID file for daemon */
export const TASKER_PID = join(TASKER_HOME, 'tasker.pid');

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
