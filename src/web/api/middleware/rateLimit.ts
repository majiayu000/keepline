/**
 * Rate Limiting Middleware
 *
 * Hardened in-memory rate limiter for API endpoints.
 *
 * Design notes:
 * - Default key is the authoritative connection address from Hono's
 *   `getConnInfo` (Bun adapter), so callers cannot bypass the limiter by
 *   spoofing `x-forwarded-for`. Forwarded headers are only honored when
 *   `CLAUDE_HUB_TRUST_PROXY=true`, in which case the *left-most* address of
 *   `x-forwarded-for` is used (RFC 7239 / Express convention).
 * - When neither a connection address nor a trusted forwarded header is
 *   available, the request is bucketed under a stable per-process anonymous
 *   key (`__noaddr__`) so missing metadata cannot silently merge every
 *   client into a single shared bucket.
 * - Each response carries the IETF rate-limit headers
 *   (`X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`).
 *   On a 429 we additionally set `Retry-After` per RFC 6585.
 * - The internal store is bounded by `maxKeys` and uses simple insertion-
 *   order eviction so a flood of unique keys cannot exhaust memory.
 * - The cleanup timer is `unref()`-ed so it does not pin the runtime open
 *   during shutdown.
 * - A custom `keyExtractor` lets callers (e.g. login routes) rate-limit per
 *   username instead of per IP, which materially raises the cost of
 *   credential-stuffing across rotating proxies.
 */

import type { Context, Next } from 'hono';
import { getConnInfo } from 'hono/bun';
import { logger } from '../../../lib/logger.js';

/**
 * Trust forwarded headers? Read on every request so operators can flip it
 * without restarting (and so tests can exercise both modes without splitting
 * across processes). The check is a single env-var read; the cost is
 * negligible relative to the rest of the request path.
 *
 * Off by default — Claude Hub binds to loopback only by default.
 */
function trustProxy(): boolean {
  return process.env.CLAUDE_HUB_TRUST_PROXY === 'true';
}

/** Hard cap on distinct keys before recency eviction kicks in. */
const DEFAULT_MAX_KEYS = 10_000;

/** Stable bucket name for callers we cannot identify by address. */
export const ANON_RATE_LIMIT_KEY = '__noaddr__';

interface RateLimitRecord {
  count: number;
  resetTime: number;
}

/** Map insertion order is preserved; we exploit that for FIFO eviction. */
const rateLimitStore = new Map<string, RateLimitRecord>();
let nextScopeId = 0;

export interface RateLimitOptions {
  /**
   * Custom key extractor (e.g. for per-username login lockout).
   * May return a string or a `Promise<string>`; for async extractors that
   * need to peek at the request body, use `c.req.raw.clone().json()` so that
   * downstream handlers can still read the original body.
   */
  keyExtractor?: (c: Context) => string | Promise<string>;
  /**
   * Namespace this limiter instance. Defaults to a unique per-middleware
   * scope so global and route-local limiters do not double-count each other.
   */
  scope?: string;
  /** Hard cap for the in-memory store. Defaults to 10k keys. */
  maxKeys?: number;
}

/**
 * Read the *first* (left-most) address from a forwarded header value.
 * Per RFC 7239 / X-Forwarded-For convention, the originating client is the
 * left-most entry; intermediate proxies append on the right. Only consulted
 * when `TRUST_PROXY` is enabled.
 */
function firstForwardedAddress(headerValue: string | undefined): string | undefined {
  if (!headerValue) return undefined;
  const first = headerValue.split(',')[0]?.trim();
  return first ? first : undefined;
}

/** Resolve the authoritative remote address for a request. */
export function getClientIp(c: Context): string {
  if (trustProxy()) {
    const fwd =
      firstForwardedAddress(c.req.header('x-forwarded-for')) ||
      c.req.header('x-real-ip');
    if (fwd) return fwd;
  }

  // Fall back to the actual TCP peer that opened the socket.
  try {
    const info = getConnInfo(c);
    const addr = info.remote?.address;
    if (addr) return addr;
  } catch {
    // getConnInfo may throw if the request is synthesised in tests.
  }

  return ANON_RATE_LIMIT_KEY;
}

/** Bound the store size via simple insertion-order (FIFO) eviction. */
function evictIfNeeded(maxKeys: number): void {
  if (rateLimitStore.size <= maxKeys) return;
  const overflow = rateLimitStore.size - maxKeys;
  let removed = 0;
  for (const key of rateLimitStore.keys()) {
    if (removed >= overflow) break;
    rateLimitStore.delete(key);
    removed++;
  }
}

export function rateLimit(
  maxRequests: number,
  windowMs: number,
  options: RateLimitOptions = {}
) {
  const { keyExtractor, maxKeys = DEFAULT_MAX_KEYS } = options;
  const scope = options.scope ?? `limit:${++nextScopeId}`;

  return async (c: Context, next: Next) => {
    const rawKey = keyExtractor
      ? await keyExtractor(c)
      : `ip:${getClientIp(c)}`;
    const key = `${scope}:${rawKey}`;
    const now = Date.now();

    let record = rateLimitStore.get(key);
    if (!record || now > record.resetTime) {
      record = { count: 0, resetTime: now + windowMs };
      // Re-insert to refresh recency for FIFO eviction.
      rateLimitStore.delete(key);
      rateLimitStore.set(key, record);
      evictIfNeeded(maxKeys);
    }

    record.count++;

    const remaining = Math.max(0, maxRequests - record.count);
    const resetSeconds = Math.ceil(record.resetTime / 1000);

    // IETF rate-limit headers, present on every response.
    c.header('X-RateLimit-Limit', String(maxRequests));
    c.header('X-RateLimit-Remaining', String(remaining));
    c.header('X-RateLimit-Reset', String(resetSeconds));

    if (record.count > maxRequests) {
      const retryAfter = Math.max(1, Math.ceil((record.resetTime - now) / 1000));
      c.header('Retry-After', String(retryAfter));
      // Don't log the raw key (may be PII); log scope only.
      logger.warn('Rate limit exceeded', {
        scope,
        keyType: keyExtractor ? 'custom' : 'address',
        count: record.count,
        limit: maxRequests,
      });
      return c.json(
        { success: false, error: 'Too many requests' },
        429
      );
    }

    await next();
  };
}

/** Clear one bucket, used after successful auth to count failures only. */
export function resetRateLimitKey(scope: string, key: string): void {
  rateLimitStore.delete(`${scope}:${key}`);
}

/** Reset the in-memory store. Test-only helper. */
export function __resetRateLimitStoreForTests(): void {
  rateLimitStore.clear();
}

/** Inspect the current store size. Test-only helper. */
export function __getRateLimitStoreSizeForTests(): number {
  return rateLimitStore.size;
}

// Periodic GC: drop expired records every 5 minutes. `unref()` so we don't
// keep the Bun event loop alive on shutdown.
const cleanupTimer = setInterval(() => {
  const now = Date.now();
  for (const [key, value] of rateLimitStore.entries()) {
    if (now > value.resetTime) {
      rateLimitStore.delete(key);
    }
  }
}, 5 * 60 * 1000);

// Bun's setInterval handle exposes `unref` but the global `Timer` typing
// does not always include it — guard at runtime.
type MaybeUnref = { unref?: () => void };
(cleanupTimer as unknown as MaybeUnref).unref?.();
