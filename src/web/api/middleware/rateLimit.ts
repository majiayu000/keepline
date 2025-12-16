/**
 * Rate Limiting Middleware
 *
 * Simple in-memory rate limiter for API endpoints
 */

import { logger } from '../../../lib/logger.js';

// Simple in-memory rate limiter
const rateLimitStore = new Map<string, { count: number; resetTime: number }>();

export function rateLimit(maxRequests: number, windowMs: number) {
  return async (c: any, next: () => Promise<void>) => {
    const ip = c.req.header('x-forwarded-for') || c.req.header('x-real-ip') || 'unknown';
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
