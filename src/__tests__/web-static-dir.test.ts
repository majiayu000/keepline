import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { selectWebStaticDir } from '../web/api/server.js';

describe('web static directory selection', () => {
  const tempDirs: string[] = [];

  function tempDir(): string {
    const dir = mkdtempSync(join(tmpdir(), 'keepline-web-static-'));
    tempDirs.push(dir);
    return dir;
  }

  function writeIndex(dir: string, title: string): void {
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, 'index.html'), `<title>${title}</title>`);
  }

  afterEach(() => {
    while (tempDirs.length > 0) {
      rmSync(tempDirs.pop()!, { recursive: true, force: true });
    }
  });

  test('prefers the current built client over source fallback', () => {
    const root = tempDir();
    const currentDist = join(root, 'public', 'dist');
    const sourceFallback = join(root, 'src', 'web', 'public');
    const legacyDist = join(root, 'src', 'web', 'public', 'dist');

    writeIndex(currentDist, 'Keepline');
    writeIndex(sourceFallback, 'Keepline');
    writeIndex(legacyDist, 'Claude Hub');

    expect(selectWebStaticDir([currentDist, sourceFallback, legacyDist])).toBe(currentDist);
  });

  test('uses source fallback instead of stale legacy dist output', () => {
    const root = tempDir();
    const currentDist = join(root, 'public', 'dist');
    const sourceFallback = join(root, 'src', 'web', 'public');
    const legacyDist = join(root, 'src', 'web', 'public', 'dist');

    writeIndex(sourceFallback, 'Keepline');
    writeIndex(legacyDist, 'Claude Hub');

    expect(selectWebStaticDir([currentDist, sourceFallback, legacyDist])).toBe(sourceFallback);
  });

  test('keeps the built package public dist fallback', () => {
    const root = tempDir();
    const sourceBuildDist = join(root, 'repo', 'public', 'dist');
    const launchedFromCwdDist = join(root, 'launch-cwd', 'public', 'dist');
    const packageSourceFallback = join(root, 'package', 'public');
    const packageBuildDist = join(root, 'package', 'public', 'dist');

    writeIndex(packageBuildDist, 'Keepline');

    expect(selectWebStaticDir([
      sourceBuildDist,
      launchedFromCwdDist,
      packageSourceFallback,
      packageBuildDist,
    ])).toBe(packageBuildDist);
  });
});
