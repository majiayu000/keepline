import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { Hono } from 'hono';
import { getWebStaticCandidates, selectWebStaticDir } from '../web/api/server.js';

describe('web static directory selection', () => {
  const tempDirs: string[] = [];
  const originalCwd = process.cwd();

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
    process.chdir(originalCwd);
    while (tempDirs.length > 0) {
      rmSync(tempDirs.pop()!, { recursive: true, force: true });
    }
  });

  test('uses source checkout build output and ignores legacy source fallbacks', () => {
    const root = tempDir();
    const moduleDir = join(root, 'repo', 'src', 'web', 'api');
    const currentDist = join(root, 'repo', 'public', 'dist');
    const legacySourceFallback = join(root, 'repo', 'src', 'web', 'public');
    const legacyDist = join(root, 'repo', 'src', 'web', 'public', 'dist');

    writeIndex(currentDist, 'Keepline');
    writeIndex(legacySourceFallback, 'Keepline');
    writeIndex(legacyDist, 'Claude Hub');

    const candidates = getWebStaticCandidates(moduleDir);

    expect(candidates).toEqual([currentDist]);
    expect(selectWebStaticDir(candidates)).toBe(currentDist);
  });

  test('does not use legacy source html when build output is missing', () => {
    const root = tempDir();
    const moduleDir = join(root, 'repo', 'src', 'web', 'api');
    const currentDist = join(root, 'repo', 'public', 'dist');
    const legacySourceFallback = join(root, 'repo', 'src', 'web', 'public');
    const legacyDist = join(root, 'repo', 'src', 'web', 'public', 'dist');

    writeIndex(legacySourceFallback, 'Keepline');
    writeIndex(legacyDist, 'Claude Hub');

    const candidates = getWebStaticCandidates(moduleDir);

    expect(candidates).toEqual([currentDist]);
    expect(selectWebStaticDir(candidates)).toBe(currentDist);
  });

  test('uses packaged public dist instead of launch cwd public dist', () => {
    const root = tempDir();
    const launchCwd = join(root, 'launch-cwd');
    const moduleDir = join(root, 'package', 'dist');
    const launchedFromCwdDist = join(launchCwd, 'public', 'dist');
    const packageBuildDist = join(root, 'package', 'public', 'dist');

    mkdirSync(launchCwd, { recursive: true });
    writeIndex(launchedFromCwdDist, 'Other App');
    writeIndex(packageBuildDist, 'Keepline');
    process.chdir(launchCwd);

    const candidates = getWebStaticCandidates(moduleDir);

    expect(candidates).toEqual([packageBuildDist]);
    expect(selectWebStaticDir(candidates)).toBe(packageBuildDist);
  });

  test('serves the packaged dashboard and assets from the built service client', async () => {
    const root = tempDir();
    const packageDir = join(root, 'package');
    const outputDir = join(packageDir, 'dist');
    const webDir = join(packageDir, 'public', 'dist');
    writeIndex(webDir, 'Keepline');
    mkdirSync(join(webDir, 'assets'));
    writeFileSync(join(webDir, 'assets', 'app.js'), 'console.log("Keepline");');
    writeFileSync(join(webDir, 'assets', 'app.css'), 'body { color: white; }');
    const build = await Bun.build({
      entrypoints: ['src/web/api/service-client.ts'],
      outdir: outputDir,
      target: 'bun',
      splitting: true,
    });
    expect(build.success).toBe(true);
    const { mountServiceClient } = await import(join(outputDir, 'service-client.js'));
    const app = new Hono();
    const dispose = mountServiceClient(app);
    const previousDist = process.env.KEEPLINE_WEB_DIST;
    delete process.env.KEEPLINE_WEB_DIST;
    process.chdir(root);
    try {
      for (const route of ['/', '/sessions']) {
        const response = await app.request(`http://localhost${route}`);
        expect(response.status).toBe(200);
        expect(await response.text()).toContain('<title>Keepline</title>');
      }
      for (const asset of ['app.js', 'app.css']) {
        const response = await app.request(`http://localhost/assets/${asset}`);
        expect(response.status).toBe(200);
        expect(await response.text()).toBe(await Bun.file(join(webDir, 'assets', asset)).text());
      }
    } finally {
      dispose();
      if (previousDist === undefined) delete process.env.KEEPLINE_WEB_DIST;
      else process.env.KEEPLINE_WEB_DIST = previousDist;
    }
  });
});
