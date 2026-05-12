/**
 * Path utilities for Claude Hub.
 */

import { cpSync, existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync } from 'fs';
import { homedir } from 'os';
import { dirname, join } from 'path';

/** Claude base directory */
export const CLAUDE_HOME = join(homedir(), '.claude');

/** Claude projects directory (primary) */
export const CLAUDE_PROJECTS = join(CLAUDE_HOME, 'projects');

/** Additional Claude data home (claude-work fork / secondary install) */
export const CLAUDE_WORK_HOME = join(homedir(), '.claude-work');

/** Additional Claude projects directory under claude-work */
export const CLAUDE_WORK_PROJECTS = join(CLAUDE_WORK_HOME, 'projects');

/**
 * All Claude project roots to scan.
 * Override via CLAUDE_HUB_PROJECT_ROOTS (colon-separated absolute paths).
 */
export const CLAUDE_PROJECT_ROOTS: string[] = (() => {
  const override = process.env.CLAUDE_HUB_PROJECT_ROOTS;
  if (override) {
    return override.split(':').map((p) => p.trim()).filter(Boolean);
  }
  return [CLAUDE_PROJECTS, CLAUDE_WORK_PROJECTS];
})();

/** Claude history file */
export const CLAUDE_HISTORY = join(CLAUDE_HOME, 'history.jsonl');

/** Claude settings file */
export const CLAUDE_SETTINGS = join(CLAUDE_HOME, 'settings.json');

/** Legacy Tasker data directory */
export const LEGACY_TASKER_HOME = join(homedir(), '.tasker');

/**
 * Resolve the Claude Hub data directory from the current environment.
 *
 * Re-reads `process.env.CLAUDE_HUB_HOME` every call so a CLI flag that
 * sets the env var after this module is imported still takes effect.
 */
export function getClaudeHubHome(): string {
  return process.env.CLAUDE_HUB_HOME || join(homedir(), '.claude-hub');
}

/** Claude Hub database file (re-resolved each call). */
export function getClaudeHubDb(): string {
  return join(getClaudeHubHome(), 'claude-hub.db');
}

/** Claude Hub log file (re-resolved each call). */
export function getClaudeHubLog(): string {
  return join(getClaudeHubHome(), 'claude-hub.log');
}

/** Claude Hub PID file for daemon (re-resolved each call). */
export function getClaudeHubPid(): string {
  return join(getClaudeHubHome(), 'claude-hub.pid');
}

/**
 * Module-load-time snapshot of the data directory.
 *
 * Kept for back-compat with consumers that read it as a constant. New code
 * should prefer `getClaudeHubHome()` so a runtime env override is honored.
 */
export const CLAUDE_HUB_HOME = getClaudeHubHome();

/** Claude Hub persisted parse-failure cache */
export const CLAUDE_HUB_PARSE_FAILURE_CACHE = join(CLAUDE_HUB_HOME, 'invalid-session-files.json');

/** Claude Hub database file (load-time snapshot). */
export const CLAUDE_HUB_DB = getClaudeHubDb();

/** Claude Hub log file (load-time snapshot). */
export const CLAUDE_HUB_LOG = getClaudeHubLog();

/** Claude Hub PID file (load-time snapshot). */
export const CLAUDE_HUB_PID = getClaudeHubPid();

/**
 * Recursively walk a directory and return [fileCount, totalBytes].
 * Used to verify a copy succeeded before deleting the source.
 */
function summarizeTree(root: string): { files: number; bytes: number } {
  let files = 0;
  let bytes = 0;
  const walk = (dir: string): void => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(path);
      } else if (entry.isFile()) {
        files += 1;
        bytes += statSync(path).size;
      }
    }
  };
  walk(root);
  return { files, bytes };
}

export function migrateClaudeHubDataHome(
  legacyHome: string = LEGACY_TASKER_HOME,
  nextHome: string = getClaudeHubHome()
): void {
  if (nextHome === legacyHome) return;
  if (existsSync(nextHome) || !existsSync(legacyHome)) return;

  try {
    renameSync(legacyHome, nextHome);
    return;
  } catch {
    // Cross-device or permission failure; fall back to copy + verify + delete.
  }

  mkdirSync(nextHome, { recursive: true });
  cpSync(legacyHome, nextHome, { recursive: true });

  // Verify the copy is complete before deleting the legacy directory.
  // A partial copy (disk full, permission error mid-tree) can succeed enough
  // to not throw but still drop files; deleting the legacy tree at that point
  // would lose the only complete copy of user data.
  const before = summarizeTree(legacyHome);
  const after = summarizeTree(nextHome);
  if (after.files !== before.files || after.bytes !== before.bytes) {
    throw new Error(
      `Aborting migration: copy verification failed. ` +
        `Source ${legacyHome} has ${before.files} files / ${before.bytes} bytes; ` +
        `destination ${nextHome} has ${after.files} files / ${after.bytes} bytes. ` +
        `Legacy directory left in place.`
    );
  }

  rmSync(legacyHome, { recursive: true, force: true });
}

/** Ensure the Claude Hub data directory exists, migrating legacy Tasker data if needed. */
export function ensureClaudeHubDataHome(): string {
  const home = getClaudeHubHome();
  migrateClaudeHubDataHome(LEGACY_TASKER_HOME, home);
  if (!existsSync(home)) {
    mkdirSync(home, { recursive: true });
  }
  return home;
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
 * Get project folder path for a directory.
 * Searches every configured Claude project root; returns the first existing
 * match, falling back to the primary root if none exist yet.
 */
export function getProjectFolder(dir: string): string {
  const folderName = dirToProjectName(dir);
  for (const root of CLAUDE_PROJECT_ROOTS) {
    const candidate = join(root, folderName);
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  return join(CLAUDE_PROJECT_ROOTS[0] ?? CLAUDE_PROJECTS, folderName);
}

/**
 * Extract session ID from JSONL filename
 */
export function extractSessionId(filename: string): string {
  return filename.replace('.jsonl', '');
}
