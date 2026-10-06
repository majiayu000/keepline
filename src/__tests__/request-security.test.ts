import { describe, expect, test } from 'bun:test';
import { isAllowedRequestHost,webContentSecurityPolicy } from '../web/api/request-security.js';

describe('request Host allowlist', () => {
  test('served web pages restrict scripts and sockets to their origin',() => {
    const policy = webContentSecurityPolicy('http://127.0.0.1:5572/menubar');
    expect(policy).toContain("script-src 'self'");
    expect(policy).toContain("connect-src 'self' ws://127.0.0.1:5572 ipc: http://ipc.localhost");
    expect(policy).not.toContain('unsafe-eval'); expect(policy).not.toContain('127.0.0.1:*');
  });
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
