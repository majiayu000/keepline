/**
 * Project root normalization for runtime-neutral scans.
 */

import { existsSync, realpathSync, statSync } from 'fs';
import { dirname, join, resolve } from 'path';
import { homedir } from 'os';
import type { RuntimeSession } from '../../domain/runtime/index.js';

function expandHome(inputPath: string): string {
  if (inputPath === '~') return homedir();
  if (inputPath.startsWith('~/')) return join(homedir(), inputPath.slice(2));
  return inputPath;
}

function safeRealpath(inputPath: string): string {
  const resolved = resolve(expandHome(inputPath));
  try {
    return realpathSync(resolved);
  } catch {
    return resolved;
  }
}

function safeIsDirectory(inputPath: string): boolean | undefined {
  try {
    return statSync(inputPath).isDirectory();
  } catch {
    return undefined;
  }
}

export function resolveProjectRoot(inputPath: string): string {
  const resolved = safeRealpath(inputPath);
  const isDirectory = safeIsDirectory(resolved);
  if (isDirectory === undefined) {
    return resolved;
  }

  let current = isDirectory ? resolved : dirname(resolved);
  while (true) {
    if (existsSync(join(current, '.git'))) {
      return current;
    }

    const parent = dirname(current);
    if (parent === current) {
      return resolved;
    }
    current = parent;
  }
}

export function withResolvedProjectRoots(sessions: RuntimeSession[]): RuntimeSession[] {
  const rootByPath = new Map<string, string>();

  return sessions.map((session) => {
    const sourcePath = session.projectRoot ?? session.cwd;
    let projectRoot = rootByPath.get(sourcePath);
    if (!projectRoot) {
      projectRoot = resolveProjectRoot(sourcePath);
      rootByPath.set(sourcePath, projectRoot);
    }

    return { ...session, projectRoot };
  });
}

export function filterSessionsByProjectRoot(
  sessions: RuntimeSession[],
  projectRoot: string
): RuntimeSession[] {
  const normalizedProjectRoot = resolveProjectRoot(projectRoot);
  return sessions.filter((session) => session.projectRoot === normalizedProjectRoot);
}
