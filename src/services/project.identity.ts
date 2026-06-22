import { createHash } from 'crypto';
import { existsSync, statSync } from 'fs';
import { homedir } from 'os';
import path from 'path';

export type ProjectIdentitySource = 'git-root' | 'cwd' | 'unknown';

export interface ProjectIdentity {
  id: string;
  rootPath: string;
  name: string;
  displayPath: string;
  source: ProjectIdentitySource;
}

export const UNKNOWN_PROJECT_ID = 'unknown';
export const UNKNOWN_PROJECT_ROOT = 'Unknown';

const UNKNOWN_PROJECT: ProjectIdentity = {
  id: UNKNOWN_PROJECT_ID,
  rootPath: UNKNOWN_PROJECT_ROOT,
  name: 'Unknown project',
  displayPath: 'Unknown',
  source: 'unknown',
};

const identityCache = new Map<string, ProjectIdentity>();

export function clearProjectIdentityCache(): void {
  identityCache.clear();
}

export function normalizeProjectPath(rawPath: string | undefined | null): string | null {
  const trimmed = rawPath?.trim();
  if (!trimmed) return null;

  const expanded = trimmed === '~'
    ? homedir()
    : trimmed.startsWith(`~${path.sep}`)
      ? path.join(homedir(), trimmed.slice(2))
      : trimmed;

  const resolved = path.isAbsolute(expanded)
    ? path.normalize(expanded)
    : path.resolve(expanded);

  const parsed = path.parse(resolved);
  if (resolved === parsed.root) return resolved;
  return resolved.replace(new RegExp(`${escapeRegExp(path.sep)}+$`), '') || parsed.root;
}

export function projectIdFromPath(projectPath: string | undefined | null): string {
  const normalized = normalizeProjectPath(projectPath);
  if (!normalized) return UNKNOWN_PROJECT.id;
  return `path-${createHash('sha256').update(normalized).digest('hex').slice(0, 16)}`;
}

export function displayProjectPath(projectPath: string): string {
  const home = homedir();
  if (projectPath === home) return '~';
  if (projectPath.startsWith(`${home}${path.sep}`)) {
    return `~${path.sep}${projectPath.slice(home.length + 1)}`;
  }
  return projectPath;
}

export function getProjectNameFromPath(projectPath: string): string {
  if (projectPath === UNKNOWN_PROJECT.rootPath) return UNKNOWN_PROJECT.name;
  const base = path.basename(projectPath);
  return base || projectPath;
}

export function resolveProjectIdentity(cwd: string | undefined | null): ProjectIdentity {
  const normalized = normalizeProjectPath(cwd);
  if (!normalized) return UNKNOWN_PROJECT;

  const cached = identityCache.get(normalized);
  if (cached) return cached;

  const gitRoot = findGitRoot(normalized);
  const rootPath = gitRoot ?? normalized;
  const identity: ProjectIdentity = {
    id: projectIdFromPath(rootPath),
    rootPath,
    name: getProjectNameFromPath(rootPath),
    displayPath: displayProjectPath(rootPath),
    source: gitRoot ? 'git-root' : 'cwd',
  };

  identityCache.set(normalized, identity);
  return identity;
}

function findGitRoot(projectPath: string): string | null {
  let current = nearestExistingPath(projectPath);
  if (!current) return null;

  try {
    if (!statSync(current).isDirectory()) {
      current = path.dirname(current);
    }
  } catch {
    return null;
  }

  while (true) {
    if (existsSync(path.join(current, '.git'))) {
      return current;
    }
    const parent = path.dirname(current);
    if (parent === current) return null;
    current = parent;
  }
}

function nearestExistingPath(projectPath: string): string | null {
  let current = projectPath;
  while (!existsSync(current)) {
    const parent = path.dirname(current);
    if (parent === current) return null;
    current = parent;
  }
  return current;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
