import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, mkdirSync, realpathSync, rmSync, symlinkSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { resolveAllowedTerminalCwd } from '../services/pty.manager.js';
import { isAllowedTerminalOrigin } from '../web/api/terminal-security.js';
import { isAllowedRequestHost } from '../web/api/request-security.js';

describe('terminal WebSocket Origin guard', () => {
  test('rejects missing Origin headers', () => {
    const req = new Request('http://127.0.0.1:3377/ws/terminal');
    expect(isAllowedTerminalOrigin(req, '127.0.0.1', 3377, false)).toBe(false);
  });

  test('rejects cross-site browser origins', () => {
    const req = new Request('http://127.0.0.1:3377/ws/terminal', {
      headers: { Origin: 'https://attacker.example' },
    });
    expect(isAllowedTerminalOrigin(req, '127.0.0.1', 3377, false)).toBe(false);
  });

  test('accepts same-origin browser connections', () => {
    const req = new Request('http://127.0.0.1:3377/ws/terminal', {
      headers: { Origin: 'http://127.0.0.1:3377' },
    });
    expect(isAllowedTerminalOrigin(req, '127.0.0.1', 3377, false)).toBe(true);
  });

  test('accepts localhost alias for a loopback-bound server', () => {
    const req = new Request('http://localhost:3377/ws/terminal', {
      headers: { Origin: 'http://localhost:3377' },
    });
    expect(isAllowedTerminalOrigin(req, '127.0.0.1', 3377, false)).toBe(true);
  });

  test('does not trust an attacker-controlled Host origin', () => {
    const req = new Request('http://attacker.example:3377/ws/terminal', {
      headers: { Origin: 'http://attacker.example:3377' },
    });
    expect(isAllowedTerminalOrigin(req, '127.0.0.1', 3377, false)).toBe(false);
  });

  test('accepts a configured HTTPS public origin for reverse proxies', () => {
    const req = new Request('http://127.0.0.1:3377/ws/terminal', {
      headers: { Origin: 'https://hub.example.com' },
    });
    expect(isAllowedTerminalOrigin(
      req,
      '127.0.0.1',
      3377,
      false,
      ['https://hub.example.com'],
    )).toBe(true);
  });

  test('accepts local interface origins for wildcard-bound servers', () => {
    const req = new Request('http://192.168.1.10:3377/ws/terminal', {
      headers: { Origin: 'http://192.168.1.10:3377' },
    });
    expect(isAllowedTerminalOrigin(
      req,
      '0.0.0.0',
      3377,
      false,
      [],
      ['192.168.1.10'],
    )).toBe(true);
  });
});

describe('request Host allowlist', () => {
  test('accepts loopback aliases for a loopback-bound server', () => {
    expect(isAllowedRequestHost(
      new Request('http://127.0.0.1:3377/ws', { headers: { host: 'localhost:3377' } }),
      '127.0.0.1',
      3377,
    )).toBe(true);
    expect(isAllowedRequestHost(
      new Request('http://127.0.0.1:3377/ws', { headers: { host: '[::1]:3377' } }),
      '127.0.0.1',
      3377,
    )).toBe(true);
  });

  test('rejects attacker-controlled Host values', () => {
    const req = new Request('http://127.0.0.1:3377/ws', {
      headers: { host: 'evil.test' },
    });
    expect(isAllowedRequestHost(req, '127.0.0.1', 3377)).toBe(false);
  });

  test('accepts configured public hosts for reverse proxies', () => {
    const req = new Request('http://127.0.0.1:3377/ws', {
      headers: { host: 'hub.example.com' },
    });
    expect(isAllowedRequestHost(
      req,
      '127.0.0.1',
      3377,
      ['https://hub.example.com'],
    )).toBe(true);
  });

  test('accepts local interface hosts for wildcard-bound servers', () => {
    const req = new Request('http://192.168.1.10:3377/ws', {
      headers: { host: '192.168.1.10:3377' },
    });
    expect(isAllowedRequestHost(req, '0.0.0.0', 3377, [], ['192.168.1.10'])).toBe(true);
  });

  test('rejects arbitrary hosts for wildcard-bound servers', () => {
    const req = new Request('http://attacker.example:3377/ws', {
      headers: { host: 'attacker.example:3377' },
    });
    expect(isAllowedRequestHost(req, '0.0.0.0', 3377, [], ['192.168.1.10'])).toBe(false);
  });
});

describe('terminal cwd allowlist', () => {
  const tempDirs: string[] = [];

  function tempDir(prefix: string): string {
    const dir = mkdtempSync(join(tmpdir(), prefix));
    tempDirs.push(dir);
    return dir;
  }

  afterEach(() => {
    while (tempDirs.length > 0) {
      rmSync(tempDirs.pop()!, { recursive: true, force: true });
    }
  });

  test('allows directories under an allowed root', () => {
    const root = tempDir('keepline-root-');
    const project = join(root, 'project');
    mkdirSync(project);

    expect(resolveAllowedTerminalCwd(project, [root])).toBe(realpathSync(project));
  });

  test('rejects directories outside allowed roots', () => {
    const root = tempDir('keepline-root-');
    const outside = tempDir('keepline-outside-');

    expect(() => resolveAllowedTerminalCwd(outside, [root])).toThrow('outside allowed roots');
  });

  test('rejects symlinks that resolve outside allowed roots', () => {
    const root = tempDir('keepline-root-');
    const outside = tempDir('keepline-outside-');
    const link = join(root, 'outside-link');
    symlinkSync(outside, link, 'dir');

    expect(() => resolveAllowedTerminalCwd(link, [root])).toThrow('outside allowed roots');
  });
});
