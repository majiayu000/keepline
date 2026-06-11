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

function runScannerScriptDetailed(homeDir: string, script: string) {
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

  const stdout = proc.stdout.toString();
  const output = stdout.trim();
  const jsonLine = output.split('\n').filter(Boolean).pop();
  if (!jsonLine) {
    throw new Error('scanner subprocess produced no JSON output');
  }

  return {
    stdout,
    data: JSON.parse(jsonLine) as Record<string, unknown>,
  };
}

function runScannerScript(homeDir: string, script: string) {
  return runScannerScriptDetailed(homeDir, script).data;
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

  test('keeps bulk scans lightweight while detailed reads still include tool calls', () => {
    const homeDir = createTempHome();

    writeSessionFile(homeDir, '-tmp-toolcalls-project', 'toolcalls-session.jsonl', [
      {
        type: 'user',
        uuid: 'user-1',
        sessionId: 'toolcalls-session',
        cwd: '/tmp/toolcalls-project',
        timestamp: '2026-04-13T11:20:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'Inspect tool calls on demand' },
      },
      {
        type: 'assistant',
        uuid: 'assistant-1',
        parentUuid: 'user-1',
        sessionId: 'toolcalls-session',
        cwd: '/tmp/toolcalls-project',
        timestamp: '2026-04-13T11:20:05.000Z',
        message: {
          role: 'assistant',
          model: 'claude-3-5-sonnet-20241022',
          content: [
            {
              type: 'tool_use',
              id: 'tool-1',
              name: 'Read',
              input: { file_path: '/tmp/toolcalls-project/src/index.ts' },
            },
            { type: 'text', text: 'Read file' },
          ],
        },
      },
    ]);

    const result = runScannerScript(homeDir, `
      const { clearSessionCache, getAllSessions, getSessionById } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const sessions = await getAllSessions();
      const detail = await getSessionById('toolcalls-session');
      console.log(JSON.stringify({
        bulk: sessions.map((session) => ({
          sessionId: session.sessionId,
          hasToolCalls: Array.isArray(session.toolCalls),
          toolCount: session.toolCount,
        })),
        detail: detail ? {
          sessionId: detail.sessionId,
          hasToolCalls: Array.isArray(detail.toolCalls),
          toolCalls: detail.toolCalls?.map((tool) => tool.name) || [],
          toolCount: detail.toolCount,
        } : null,
      }));
    `);

    expect(result.bulk).toEqual([
      {
        sessionId: 'toolcalls-session',
        hasToolCalls: false,
        toolCount: 1,
      },
    ]);
    expect(result.detail).toEqual({
      sessionId: 'toolcalls-session',
      hasToolCalls: true,
      toolCalls: ['Read'],
      toolCount: 1,
    });
  });

  test('rich bulk scans include sub-agent files and tool calls for persistence sync', () => {
    const homeDir = createTempHome();

    writeSessionFile(homeDir, '-tmp-agent-project', 'parent-session.jsonl', [
      {
        type: 'user',
        uuid: 'user-parent',
        sessionId: 'parent-session',
        cwd: '/tmp/agent-project',
        timestamp: '2026-04-13T11:25:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'Launch a sub-agent' },
      },
      {
        type: 'assistant',
        uuid: 'assistant-parent',
        parentUuid: 'user-parent',
        sessionId: 'parent-session',
        cwd: '/tmp/agent-project',
        timestamp: '2026-04-13T11:25:05.000Z',
        message: {
          role: 'assistant',
          model: 'claude-3-5-sonnet-20241022',
          content: [
            {
              type: 'tool_use',
              id: 'tool-parent',
              name: 'Bash',
              input: { command: 'pwd' },
            },
          ],
        },
      },
    ]);

    writeSessionFile(homeDir, '-tmp-agent-project', 'agent-agent-123.jsonl', [
      {
        type: 'user',
        uuid: 'user-agent',
        sessionId: 'parent-session',
        cwd: '/tmp/agent-project',
        timestamp: '2026-04-13T11:26:00.000Z',
        userType: 'external',
        agentId: 'agent-123',
        isSidechain: true,
        message: { role: 'user', content: 'Inspect from a sub-agent' },
      },
      {
        type: 'assistant',
        uuid: 'assistant-agent',
        parentUuid: 'user-agent',
        sessionId: 'parent-session',
        cwd: '/tmp/agent-project',
        timestamp: '2026-04-13T11:26:05.000Z',
        agentId: 'agent-123',
        isSidechain: true,
        message: {
          role: 'assistant',
          model: 'claude-3-5-sonnet-20241022',
          content: [
            {
              type: 'tool_use',
              id: 'tool-agent',
              name: 'Read',
              input: { file_path: '/tmp/agent-project/src/index.ts' },
            },
          ],
          usage: {
            input_tokens: 10,
            output_tokens: 5,
          },
        },
      },
    ]);

    const result = runScannerScript(homeDir, `
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      await getAllSessions();
      const sessions = await getAllSessions({ includeSubAgents: true, includeToolCalls: true });
      console.log(JSON.stringify({
        sessions: sessions.map((session) => ({
          sessionId: session.sessionId,
          parentSessionId: session.parentSessionId ?? null,
          isSubAgent: session.isSubAgent,
          toolCalls: session.toolCalls?.map((tool) => tool.name) || [],
          totalTokens: session.usageStats?.totalTokens || 0,
        })).sort((a, b) => a.sessionId.localeCompare(b.sessionId)),
      }));
    `);

    expect(result.sessions).toEqual([
      {
        sessionId: 'agent-agent-123',
        parentSessionId: 'parent-session',
        isSubAgent: true,
        toolCalls: ['Read'],
        totalTokens: 15,
      },
      {
        sessionId: 'parent-session',
        parentSessionId: null,
        isSubAgent: false,
        toolCalls: ['Bash'],
        totalTokens: 0,
      },
    ]);
  });

  test('persists broken-file skips across subprocess restarts when mtime is unchanged', () => {
    const homeDir = createTempHome();
    const brokenPath = writeSessionFile(homeDir, '-tmp-persist-broken', 'persist-broken.jsonl', [
      {
        type: 'user',
        uuid: 'user-1',
        sessionId: 'persist-broken',
        cwd: '/tmp/persist-broken',
        timestamp: '2026-04-13T12:00:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'broken file should be persisted' },
      },
      '{"type":"assistant", invalid json',
    ]);

    const firstRun = runScannerScriptDetailed(homeDir, `
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const sessions = await getAllSessions();
      console.log(JSON.stringify({ count: sessions.length }));
    `);
    const secondRun = runScannerScriptDetailed(homeDir, `
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const sessions = await getAllSessions();
      console.log(JSON.stringify({ count: sessions.length }));
    `);

    expect(firstRun.data.count).toBe(0);
    expect(secondRun.data.count).toBe(0);
    expect(firstRun.stdout.includes(brokenPath)).toBe(true);
    expect(secondRun.stdout.includes(brokenPath)).toBe(false);
  });

  test('summarizes bulk parse failures instead of logging each file individually', () => {
    const homeDir = createTempHome();

    for (let i = 0; i < 4; i++) {
      writeSessionFile(homeDir, '-tmp-many-broken', `broken-${i}.jsonl`, [
        {
          type: 'user',
          uuid: `user-${i}`,
          sessionId: `broken-${i}`,
          cwd: '/tmp/many-broken',
          timestamp: '2026-04-13T12:10:00.000Z',
          userType: 'external',
          message: { role: 'user', content: 'broken file' },
        },
        '{"type":"assistant", invalid json',
      ]);
    }

    const result = runScannerScriptDetailed(homeDir, `
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const sessions = await getAllSessions();
      console.log(JSON.stringify({ count: sessions.length }));
    `);

    expect(result.data.count).toBe(0);
    expect(result.stdout.includes('Skipped invalid session files during scan')).toBe(true);
    expect(result.stdout.includes('Failed to parse session file:')).toBe(false);
  });

  test('skips session files whose filename does not contain a safe session ID', () => {
    const homeDir = createTempHome();

    writeSessionFile(homeDir, '-tmp-unsafe-filename', 'safe1234;touch-owned.jsonl', [
      {
        type: 'user',
        uuid: 'user-1',
        sessionId: 'safe1234;touch-owned',
        cwd: '/tmp/unsafe-filename',
        timestamp: '2026-04-13T12:20:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'malicious filename' },
      },
    ]);

    const result = runScannerScriptDetailed(homeDir, `
      const { clearSessionCache, getAllSessions, scanProjectsDirectory } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const files = scanProjectsDirectory();
      const sessions = await getAllSessions();
      console.log(JSON.stringify({
        files: files.map((file) => file.sessionId),
        sessions: sessions.map((session) => session.sessionId),
      }));
    `);

    expect(result.data.files).toEqual([]);
    expect(result.data.sessions).toEqual([]);
    expect(result.stdout.includes('Skipped session files with invalid session IDs')).toBe(true);
  });

  test('skips parsed sessions whose JSONL payload contains an unsafe session ID', () => {
    const homeDir = createTempHome();

    writeSessionFile(homeDir, '-tmp-unsafe-payload', 'safe-session-123.jsonl', [
      {
        type: 'user',
        uuid: 'user-1',
        sessionId: 'safe1234;touch-owned',
        cwd: '/tmp/unsafe-payload',
        timestamp: '2026-04-13T12:30:00.000Z',
        userType: 'external',
        message: { role: 'user', content: 'malicious payload session id' },
      },
    ]);

    const result = runScannerScriptDetailed(homeDir, `
      const { clearSessionCache, getAllSessions } = await import('./src/adapters/claude/scanner.ts');
      clearSessionCache();
      const sessions = await getAllSessions();
      console.log(JSON.stringify({
        sessions: sessions.map((session) => session.sessionId),
      }));
    `);

    expect(result.data.sessions).toEqual([]);
    expect(result.stdout.includes('Skipped invalid session files during scan')).toBe(true);
  });
});
