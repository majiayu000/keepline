import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import auth from '../web/api/routes/auth.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { __resetRateLimitStoreForTests } from '../web/api/middleware/rateLimit.js';

interface LocalLoginResponse {
  success: boolean;
  data?: {
    token?: unknown;
  };
}

async function parseJson(response: Response): Promise<LocalLoginResponse> {
  return (await response.json()) as LocalLoginResponse;
}

async function postFromLoopback(): Promise<{ status: number; body: LocalLoginResponse }> {
  const server = Bun.serve({
    hostname: '127.0.0.1',
    port: 0,
    fetch(req, server) {
      return auth.fetch(req, { server });
    },
  });

  try {
    const response = await fetch(`http://127.0.0.1:${server.port}/local`, {
      method: 'POST',
    });
    return { status: response.status, body: await parseJson(response) };
  } finally {
    server.stop(true);
  }
}

describe('local auth route', () => {
  beforeEach(() => {
    resetDatabase();
    __resetRateLimitStoreForTests();
    delete process.env.CLAUDE_HUB_HOST;
    delete process.env.CLAUDE_HUB_PUBLIC_ORIGIN;
    delete process.env.CLAUDE_HUB_ALLOWED_ORIGINS;
    delete process.env.CLAUDE_HUB_TRUST_PROXY;
  });

  afterEach(() => {
    __resetRateLimitStoreForTests();
    delete process.env.CLAUDE_HUB_HOST;
    delete process.env.CLAUDE_HUB_PUBLIC_ORIGIN;
    delete process.env.CLAUDE_HUB_ALLOWED_ORIGINS;
    delete process.env.CLAUDE_HUB_TRUST_PROXY;
    closeDatabase();
  });

  test('rejects a spoofed Host header when there is no loopback TCP peer', async () => {
    const response = await auth.fetch(
      new Request('http://public.example/local', {
        method: 'POST',
        headers: { host: 'localhost:3377' },
      })
    );

    expect(response.status).toBe(403);
    const body = await parseJson(response);
    expect(body.success).toBe(false);
    expect(body.data?.token).toBeUndefined();
  });

  test('does not treat trusted proxy forwarded headers as local login proof', async () => {
    process.env.CLAUDE_HUB_TRUST_PROXY = 'true';

    const response = await auth.fetch(
      new Request('http://public.example/local', {
        method: 'POST',
        headers: {
          host: 'public.example',
          'x-forwarded-for': '127.0.0.1',
          'x-real-ip': '127.0.0.1',
        },
      })
    );

    expect(response.status).toBe(403);
    const body = await parseJson(response);
    expect(body.success).toBe(false);
    expect(body.data?.token).toBeUndefined();
  });

  test('rejects local login when the server is configured for public binding', async () => {
    process.env.CLAUDE_HUB_HOST = '0.0.0.0';

    const response = await postFromLoopback();

    expect(response.status).toBe(403);
    expect(response.body.success).toBe(false);
    expect(response.body.data?.token).toBeUndefined();
  });

  test('rejects local login when public proxy origins are configured', async () => {
    process.env.CLAUDE_HUB_PUBLIC_ORIGIN = 'https://hub.example.com';

    const response = await postFromLoopback();

    expect(response.status).toBe(403);
    expect(response.body.success).toBe(false);
    expect(response.body.data?.token).toBeUndefined();
  });

  test('allows local login from an actual loopback socket', async () => {
    const response = await postFromLoopback();

    expect(response.status).toBe(200);
    expect(response.body.success).toBe(true);
    expect(typeof response.body.data?.token).toBe('string');
  });
});
