/**
 * Integration tests for the hardened rate limiter.
 *
 * Covers:
 * - Authoritative IP cannot be spoofed via x-forwarded-for in default
 *   (TRUST_PROXY=false) mode.
 * - Standard IETF rate-limit headers are emitted on every request.
 * - 429 responses carry Retry-After.
 * - Custom keyExtractor (sync + async) buckets independently.
 * - Store size stays bounded under unique-key flooding.
 *
 * No mocks: each test wires a fresh Hono app to the real middleware.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { Hono } from 'hono';
import {
  rateLimit,
  resetRateLimitKey,
  __resetRateLimitStoreForTests,
  __getRateLimitStoreSizeForTests,
} from '../web/api/middleware/rateLimit.js';

function buildApp(maxRequests: number, windowMs: number) {
  const app = new Hono();
  app.use('*', rateLimit(maxRequests, windowMs));
  app.get('/ping', (c) => c.json({ ok: true }));
  return app;
}

describe('rateLimit middleware', () => {
  beforeEach(() => {
    __resetRateLimitStoreForTests();
    delete process.env.CLAUDE_HUB_TRUST_PROXY;
  });

  afterEach(() => {
    __resetRateLimitStoreForTests();
    delete process.env.CLAUDE_HUB_TRUST_PROXY;
  });

  test('emits IETF rate-limit headers on a normal response', async () => {
    const app = buildApp(3, 60_000);
    const res = await app.fetch(new Request('http://localhost/ping'));
    expect(res.status).toBe(200);
    expect(res.headers.get('X-RateLimit-Limit')).toBe('3');
    // First request burns 1, so 2 remain.
    expect(res.headers.get('X-RateLimit-Remaining')).toBe('2');
    expect(res.headers.get('X-RateLimit-Reset')).toMatch(/^\d+$/);
    expect(res.headers.get('Retry-After')).toBeNull();
  });

  test('returns 429 with Retry-After once the limit is exceeded', async () => {
    const app = buildApp(2, 60_000);

    const a = await app.fetch(new Request('http://localhost/ping'));
    const b = await app.fetch(new Request('http://localhost/ping'));
    const c = await app.fetch(new Request('http://localhost/ping'));

    expect(a.status).toBe(200);
    expect(b.status).toBe(200);
    expect(c.status).toBe(429);
    expect(c.headers.get('Retry-After')).toMatch(/^\d+$/);
    const body = (await c.json()) as { success: boolean; error: string };
    expect(body.success).toBe(false);
    expect(body.error).toBe('Too many requests');
  });

  test('does NOT trust x-forwarded-for by default — spoofing cannot bypass limit', async () => {
    // TRUST_PROXY is false (default). All requests should land in the same
    // anonymous bucket regardless of forwarded values, because synthesised
    // Request objects in tests have no socket peer.
    const app = buildApp(2, 60_000);

    const r1 = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '1.1.1.1' },
      })
    );
    const r2 = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '2.2.2.2' },
      })
    );
    const r3 = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '3.3.3.3' },
      })
    );

    expect(r1.status).toBe(200);
    expect(r2.status).toBe(200);
    expect(r3.status).toBe(429);
  });

  test('honours x-forwarded-for ONLY when CLAUDE_HUB_TRUST_PROXY=true', async () => {
    process.env.CLAUDE_HUB_TRUST_PROXY = 'true';
    const app = buildApp(1, 60_000);

    const a = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '1.1.1.1' },
      })
    );
    const b = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '2.2.2.2' },
      })
    );

    // Different forwarded clients, so each has its own bucket.
    expect(a.status).toBe(200);
    expect(b.status).toBe(200);

    // Same forwarded client second time: blocked.
    const c = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '1.1.1.1' },
      })
    );
    expect(c.status).toBe(429);
  });

  test('uses left-most address from a comma-separated x-forwarded-for', async () => {
    process.env.CLAUDE_HUB_TRUST_PROXY = 'true';
    const app = buildApp(1, 60_000);

    // Both requests claim originating client 9.9.9.9, with different proxy
    // chains appended. The left-most address must be the bucket key.
    const a = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '9.9.9.9, 10.0.0.1' },
      })
    );
    const b = await app.fetch(
      new Request('http://localhost/ping', {
        headers: { 'x-forwarded-for': '9.9.9.9, 10.0.0.2' },
      })
    );

    expect(a.status).toBe(200);
    expect(b.status).toBe(429);
  });

  test('custom synchronous keyExtractor buckets independently', async () => {
    const app = new Hono();
    app.use(
      '*',
      rateLimit(1, 60_000, {
        keyExtractor: (c) => `tag:${c.req.header('x-tag') ?? 'none'}`,
      })
    );
    app.get('/ping', (c) => c.json({ ok: true }));

    const a = await app.fetch(
      new Request('http://localhost/ping', { headers: { 'x-tag': 'alice' } })
    );
    const b = await app.fetch(
      new Request('http://localhost/ping', { headers: { 'x-tag': 'bob' } })
    );
    const a2 = await app.fetch(
      new Request('http://localhost/ping', { headers: { 'x-tag': 'alice' } })
    );

    expect(a.status).toBe(200);
    expect(b.status).toBe(200);
    expect(a2.status).toBe(429);
  });

  test('separate middleware instances do not double-count the same address', async () => {
    const app = new Hono();
    app.use('*', rateLimit(100, 60_000));
    app.get('/login', rateLimit(2, 60_000), (c) => c.json({ ok: true }));

    const first = await app.fetch(new Request('http://localhost/login'));
    const second = await app.fetch(new Request('http://localhost/login'));
    const third = await app.fetch(new Request('http://localhost/login'));

    expect(first.status).toBe(200);
    expect(second.status).toBe(200);
    expect(third.status).toBe(429);
  });

  test('async keyExtractor (e.g. body-derived) is awaited', async () => {
    const app = new Hono();
    app.use(
      '*',
      rateLimit(1, 60_000, {
        keyExtractor: async (c) => {
          const body = (await c.req.raw.clone().json()) as { user?: string };
          return `u:${body.user ?? 'anon'}`;
        },
      })
    );
    app.post('/login', (c) => c.json({ ok: true }));

    const post = (user: string) =>
      app.fetch(
        new Request('http://localhost/login', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ user }),
        })
      );

    const a = await post('alice');
    const b = await post('bob');
    const a2 = await post('alice');

    expect(a.status).toBe(200);
    expect(b.status).toBe(200);
    expect(a2.status).toBe(429);
  });

  test('store size is bounded by maxKeys', async () => {
    __resetRateLimitStoreForTests();

    const app = new Hono();
    app.use(
      '*',
      rateLimit(100, 60_000, {
        keyExtractor: (c) => `k:${c.req.header('x-k')}`,
        maxKeys: 5,
      })
    );
    app.get('/ping', (c) => c.json({ ok: true }));

    for (let i = 0; i < 50; i++) {
      await app.fetch(
        new Request('http://localhost/ping', {
          headers: { 'x-k': String(i) },
        })
      );
    }

    expect(__getRateLimitStoreSizeForTests()).toBeLessThanOrEqual(5);
  });

  test('resetRateLimitKey clears one scoped bucket', async () => {
    const scope = 'test-login';
    const app = new Hono();
    app.use(
      '*',
      rateLimit(1, 60_000, {
        scope,
        keyExtractor: (c) => `u:${c.req.header('x-user')}`,
      })
    );
    app.get('/login', (c) => c.json({ ok: true }));

    const a = await app.fetch(
      new Request('http://localhost/login', { headers: { 'x-user': 'alice' } })
    );
    const b = await app.fetch(
      new Request('http://localhost/login', { headers: { 'x-user': 'alice' } })
    );
    resetRateLimitKey(scope, 'u:alice');
    const c = await app.fetch(
      new Request('http://localhost/login', { headers: { 'x-user': 'alice' } })
    );

    expect(a.status).toBe(200);
    expect(b.status).toBe(429);
    expect(c.status).toBe(200);
  });
});
