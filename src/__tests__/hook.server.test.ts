import { afterEach, describe, expect, test } from 'bun:test';
import type { FastifyInstance } from 'fastify';
import { createHookServer } from '../adapters/hook/server.js';

describe('hook server request security', () => {
  let server: FastifyInstance | null = null;

  afterEach(async () => {
    if (server) {
      await server.close();
      server = null;
    }
  });

  function app(): FastifyInstance {
    server = createHookServer();
    return server;
  }

  test('rejects non-loopback Host headers before hook validation', async () => {
    const response = await app().inject({
      method: 'POST',
      url: '/hook',
      headers: {
        host: 'attacker.example',
        'content-type': 'application/json',
      },
      payload: {},
    });

    expect(response.statusCode).toBe(403);
    const body = response.json() as { success: boolean; error: string };
    expect(body).toEqual({ success: false, error: 'Forbidden' });
  });

  test('rejects cross-origin context reads', async () => {
    const response = await app().inject({
      method: 'GET',
      url: '/context?path=/tmp/project',
      headers: {
        host: '127.0.0.1:7890',
        origin: 'https://attacker.example',
      },
    });

    expect(response.statusCode).toBe(403);
  });

  test('accepts loopback health requests', async () => {
    const response = await app().inject({
      method: 'GET',
      url: '/health',
      headers: {
        host: 'localhost:7890',
      },
    });

    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ status: 'ok' });
  });

  test('allows loopback hook requests to reach payload validation', async () => {
    const response = await app().inject({
      method: 'POST',
      url: '/hook',
      headers: {
        host: '127.0.0.1:7890',
        'content-type': 'application/json',
      },
      payload: {},
    });

    expect(response.statusCode).toBe(400);
    const body = response.json() as { success: boolean; error: string };
    expect(body).toEqual({
      success: false,
      error: 'Invalid hook event payload',
    });
  });

  test('rejects cross-site browser fetch metadata', async () => {
    const response = await app().inject({
      method: 'POST',
      url: '/hook',
      headers: {
        host: '127.0.0.1:7890',
        'sec-fetch-site': 'cross-site',
        'content-type': 'application/json',
      },
      payload: {},
    });

    expect(response.statusCode).toBe(403);
  });
});
