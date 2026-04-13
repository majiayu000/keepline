import { describe, expect, test } from 'bun:test';
import { parseClaudePsOutput } from '../adapters/process/scanner.js';

describe('Process Parser', () => {
  test('parses Claude ps rows and skips shell wrappers or unrelated commands', () => {
    const output = [
      '12345 1.2 0.5 ttys001 Mon Jan  6 10:30:45 2026 /usr/local/bin/claude --model sonnet --cwd /tmp/app',
      '12346 0.1 0.2 ttys001 Mon Jan  6 10:30:46 2026 /bin/zsh -lc claude',
      '12347 0.3 0.1 ttys001 Mon Jan  6 10:30:47 2026 /usr/bin/python worker.py',
    ].join('\n');

    const parsed = parseClaudePsOutput(output);

    expect(parsed).toHaveLength(1);
    expect(parsed[0]).toMatchObject({
      pid: 12345,
      cpu: 1.2,
      mem: 0.5,
      tty: 'ttys001',
      argsRaw: '--model sonnet --cwd /tmp/app',
    });
    expect(parsed[0].startTimeMs).toBe(Date.parse('Mon Jan  6 10:30:45 2026'));
  });
});
