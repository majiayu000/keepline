/**
 * Rate Limiting Middleware
 *
 * Simple in-memory rate limiter for API endpoints
 */

import { getConnInfo } from 'hono/bun';
import { logger } from '../../../lib/logger.js';

// Simple in-memory rate limiter
const rateLimitStore = new Map<string, { count: number; resetTime: number }>();

/**
 * Resolve the client IP for rate-limiting and audit logging.
 *
 * `x-forwarded-for` / `x-real-ip` are client-controlled when no proxy
 * fronts the server, so an attacker on the same loopback can rotate the
 * header to bypass per-IP limits. Only honor those headers when
 * `TRUSTED_PROXY=true` is explicitly set; otherwise fall back to the
 * actual socket peer reported by Bun.
 */
export function getClientIp(c: any): string {
  if (process.env.TRUSTED_PROXY === 'true') {
    const xff = c.req.header('x-forwarded-for');
    if (xff) return xff.split(',')[0]!.trim();
    const xri = c.req.header('x-real-ip');
    if (xri) return xri;
  }
  try {
    return getConnInfo(c).remote.address ?? 'unknown';
  } catch {
    return 'unknown';
  }
}

export function rateLimit(maxRequests: number, windowMs: number) {
  return async (c: any, next: () => Promise<void>) => {
    const ip = getClientIp(c);
    const now = Date.now();

    let record = rateLimitStore.get(ip);
    if (!record || now > record.resetTime) {
      record = { count: 0, resetTime: now + windowMs };
      rateLimitStore.set(ip, record);
    }

    record.count++;

    if (record.count > maxRequests) {
      logger.warn('Rate limit exceeded', { ip, count: record.count });
      return c.json({ success: false, error: 'Too many requests' }, 429);
    }

    await next();
  };
}

// Clean up rate limit store periodically (every 5 minutes)
setInterval(() => {
  const now = Date.now();
  for (const [key, value] of rateLimitStore.entries()) {
    if (now > value.resetTime) {
      rateLimitStore.delete(key);
    }
  }
}, 5 * 60 * 1000);
