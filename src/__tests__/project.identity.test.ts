import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import {
  clearProjectIdentityCache,
  projectIdFromPath,
  resolveProjectIdentity,
} from '../services/project.identity.js';

const tmpRoots: string[] = [];

function makeTempRoot(prefix: string): string {
  const root = mkdtempSync(join(tmpdir(), prefix));
  tmpRoots.push(root);
  return root;
}

afterEach(() => {
  clearProjectIdentityCache();
  for (const root of tmpRoots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe('Project identity', () => {
  test('resolves nested directories to the git root', () => {
    const root = makeTempRoot('keepline-project-root-');
    mkdirSync(join(root, '.git'));
    const nested = join(root, 'packages', 'app');
    mkdirSync(nested, { recursive: true });

    const identity = resolveProjectIdentity(nested);

    expect(identity.rootPath).toBe(root);
    expect(identity.source).toBe('git-root');
    expect(identity.name).toBe(root.split('/').pop()!);
  });

  test('treats .git files as worktree roots', () => {
    const root = makeTempRoot('keepline-worktree-root-');
    writeFileSync(join(root, '.git'), 'gitdir: /tmp/example/.git/worktrees/demo');
    const nested = join(root, 'src');
    mkdirSync(nested);

    const identity = resolveProjectIdentity(nested);

    expect(identity.rootPath).toBe(root);
    expect(identity.source).toBe('git-root');
  });

  test('keeps missing historical directories filterable by cwd', () => {
    const missing = join(tmpdir(), 'keepline-missing-worktree', 'app');

    const identity = resolveProjectIdentity(missing);

    expect(identity.rootPath).toBe(missing);
    expect(identity.source).toBe('cwd');
    expect(identity.id).not.toBe('unknown');
  });

  test('walks from deleted session directories to an existing parent git root', () => {
    const root = makeTempRoot('keepline-deleted-leaf-root-');
    mkdirSync(join(root, '.git'));
    const deletedLeaf = join(root, 'packages', 'removed-app');

    const identity = resolveProjectIdentity(deletedLeaf);

    expect(identity.rootPath).toBe(root);
    expect(identity.source).toBe('git-root');
  });

  test('uses collision-resistant IDs rather than lossy slugs', () => {
    expect(projectIdFromPath('/tmp/a-b')).not.toBe(projectIdFromPath('/tmp/a/b'));
    expect(projectIdFromPath('/tmp/Foo')).not.toBe(projectIdFromPath('/tmp/foo'));
  });
});
