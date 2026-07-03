import { describe, expect, test } from 'bun:test';
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
