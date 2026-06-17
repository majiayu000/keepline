import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import auth from '../web/api/routes/auth.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { __resetRateLimitStoreForTests } from '../web/api/middleware/rateLimit.js';

function loginRequest(password: string, forwardedFor: string): Request {
  return new Request('http://localhost/login', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-forwarded-for': forwardedFor,
    },
    body: JSON.stringify({
      username: 'alice',
      password,
    }),
  });
}

describe('auth login rate limiting', () => {
  beforeEach(async () => {
    resetDatabase();
    __resetRateLimitStoreForTests();
    process.env.KEEPLINE_TRUST_PROXY = 'true';
    await setupUser('alice', 'password123');
  });

  afterEach(() => {
    __resetRateLimitStoreForTests();
    delete process.env.KEEPLINE_TRUST_PROXY;
    closeDatabase();
  });

  test('locks a username across rotating trusted proxy addresses', async () => {
    for (let i = 0; i < 10; i++) {
      const response = await auth.fetch(loginRequest('wrong-password', `10.0.0.${i}`));
      expect(response.status).toBe(401);
    }

    const locked = await auth.fetch(loginRequest('wrong-password', '10.0.0.99'));
    expect(locked.status).toBe(429);
  });

  test('successful login resets the per-username failure bucket', async () => {
    for (let i = 0; i < 9; i++) {
      const response = await auth.fetch(loginRequest('wrong-password', `10.0.1.${i}`));
      expect(response.status).toBe(401);
    }

    const success = await auth.fetch(loginRequest('password123', '10.0.1.50'));
    expect(success.status).toBe(200);

    const afterReset = await auth.fetch(loginRequest('wrong-password', '10.0.1.51'));
    expect(afterReset.status).toBe(401);
  });
});
