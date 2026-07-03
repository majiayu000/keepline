import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import sessions from '../web/api/routes/sessions.js';
import memory from '../web/api/routes/memory.js';
import status from '../web/api/routes/status.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';

describe('Protected API routes', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('sessions route rejects unauthenticated requests', async () => {
    const response = await sessions.fetch(new Request('http://localhost/?skipSync=true&fields=basic'));
    expect(response.status).toBe(401);
  });

  test('sessions route accepts a valid bearer token', async () => {
    const { token } = await setupUser('alice', 'password123');
    const response = await sessions.fetch(new Request('http://localhost/?skipSync=true&fields=basic', {
      headers: { Authorization: `Bearer ${token}` },
    }));

    expect(response.status).toBe(200);
    const body = await response.json() as { success: boolean };
    expect(body.success).toBe(true);
  });

  test('memory route rejects unauthenticated requests', async () => {
    const response = await memory.fetch(new Request('http://localhost/'));
    expect(response.status).toBe(401);
  });

  test('status route rejects unauthenticated hook availability requests', async () => {
    const response = await status.fetch(new Request('http://localhost/hooks'));
    expect(response.status).toBe(401);
  });

  test('memory route accepts a valid bearer token', async () => {
    const { token } = await setupUser('bob', 'password123');
    const response = await memory.fetch(new Request('http://localhost/', {
      headers: { Authorization: `Bearer ${token}` },
    }));

    expect(response.status).toBe(200);
    const body = await response.json() as { success: boolean; data: unknown[] };
    expect(body.success).toBe(true);
    expect(Array.isArray(body.data)).toBe(true);
  });
});
