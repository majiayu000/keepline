import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { parseAgentPsOutput, parseClaudePsOutput } from '../adapters/process/scanner.js';

describe('Process Parser', () => {
  test('returns an empty list when ps output has no supported agent rows', () => {
    const output = [
      '12340 0.0 0.1 ?? Mon Jan  6 10:30:40 2026 /usr/sbin/distnoted',
      '12341 0.1 0.2 ?? Mon Jan  6 10:30:41 2026 /usr/bin/python worker.py',
    ].join('\n');

    expect(parseAgentPsOutput(output)).toEqual([]);
    expect(parseClaudePsOutput(output)).toEqual([]);
  });

  test('parses Claude ps rows and skips shell wrappers or unrelated commands', () => {
    const output = [
      '12345 1.2 0.5 ttys001 Mon Jan  6 10:30:45 2026 /usr/local/bin/claude --model sonnet --cwd /tmp/app',
      '12344 0.8 0.4 ttys002 Mon Jan  6 10:30:44 2026 /usr/local/bin/node /usr/local/bin/claude --model opus',
      '12346 0.1 0.2 ttys001 Mon Jan  6 10:30:46 2026 /bin/zsh -lc claude',
      '12347 0.3 0.1 ttys001 Mon Jan  6 10:30:47 2026 /usr/bin/python worker.py',
    ].join('\n');

    const parsed = parseClaudePsOutput(output);

    expect(parsed).toHaveLength(2);
    expect(parsed[0]).toMatchObject({
      pid: 12345,
      cpu: 1.2,
      mem: 0.5,
      tty: 'ttys001',
      argsRaw: '--model sonnet --cwd /tmp/app',
    });
    expect(parsed[0].startTimeMs).toBe(Date.parse('Mon Jan  6 10:30:45 2026'));
    expect(parsed[1]).toMatchObject({
      pid: 12344,
      argsRaw: '--model opus',
    });
  });

  test('desktop Claude paths with spaces preserve arguments and skip disclaimer wrappers',() => {
    const command = '/Users/me/Library/Application Support/Claude/claude-code/2.1.288/build/claude.app/Contents/MacOS/claude --output-format stream-json --resume=session-123';
    const parsed = parseAgentPsOutput(`12345 1.2 0.5 ?? Mon Jan  6 10:30:45 2026 ${command}\n12346 0 0 ?? Mon Jan  6 10:30:45 2026 /Applications/Claude.app/Contents/Helpers/disclaimer --pgroup -- ${command}`);
    expect(parsed).toHaveLength(1); expect(parsed[0]).toMatchObject({ client: 'claude',argsRaw: '--output-format stream-json --resume=session-123' });
  });
  test('parses Codex CLI rows and filters Codex helper processes', () => {
    const output = [
      '22345 1.2 0.5 ttys001 Mon Jan  6 10:30:45 2026 /usr/local/bin/codex resume 019ed4a3-2186-7e51-9aa1-ca1e376549b8',
      '22344 0.4 0.2 ttys002 Mon Jan  6 10:30:44 2026 /usr/local/bin/node /usr/local/bin/codex resume --last',
      '22346 0.1 0.2 ?? Mon Jan  6 10:30:46 2026 /Applications/Codex.app/Contents/MacOS/Codex',
      '22347 0.3 0.1 ?? Mon Jan  6 10:30:47 2026 /usr/local/bin/codex app-server',
      '22348 0.3 0.1 ?? Mon Jan  6 10:30:48 2026 /usr/local/bin/codex_chronicle',
    ].join('\n');

    const parsed = parseAgentPsOutput(output);

    expect(parsed).toHaveLength(2);
    expect(parsed[0]).toMatchObject({
      client: 'codex',
      pid: 22345,
      argsRaw: 'resume 019ed4a3-2186-7e51-9aa1-ca1e376549b8',
    });
    expect(parsed[1]).toMatchObject({
      client: 'codex',
      pid: 22344,
      argsRaw: 'resume --last',
    });
  });
});


describe('Process scanner cwd failures', () => {
  const psOutput = [
    '12345 1.2 0.5 ttys001 Mon Jan  6 10:30:45 2026 /usr/local/bin/claude',
    '22345 0.4 0.2 ttys002 Mon Jan  6 10:30:44 2026 /usr/local/bin/codex',
  ].join('\n');
  const cwdOutput = 'claude 12345 fixture cwd DIR 1,1 0 1 /tmp/fixture project\n';

  const fixtureDirs: string[] = [];
  afterEach(() => {
    while (fixtureDirs.length) rmSync(fixtureDirs.pop()!, { recursive: true, force: true });
  });

  function runScannerScript(script: string, fixtures?: { ps: string; cwd: string }) {
    const bin = mkdtempSync(join(tmpdir(), 'keepline-test-process-bin-'));
    fixtureDirs.push(bin);
    // Block real process inspection even if a builtin mock stops working.
    writeFileSync(join(bin, 'ps'), '#!/bin/sh\nprintf \'%s\' "$KEEPLINE_FIXTURE_PS"\n', { mode: 0o755 });
    writeFileSync(join(bin, 'lsof'), '#!/bin/sh\nprintf \'%s\' "$KEEPLINE_FIXTURE_CWD"\nexit 1\n', { mode: 0o755 });
    return Bun.spawnSync({
      cmd: [process.execPath, '--eval', script],
      env: {
        ...process.env, PATH: `${bin}:${process.env.PATH}`,
        KEEPLINE_FIXTURE_PS: fixtures?.ps ?? '', KEEPLINE_FIXTURE_CWD: fixtures?.cwd ?? '',
      },
      stdout: 'pipe', stderr: 'pipe',
    });
  }

  function scanWithLsof(error: Record<string, unknown> | null, output = cwdOutput) {
    const script = `
      const { spyOn } = await import('bun:test');
      const childProcess = await import('child_process');
      spyOn(childProcess, 'execSync').mockImplementation((command) => {
        if (command.startsWith('ps ')) return ${JSON.stringify(psOutput)};
        if (command.startsWith('lsof ')) {
          const details = ${JSON.stringify(error)};
          if (details) throw Object.assign(new Error('fixture lsof failure'), details);
          return ${JSON.stringify(output)};
        }
        throw new Error('Unexpected command: ' + command);
      });
      const { scanAgentProcesses } = await import('./src/adapters/process/scanner.ts');
      console.log(JSON.stringify(scanAgentProcesses().map(({pid, client, cwd}) => ({pid, client, cwd}))));
    `;
    const proc = runScannerScript(script);
    if (proc.exitCode !== 0) throw new Error(proc.stderr.toString());
    return JSON.parse(proc.stdout.toString().trim());
  }

  test('reads stdout from an actual mocked lsof command exiting one', () => {
    const proc = runScannerScript(`
      const { scanAgentProcesses } = await import('./src/adapters/process/scanner.ts');
      console.log(JSON.stringify(scanAgentProcesses().map(({pid, cwd}) => ({pid, cwd}))));
    `, { ps: psOutput, cwd: cwdOutput });
    expect(proc.exitCode).toBe(0);
    expect(JSON.parse(proc.stdout.toString().trim())).toEqual([
      { pid: 12345, cwd: '/tmp/fixture project' }, { pid: 22345, cwd: '' },
    ]);
  });

  test('retains partial cwd stdout when lsof exits one', () => {
    expect(scanWithLsof({ status: 1, signal: null, stdout: cwdOutput })).toEqual([
      { pid: 12345, client: 'claude', cwd: '/tmp/fixture project' },
      { pid: 22345, client: 'codex', cwd: '' },
    ]);
  });

  test('retains live ps rows when lsof returns no cwd data', () => {
    expect(scanWithLsof({ status: 1, signal: null, stdout: '' })).toEqual([
      { pid: 12345, client: 'claude', cwd: '' },
      { pid: 22345, client: 'codex', cwd: '' },
    ]);
  });

  for (const error of [
    { status: null, signal: 'SIGTERM', stdout: cwdOutput },
    { code: 'ENOENT', stdout: cwdOutput },
  ]) {
    test(`ignores partial cwd data for ${error.signal ?? error.code}`, () => {
      expect(scanWithLsof(error)).toEqual([
        { pid: 12345, client: 'claude', cwd: '' },
        { pid: 22345, client: 'codex', cwd: '' },
      ]);
    });
  }

  test('preserves the process scan error contract when ps fails', () => {
    const proc = runScannerScript(`
        const { spyOn } = await import('bun:test');
        const childProcess = await import('child_process');
        spyOn(childProcess, 'execSync').mockImplementation(() => { throw new Error('fixture ps failure'); });
        const { scanAgentProcesses } = await import('./src/adapters/process/scanner.ts');
        try { scanAgentProcesses(); throw new Error('scan should fail'); }
        catch (error) { console.log(JSON.stringify({name: error.name, code: error.code, message: error.message})); }
      `);
    expect(proc.exitCode).toBe(0);
    expect(JSON.parse(proc.stdout.toString().trim())).toEqual({
      name: 'ProcessScanError', code: 'PROCESS_SCAN_ERROR', message: 'Failed to scan processes',
    });
  });
});
