/**
 * Auth Routes for Web Terminal
 *
 * Handles setup, login, logout, and status checks.
 */

import { Hono } from 'hono';
import { rateLimit, getClientIp } from '../middleware/rateLimit.js';
import { authMiddleware } from '../middleware/auth.js';
import {
  isSetupComplete,
  setupUser,
  login,
  revokeToken,
  logAudit,
} from '../../../services/auth.service.js';
import type { JwtPayload } from '../../../services/auth.service.js';

const auth = new Hono();

// Per-username login attempt counter, in addition to the per-IP rateLimit
// middleware. Bounds brute-force against a single account even when an
// attacker rotates IPs.
const usernameLoginAttempts = new Map<string, { count: number; resetTime: number }>();
const LOGIN_ATTEMPTS_PER_USERNAME = 5;
const LOGIN_USERNAME_WINDOW_MS = 15 * 60 * 1000;

setInterval(() => {
  const now = Date.now();
  for (const [k, v] of usernameLoginAttempts.entries()) {
    if (now > v.resetTime) usernameLoginAttempts.delete(k);
  }
}, LOGIN_USERNAME_WINDOW_MS);

// GET /api/auth/status - Check if setup is complete and if user is authenticated
auth.get('/status', async (c) => {
  const setupComplete = await isSetupComplete();

  // Check if current token is valid
  let authenticated = false;
  let username: string | undefined;
  const authHeader = c.req.header('Authorization');
  if (authHeader?.startsWith('Bearer ')) {
    const { verifyToken } = await import('../../../services/auth.service.js');
    const payload = verifyToken(authHeader.slice(7));
    if (payload) {
      authenticated = true;
      username = payload.username;
    }
  }

  return c.json({ success: true, data: { setupComplete, authenticated, username } });
});

// POST /api/auth/setup - First-run user creation
auth.post('/setup', rateLimit(5, 60 * 1000), async (c) => {
  try {
    const body = await c.req.json();
    const { username, password, enableTotp } = body;

    if (!username || !password) {
      return c.json({ success: false, error: 'Username and password required' }, 400);
    }
    if (username.length < 3 || username.length > 32) {
      return c.json({ success: false, error: 'Username must be 3-32 characters' }, 400);
    }
    if (password.length < 8) {
      return c.json({ success: false, error: 'Password must be at least 8 characters' }, 400);
    }

    const result = await setupUser(username, password, enableTotp);
    const ip = getClientIp(c);
    logAudit(null, 'setup_complete', ip);

    return c.json({ success: true, data: result });
  } catch (e) {
    const message = e instanceof Error ? e.message : 'Setup failed';
    return c.json({ success: false, error: message }, 400);
  }
});

// POST /api/auth/login - Authenticate
auth.post('/login', rateLimit(10, 60 * 1000), async (c) => {
  const ip = getClientIp(c);
  try {
    const body = await c.req.json();
    const { username, password, totpCode } = body;

    if (!username || !password) {
      return c.json({ success: false, error: 'Username and password required' }, 400);
    }

    // Per-username attempt cap (in addition to per-IP middleware above).
    const now = Date.now();
    let record = usernameLoginAttempts.get(username);
    if (!record || now > record.resetTime) {
      record = { count: 0, resetTime: now + LOGIN_USERNAME_WINDOW_MS };
      usernameLoginAttempts.set(username, record);
    }
    if (record.count >= LOGIN_ATTEMPTS_PER_USERNAME) {
      logAudit(null, 'login_locked', ip, `User: ${username}`);
      return c.json({ success: false, error: 'Too many attempts for this account' }, 429);
    }
    record.count++;

    const result = await login(username, password, totpCode);
    // Reset on success so legitimate users are not locked out by retries.
    usernameLoginAttempts.delete(username);
    logAudit(null, 'login_success', ip, `User: ${username}`);

    return c.json({ success: true, data: result });
  } catch (e) {
    logAudit(null, 'login_failed', ip);
    const message = e instanceof Error ? e.message : 'Login failed';
    return c.json({ success: false, error: message }, 401);
  }
});

// POST /api/auth/logout - Revoke current token
auth.post('/logout', authMiddleware, async (c) => {
  const user = (c as any).get('user') as JwtPayload;
  revokeToken(user.jti);
  logAudit(user.sub, 'logout');
  return c.json({ success: true });
});

export default auth;
