/**
 * Auth Routes for Web Terminal
 *
 * Handles setup, login, logout, and status checks.
 */

import { Hono } from 'hono';
import { rateLimit } from '../middleware/rateLimit.js';
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
    const ip = c.req.header('x-forwarded-for') || c.req.header('x-real-ip') || 'unknown';
    logAudit(null, 'setup_complete', ip);

    return c.json({ success: true, data: result });
  } catch (e) {
    const message = e instanceof Error ? e.message : 'Setup failed';
    return c.json({ success: false, error: message }, 400);
  }
});

// POST /api/auth/login - Authenticate
auth.post('/login', rateLimit(10, 60 * 1000), async (c) => {
  try {
    const body = await c.req.json();
    const { username, password, totpCode } = body;

    if (!username || !password) {
      return c.json({ success: false, error: 'Username and password required' }, 400);
    }

    const result = await login(username, password, totpCode);
    const ip = c.req.header('x-forwarded-for') || c.req.header('x-real-ip') || 'unknown';
    logAudit(null, 'login_success', ip, `User: ${username}`);

    return c.json({ success: true, data: result });
  } catch (e) {
    const ip = c.req.header('x-forwarded-for') || c.req.header('x-real-ip') || 'unknown';
    logAudit(null, 'login_failed', ip);
    const message = e instanceof Error ? e.message : 'Login failed';
    return c.json({ success: false, error: message }, 401);
  }
});

// POST /api/auth/logout - Revoke current token
auth.post('/logout', authMiddleware, async (c) => {
  const user = c.get('user') as JwtPayload;
  revokeToken(user.jti);
  logAudit(user.sub, 'logout');
  return c.json({ success: true });
});

export default auth;
