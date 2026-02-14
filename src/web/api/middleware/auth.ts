/**
 * Auth Middleware for Terminal API
 *
 * Extracts Bearer token, verifies JWT, sets user context.
 */

import type { Context, Next } from 'hono';
import { verifyToken } from '../../../services/auth.service.js';

export async function authMiddleware(c: Context, next: Next) {
  const authHeader = c.req.header('Authorization');
  if (!authHeader?.startsWith('Bearer ')) {
    return c.json({ success: false, error: 'Unauthorized' }, 401);
  }

  const token = authHeader.slice(7);
  const payload = verifyToken(token);
  if (!payload) {
    return c.json({ success: false, error: 'Invalid or expired token' }, 401);
  }

  c.set('user', payload);
  c.set('token', token);
  await next();
}
