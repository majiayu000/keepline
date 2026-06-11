/**
 * Tests for JWT token verification edge cases.
 *
 * Focus on the timing-attack hardening of `verifyToken`:
 * - Forged signatures of *correct length* but wrong content are rejected.
 * - Forged signatures of *wrong length* are rejected without throwing.
 * - Tampered payloads (re-encoded body) are rejected because the signature
 *   no longer matches the recomputed HMAC.
 * - Valid tokens still verify.
 *
 * We do not measure wall-clock timing here (flaky in CI). The point of
 * `timingSafeEqual` is correctness *and* uniform timing; correctness is the
 * directly testable guarantee.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { login, setupUser, verifyToken } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, execSql } from '../infrastructure/database/sqlite.js';

describe('verifyToken hardening', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('accepts a freshly-issued token', async () => {
    const { token } = await setupUser('alice', 'password123');
    const payload = verifyToken(token);
    expect(payload).not.toBeNull();
    expect(payload?.username).toBe('alice');
  });

  test('rejects a token with the same length but wrong signature', async () => {
    const { token } = await setupUser('alice', 'password123');
    const [header, body, signature] = token.split('.');
    expect(signature).toBeDefined();

    // Flip every character of the signature — preserves length, breaks content.
    const forged = signature
      .split('')
      .map((ch) => (ch === 'A' ? 'B' : 'A'))
      .join('');
    expect(forged.length).toBe(signature.length);

    const tampered = `${header}.${body}.${forged}`;
    expect(verifyToken(tampered)).toBeNull();
  });

  test('rejects a token whose signature is the wrong length without throwing', async () => {
    const { token } = await setupUser('alice', 'password123');
    const [header, body] = token.split('.');

    // Empty signature.
    expect(() => verifyToken(`${header}.${body}.`)).not.toThrow();
    expect(verifyToken(`${header}.${body}.`)).toBeNull();

    // Truncated signature (one byte short).
    const truncated = `${header}.${body}.shortsig`;
    expect(verifyToken(truncated)).toBeNull();

    // Over-long garbage signature.
    const overlong = `${header}.${body}.${'a'.repeat(1024)}`;
    expect(verifyToken(overlong)).toBeNull();
  });

  test('rejects a token with a re-encoded payload that drops exp', async () => {
    const { token } = await setupUser('alice', 'password123');
    const [header, , signature] = token.split('.');

    // Try to forge a body claiming admin without re-signing.
    const evilBody = Buffer.from(
      JSON.stringify({
        sub: 'attacker',
        username: 'attacker',
        iat: 0,
        exp: 9_999_999_999,
        jti: 'attacker',
      })
    ).toString('base64url');

    const forged = `${header}.${evilBody}.${signature}`;
    expect(verifyToken(forged)).toBeNull();
  });

  test('rejects garbage input shapes gracefully', () => {
    expect(verifyToken('')).toBeNull();
    expect(verifyToken('not-a-token')).toBeNull();
    expect(verifyToken('a.b')).toBeNull();
    expect(verifyToken('a.b.c.d')).toBeNull();
  });

  test('login fails closed when audit logging is unavailable', async () => {
    await setupUser('alice', 'password123');
    execSql('DROP TABLE terminal_audit_log');

    await expect(login('alice', 'password123')).rejects.toThrow('Failed to write audit log');
  });
});
