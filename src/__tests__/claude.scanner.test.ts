import { afterEach, describe, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

const tempDirs: string[] = [];

function createTempHome(): string {
  const homeDir = mkdtempSync(join(tmpdir(), 'claude-hub-home-'));
  tempDirs.push(homeDir);
  return homeDir;
}

function writeSessionFile(
  homeDir: string,
  projectDirName: string,
  filename: string,
  lines: Array<Record<string, unknown> | string>
): string {
  const projectDir = join(homeDir, '.claude', 'projects', projectDirName);
  mkdirSync(projectDir, { recursive: true });
  const filePath = join(projectDir, filename);
  const contents = lines
    .map((line) => (typeof line === 'string' ? line : JSON.stringify(line)))
    .join('\n');
  writeFileSync(filePath, `${contents}\n`);
  return filePath;
}

function runScannerScript(homeDir: string, script: string) {
  const proc = Bun.spawnSync({
    cmd: [process.execPath, '--eval', script],
    cwd: process.cwd(),
    env: {
      ...process.env,
      HOME: homeDir,
      CLAUDE_HUB_HOME: join(homeDir, '.claude-hub'),
    },
    stdout: 'pipe',
    stderr: 'pipe',
  });

  if (proc.exitCode !== 0) {
    throw new Error(proc.stderr.toString() || `scanner subprocess failed with code ${proc.exitCode}`);
  }

  const output = proc.stdout.toString().trim();
  const jsonLine = output.split('\n').filter(Boolean).pop();
  if (!jsonLine) {
    throw new Error('scanner subprocess produced no JSON output');
  }

  return JSON.parse(jsonLine) as Record<string, unknown>;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    rmSync(tempDirs.pop()!, { recursive: true, force: true });
  }
});

describe('Claude Scanner', () => {
  test('returns valid sessions and keeps broken files excluded across repeated scans', () => {
    const homeDir = createTempHome();

    writeSessionFile(homeDir, '-tmp-valid-project', 'valid-session.jsonl', [
      {
        type: 'user',
        uuid: 'user-1',
        sessionId: 'valid-session',
        cwd: '/tmp/valid-project',
        timestamp: '2026-04-13T11:00:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'Profile the valid parser path' },
      },
    ]);

    writeSessionFile(homeDir, '-tmp-broken-project', 'broken-session.jsonl', [
      {
        type: 'user',
        uuid: 'user-2',
        sessionId: 'broken-session',
        cwd: '/tmp/broken-project',
        timestamp: '2026-04-13T11:00:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'This file is broken' },
      },
      '{"type":"assistant", invalid json',
    ]);

    const result = runScannerScript(homeDir, `
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const first = await getAllSessions();
      const second = await getAllSessions();
      console.log(JSON.stringify({
        first: first.map((session) => session.sessionId),
        second: second.map((session) => session.sessionId),
      }));
    `);

    expect(result.first).toEqual(['valid-session']);
    expect(result.second).toEqual(['valid-session']);
  });

  test('reparses a previously broken file after its contents change', () => {
    const homeDir = createTempHome();
    const filePath = writeSessionFile(homeDir, '-tmp-repair-project', 'repair-session.jsonl', [
      {
        type: 'user',
        uuid: 'user-1',
        sessionId: 'repair-session',
        cwd: '/tmp/repair-project',
        timestamp: '2026-04-13T11:10:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'Broken on first scan' },
      },
      '{"type":"assistant", invalid json',
    ]);

    const result = runScannerScript(homeDir, `
      const { utimesSync, writeFileSync } = await import('fs');
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const first = await getAllSessions();
      writeFileSync(${JSON.stringify(filePath)}, JSON.stringify({
        type: 'user',
        uuid: 'user-1',
        sessionId: 'repair-session',
        cwd: '/tmp/repair-project',
        timestamp: '2026-04-13T11:10:05.000Z',
        userType: 'external',
        message: { role: 'user', content: 'Fixed on second scan' },
      }) + '\\n');
      const nextTime = new Date(Date.now() + 2000);
      utimesSync(${JSON.stringify(filePath)}, nextTime, nextTime);
      const second = await getAllSessions();
      console.log(JSON.stringify({
        firstCount: first.length,
        second: second.map((session) => ({
          sessionId: session.sessionId,
          firstMessage: session.firstMessage,
        })),
      }));
    `);

    expect(result.firstCount).toBe(0);
    expect(result.second).toEqual([
      {
        sessionId: 'repair-session',
        firstMessage: 'Fixed on second scan',
      },
    ]);
  });
});
