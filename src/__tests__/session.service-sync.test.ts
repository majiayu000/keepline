import { afterEach, describe, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

const tempDirs: string[] = [];

function createTempHome(): string {
  const homeDir = mkdtempSync(join(tmpdir(), 'keepline-test-sync-home-'));
  tempDirs.push(homeDir);
  const bin = join(homeDir, 'fixture-bin');
  mkdirSync(bin);
  for (const tool of ['ps', 'lsof']) {
    writeFileSync(join(bin, tool), '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  }
  return homeDir;
}

function writeClaudeSessionFile(homeDir: string, sessionId: string, timestamp = '2026-04-13T16:05:00.000Z'): void {
  const projectDir = join(homeDir, '.claude', 'projects', '-tmp-completed-sync');
  mkdirSync(projectDir, { recursive: true });
  writeFileSync(
    join(projectDir, `${sessionId}.jsonl`),
    JSON.stringify({
      type: 'user',
      uuid: 'user-1',
      sessionId,
      cwd: '/tmp/completed-sync',
      timestamp,
      userType: 'external',
      message: {
        role: 'user',
        content: 'Completed sync regression',
      },
    }) + '\n'
  );
}

function writeClaudeSessionMessages(homeDir: string, sessionId: string, messages: string[]): void {
  const projectDir = join(homeDir, '.claude', 'projects', '-tmp-completed-sync');
  mkdirSync(projectDir, { recursive: true });
  const lines = messages.map((content, index) => JSON.stringify({
    type: 'user',
    uuid: `user-${index}`,
    sessionId,
    cwd: '/tmp/completed-sync',
    timestamp: `2026-04-13T16:05:0${index}.000Z`,
    userType: 'external',
    message: { role: 'user', content },
  }));
  writeFileSync(join(projectDir, `${sessionId}.jsonl`), `${lines.join('\n')}\n`);
}

function runSyncScript(homeDir: string, script: string): Record<string, unknown> {
  const proc = Bun.spawnSync({
    cmd: [process.execPath, '--eval', script],
    cwd: process.cwd(),
    env: {
      ...process.env,
      HOME: homeDir,
      PATH: `${join(homeDir, 'fixture-bin')}:${process.env.PATH}`,
      KEEPLINE_HOME: homeDir,
      KEEPLINE_TEST_HOME: homeDir,
      KEEPLINE_TEST_ISOLATED: '1',
    },
    stdout: 'pipe',
    stderr: 'pipe',
  });

  if (proc.exitCode !== 0) {
    throw new Error(proc.stderr.toString() || `sync subprocess failed with code ${proc.exitCode}`);
  }

  const jsonLine = proc.stdout.toString().trim().split('\n').filter(Boolean).pop();
  if (!jsonLine) {
    throw new Error('sync subprocess produced no JSON output');
  }
  return JSON.parse(jsonLine) as Record<string, unknown>;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    rmSync(tempDirs.pop()!, { recursive: true, force: true });
  }
});

describe('SessionService sync', () => {
  test('preserves user-completed sessions during later file syncs', () => {
    const homeDir = createTempHome();
    const sessionId = 'completed-sync-1';
    writeClaudeSessionFile(homeDir, sessionId);

    const result = runSyncScript(homeDir, `
      const { resetDatabase } = await import('./src/db/migrations.ts');
      const { closeDatabase } = await import('./src/infrastructure/database/sqlite.ts');
      const { sessionRepository } = await import('./src/infrastructure/database/repositories/session.repository.ts');
      const { sessionService } = await import('./src/services/session.service.ts');

      resetDatabase();
      const completedAt = new Date('2026-04-13T16:00:00.000Z');
      sessionRepository.upsert({
        sessionId: ${JSON.stringify(sessionId)},
        client: 'claude',
        directory: '/tmp/completed-sync',
        status: 'completed',
        title: 'Completed task',
        initialPrompt: 'Completed sync regression',
        startedAt: new Date('2026-04-13T15:55:00.000Z'),
        lastActiveAt: completedAt,
        completedAt,
        pid: 999999,
        toolCount: 0,
        messageCount: 1,
      });

      const syncResult = await sessionService.syncSessions({ fullSync: true });
      const fetched = sessionRepository.findBySessionId(${JSON.stringify(sessionId)});
      console.log(JSON.stringify({
        syncResult,
        status: fetched?.status,
        completedAt: fetched?.completedAt?.toISOString(),
        lastActiveAt: fetched?.lastActiveAt.toISOString(),
        pid: fetched?.pid ?? null,
      }));
      closeDatabase();
    `);

    expect(result).toEqual({
      syncResult: { discovered: 0, updated: 1, lost: 0 },
      status: 'completed',
      completedAt: '2026-04-13T16:00:00.000Z',
      lastActiveAt: '2026-04-13T16:05:00.000Z',
      pid: null,
    });
  });

  test('stores historical transcripts without exposing them as operational lost sessions', () => {
    const homeDir = createTempHome();
    const sessionId = 'historical-sync-1';
    writeClaudeSessionFile(homeDir, sessionId);

    const result = runSyncScript(homeDir, `
      const { resetDatabase } = await import('./src/db/migrations.ts');
      const { closeDatabase } = await import('./src/infrastructure/database/sqlite.ts');
      const { sessionRepository } = await import('./src/infrastructure/database/repositories/session.repository.ts');
      const { sessionService } = await import('./src/services/session.service.ts');

      resetDatabase();
      const syncResult = await sessionService.syncSessions({ fullSync: true });
      console.log(JSON.stringify({
        syncResult,
        stored: sessionRepository.findAll().map((session) => ({
          sessionId: session.sessionId,
          status: session.status,
        })),
        operational: sessionRepository.findOperational().map((session) => session.sessionId),
      }));
      closeDatabase();
    `);

    expect(result).toEqual({
      syncResult: { discovered: 1, updated: 0, lost: 0 },
      stored: [{ sessionId, status: 'lost' }],
      operational: [],
    });
  });

  test('refreshes an injected-context title when a real task is found', () => {
    const homeDir = createTempHome();
    const sessionId = 'context-title-sync-1';
    writeClaudeSessionMessages(homeDir, sessionId, [
      '<recommended_plugins>plugin catalog</recommended_plugins>',
      '# AGENTS.md instructions for /tmp/completed-sync\n<INSTRUCTIONS>rules</INSTRUCTIONS>',
      '<environment_context><cwd>/tmp/completed-sync</cwd></environment_context>',
      'Repair the persisted task title',
    ]);

    const result = runSyncScript(homeDir, `
      const { resetDatabase } = await import('./src/db/migrations.ts');
      const { closeDatabase } = await import('./src/infrastructure/database/sqlite.ts');
      const { sessionRepository } = await import('./src/infrastructure/database/repositories/session.repository.ts');
      const { sessionService } = await import('./src/services/session.service.ts');

      resetDatabase();
      sessionRepository.upsert({
        sessionId: ${JSON.stringify(sessionId)},
        client: 'claude',
        directory: '/tmp/completed-sync',
        status: 'lost',
        title: '继续',
        initialPrompt: '<recommended_plugins>plugin catalog</recommended_plugins>',
        startedAt: new Date('2026-04-13T16:00:00.000Z'),
        lastActiveAt: new Date('2026-04-13T16:00:00.000Z'),
        toolCount: 0,
        messageCount: 1,
      });

      await sessionService.syncSessions({ fullSync: true });
      const fetched = sessionRepository.findBySessionId(${JSON.stringify(sessionId)});
      console.log(JSON.stringify({
        title: fetched?.title,
        initialPrompt: fetched?.initialPrompt,
      }));
      closeDatabase();
    `);

    expect(result).toEqual({
      title: 'Repair the persisted task title',
      initialPrompt: 'Repair the persisted task title',
    });
  });

  test('preserves a normal title while refreshing scanned session fields', () => {
    const homeDir = createTempHome();
    const sessionId = 'normal-title-sync-1';
    writeClaudeSessionMessages(homeDir, sessionId, ['A newer transcript task']);

    const result = runSyncScript(homeDir, `
      const { resetDatabase } = await import('./src/db/migrations.ts');
      const { closeDatabase } = await import('./src/infrastructure/database/sqlite.ts');
      const { sessionRepository } = await import('./src/infrastructure/database/repositories/session.repository.ts');
      const { sessionService } = await import('./src/services/session.service.ts');

      resetDatabase();
      sessionRepository.upsert({
        sessionId: ${JSON.stringify(sessionId)},
        client: 'claude',
        directory: '/tmp/completed-sync',
        status: 'lost',
        title: 'AGENTS.md instructions parser fix',
        initialPrompt: 'Original user task',
        startedAt: new Date('2026-04-13T16:00:00.000Z'),
        lastActiveAt: new Date('2026-04-13T16:00:00.000Z'),
        toolCount: 0,
        messageCount: 1,
      });

      await sessionService.syncSessions({ fullSync: true });
      const fetched = sessionRepository.findBySessionId(${JSON.stringify(sessionId)});
      console.log(JSON.stringify({ title: fetched?.title, initialPrompt: fetched?.initialPrompt }));
      closeDatabase();
    `);

    expect(result).toEqual({
      title: 'AGENTS.md instructions parser fix',
      initialPrompt: 'Original user task',
    });
  });
});


describe('SessionService sync with missing cwd', () => {
  function syncFixture(options: { transcript: boolean; psListsPid: boolean; alive: boolean; partialCwd?: boolean; status?: 'idle' | 'waiting'; statusSource?: 'hook' | 'scan'; newActivity?: boolean; reusePid?: boolean }) {
    const homeDir = createTempHome();
    const sessionId = 'live-cwd-sync';
    if (options.transcript) writeClaudeSessionFile(homeDir, sessionId, options.newActivity ? new Date().toISOString() : undefined);
    if (options.reusePid) writeClaudeSessionFile(homeDir, 'new-cwd-sync', new Date().toISOString());
    const now = new Date();
    const [day, month, date, year] = now.toDateString().split(' ');
    const psStart = options.reusePid ? `${day} ${month} ${date} ${now.toTimeString().slice(0, 8)} ${year}` : 'Mon Jan 6 10:30:45 2026';
    return runSyncScript(homeDir, `
      const { spyOn } = await import('bun:test');
      const childProcess = await import('child_process');
      const psOutput = ${JSON.stringify(options.psListsPid
        ? `12345 0.1 0.5 ttys001 ${psStart} /usr/local/bin/claude`
        : '')};
      spyOn(childProcess, 'execSync').mockImplementation((command) => {
        if (command.startsWith('ps ')) return psOutput;
        if (command.startsWith('lsof ')) throw Object.assign(new Error('fixture lsof failure'), {
          status: 1, signal: null,
          stdout: ${JSON.stringify(options.partialCwd ? 'claude 12345 fixture cwd DIR 1,1 0 1 /tmp/completed-sync\n' : '')},
        });
        throw new Error('Unexpected command: ' + command);
      });
      const checkedPids = [];
      process.kill = (pid, signal) => {
        if (signal !== 0) throw new Error('Only liveness checks are allowed');
        checkedPids.push(pid);
        if (pid === 12345 && ${options.alive}) return true;
        throw Object.assign(new Error('fixture process exited'), { code: 'ESRCH' });
      };
      const { resetDatabase } = await import('./src/db/migrations.ts');
      const { closeDatabase } = await import('./src/infrastructure/database/sqlite.ts');
      const { sessionRepository } = await import('./src/infrastructure/database/repositories/session.repository.ts');
      const { sessionService } = await import('./src/services/session.service.ts');
      resetDatabase();
      for (const [id, pid] of [[${JSON.stringify(sessionId)}, 12345], ['dead-cwd-sync', 12346]]) {
        sessionRepository.upsert({
          sessionId: id, client: 'claude', directory: '/tmp/completed-sync',
          status: ${JSON.stringify(options.status ?? 'waiting')}, statusSource: ${JSON.stringify(options.statusSource ?? 'hook')}, pid, tty: 'ttys001',
          title: 'Fixture task', initialPrompt: 'Fixture task',
          startedAt: new Date('2026-01-06T10:00:00Z'),
          lastActiveAt: new Date('2026-04-13T16:06:00Z'),
          wasProcessObserved: true,
        });
      }
      const lostEvents = [];
      const { on } = await import('./src/lib/events.ts');
      on('session:lost', ({session}) => lostEvents.push(session.sessionId));
      const syncResult = await sessionService.syncSessions({ fullSync: true });
      const { SessionAggregator } = await import('./src/services/session.aggregator.ts');
      console.log(JSON.stringify({
        aggregated: new SessionAggregator(sessionRepository).getAggregatedSessions().map(({sessionId,status,processRunning}) => ({sessionId,status,processRunning})),
        syncResult, checkedPids, lostEvents,
        sessions: sessionRepository.findAll().map(({sessionId,status,statusSource,pid,tty}) =>
          ({sessionId,status,statusSource,pid: pid ?? null,tty: tty ?? null})),
      }));
      closeDatabase();
    `);
  }

  test('standalone aggregation keeps a known PID live without cwd', () => {
    const result = syncFixture({ transcript: true, psListsPid: true, alive: true });
    expect(result.aggregated).toContainEqual({ sessionId: 'live-cwd-sync', status: 'waiting', processRunning: true });
  });

  test('does not retain an old PID that was attributed to a new transcript', () => {
    const result = syncFixture({ transcript: true, psListsPid: true, alive: true, partialCwd: true, reusePid: true });
    expect(result.syncResult).toEqual({ discovered: 1, updated: 1, lost: 2 });
    expect(result.sessions).toContainEqual({
      sessionId: 'live-cwd-sync', status: 'lost', statusSource: 'scan', pid: null, tty: null,
    });
    expect((result.sessions as Array<{sessionId: string; pid: number | null}>).filter(session => session.pid === 12345).map(session => session.sessionId)).toEqual(['new-cwd-sync']);
  });

  for (const psListsPid of [true, false]) {
    test(`refreshes scan-sourced idle state from new transcript activity, psListsPid=${psListsPid}`, () => {
      const result = syncFixture({ transcript: true, psListsPid, alive: true, status: 'idle', statusSource: 'scan', newActivity: true });
      expect(result.sessions).toContainEqual({
        sessionId: 'live-cwd-sync', status: 'running', statusSource: 'scan', pid: 12345, tty: 'ttys001',
      });
    });
    test(`ages scan-sourced waiting state into idle, psListsPid=${psListsPid}`, () => {
      const result = syncFixture({ transcript: true, psListsPid, alive: true, statusSource: 'scan' });
      expect(result.sessions).toContainEqual({
        sessionId: 'live-cwd-sync', status: 'idle', statusSource: 'scan', pid: 12345, tty: 'ttys001',
      });
    });
  }

  test('uses cwd stdout from a nonzero exit without losing the live session', () => {
    const result = syncFixture({ transcript: true, psListsPid: true, alive: true, partialCwd: true });
    expect(result.syncResult).toEqual({ discovered: 0, updated: 1, lost: 1 });
    expect(result.lostEvents).toEqual(['dead-cwd-sync']);
    expect(result.sessions).toContainEqual({
      sessionId: 'live-cwd-sync', status: 'waiting', statusSource: 'hook', pid: 12345, tty: 'ttys001',
    });
  });

  for (const transcript of [true, false]) {
    for (const psListsPid of [true, false]) {
      test(`keeps a live PID with transcript=${transcript}, psListsPid=${psListsPid}`, () => {
        const result = syncFixture({ transcript, psListsPid, alive: true });
        expect(result.syncResult).toEqual({ discovered: 0, updated: transcript ? 1 : 0, lost: 1 });
        expect(result.lostEvents).toEqual(['dead-cwd-sync']);
        expect(result.sessions).toContainEqual({
          sessionId: 'live-cwd-sync', status: 'waiting', statusSource: 'hook', pid: 12345, tty: 'ttys001',
        });
        expect((result.checkedPids as number[]).filter(pid => pid === 12345)).toHaveLength(psListsPid ? 0 : 1);
      });
    }

    test(`still marks dead sessions lost with transcript=${transcript}`, () => {
      const result = syncFixture({ transcript, psListsPid: false, alive: false });
      expect(result.syncResult).toEqual({ discovered: 0, updated: transcript ? 1 : 0, lost: 2 });
      expect(result.lostEvents).toContain('live-cwd-sync');
      expect(result.lostEvents).toContain('dead-cwd-sync');
      expect(result.sessions).toContainEqual({
        sessionId: 'live-cwd-sync', status: 'lost', statusSource: 'scan', pid: null,
        tty: transcript ? null : 'ttys001',
      });
    });
  }
});
