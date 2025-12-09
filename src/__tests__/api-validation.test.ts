/**
 * Tests for API input validation
 */

import { describe, test, expect } from 'bun:test';

// Re-implement validation functions for testing (they're not exported)
const VALID_RECOVERY_METHODS = ['resume', 'continue', 'new'] as const;

function isValidSessionId(id: string): boolean {
  return typeof id === 'string' && /^[a-zA-Z0-9-_]{8,64}$/.test(id);
}

interface RecoverRequestBody {
  method?: 'resume' | 'continue' | 'new';
  openTerminal?: boolean;
  skipPermissions?: boolean;
}

function validateRecoverRequest(body: unknown): { valid: true; data: RecoverRequestBody } | { valid: false; error: string } {
  if (body === null || typeof body !== 'object') {
    return { valid: true, data: {} };
  }

  const obj = body as Record<string, unknown>;

  if (obj.method !== undefined) {
    if (typeof obj.method !== 'string' || !VALID_RECOVERY_METHODS.includes(obj.method as any)) {
      return { valid: false, error: `Invalid method. Must be one of: ${VALID_RECOVERY_METHODS.join(', ')}` };
    }
  }

  if (obj.openTerminal !== undefined && typeof obj.openTerminal !== 'boolean') {
    return { valid: false, error: 'openTerminal must be a boolean' };
  }

  if (obj.skipPermissions !== undefined && typeof obj.skipPermissions !== 'boolean') {
    return { valid: false, error: 'skipPermissions must be a boolean' };
  }

  return {
    valid: true,
    data: {
      method: obj.method as RecoverRequestBody['method'],
      openTerminal: obj.openTerminal as boolean | undefined,
      skipPermissions: obj.skipPermissions as boolean | undefined,
    },
  };
}

function validateStopRequest(body: unknown): { valid: true; data: { force?: boolean } } | { valid: false; error: string } {
  if (body === null || typeof body !== 'object') {
    return { valid: true, data: {} };
  }

  const obj = body as Record<string, unknown>;

  if (obj.force !== undefined && typeof obj.force !== 'boolean') {
    return { valid: false, error: 'force must be a boolean' };
  }

  return { valid: true, data: { force: obj.force as boolean | undefined } };
}

describe('isValidSessionId', () => {
  test('accepts valid UUIDs', () => {
    expect(isValidSessionId('550e8400-e29b-41d4-a716-446655440000')).toBe(true);
  });

  test('accepts alphanumeric strings of valid length', () => {
    expect(isValidSessionId('abc12345')).toBe(true);
    expect(isValidSessionId('abcdefghij1234567890')).toBe(true);
  });

  test('accepts underscores and hyphens', () => {
    expect(isValidSessionId('abc-123_def')).toBe(true);
  });

  test('rejects too short strings', () => {
    expect(isValidSessionId('abc')).toBe(false);
    expect(isValidSessionId('1234567')).toBe(false);
  });

  test('rejects too long strings', () => {
    expect(isValidSessionId('a'.repeat(65))).toBe(false);
  });

  test('rejects strings with invalid characters', () => {
    expect(isValidSessionId('abc@123')).toBe(false);
    expect(isValidSessionId('abc 123')).toBe(false);
    expect(isValidSessionId('abc.123')).toBe(false);
  });

  test('rejects non-strings', () => {
    expect(isValidSessionId(123 as any)).toBe(false);
    expect(isValidSessionId(null as any)).toBe(false);
  });
});

describe('validateRecoverRequest', () => {
  test('accepts empty body', () => {
    const result = validateRecoverRequest({});
    expect(result.valid).toBe(true);
  });

  test('accepts null body', () => {
    const result = validateRecoverRequest(null);
    expect(result.valid).toBe(true);
  });

  test('accepts valid method "resume"', () => {
    const result = validateRecoverRequest({ method: 'resume' });
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.data.method).toBe('resume');
    }
  });

  test('accepts valid method "continue"', () => {
    const result = validateRecoverRequest({ method: 'continue' });
    expect(result.valid).toBe(true);
  });

  test('accepts valid method "new"', () => {
    const result = validateRecoverRequest({ method: 'new' });
    expect(result.valid).toBe(true);
  });

  test('rejects invalid method', () => {
    const result = validateRecoverRequest({ method: 'invalid' });
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.error).toContain('Invalid method');
    }
  });

  test('accepts boolean openTerminal', () => {
    const result = validateRecoverRequest({ openTerminal: true });
    expect(result.valid).toBe(true);
  });

  test('rejects non-boolean openTerminal', () => {
    const result = validateRecoverRequest({ openTerminal: 'true' });
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.error).toContain('openTerminal');
    }
  });

  test('accepts boolean skipPermissions', () => {
    const result = validateRecoverRequest({ skipPermissions: false });
    expect(result.valid).toBe(true);
  });

  test('rejects non-boolean skipPermissions', () => {
    const result = validateRecoverRequest({ skipPermissions: 1 });
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.error).toContain('skipPermissions');
    }
  });

  test('accepts full valid request', () => {
    const result = validateRecoverRequest({
      method: 'resume',
      openTerminal: true,
      skipPermissions: false,
    });
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.data.method).toBe('resume');
      expect(result.data.openTerminal).toBe(true);
      expect(result.data.skipPermissions).toBe(false);
    }
  });
});

describe('validateStopRequest', () => {
  test('accepts empty body', () => {
    const result = validateStopRequest({});
    expect(result.valid).toBe(true);
  });

  test('accepts null body', () => {
    const result = validateStopRequest(null);
    expect(result.valid).toBe(true);
  });

  test('accepts boolean force true', () => {
    const result = validateStopRequest({ force: true });
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.data.force).toBe(true);
    }
  });

  test('accepts boolean force false', () => {
    const result = validateStopRequest({ force: false });
    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.data.force).toBe(false);
    }
  });

  test('rejects non-boolean force', () => {
    const result = validateStopRequest({ force: 'true' });
    expect(result.valid).toBe(false);
    if (!result.valid) {
      expect(result.error).toContain('force');
    }
  });

  test('rejects number force', () => {
    const result = validateStopRequest({ force: 1 });
    expect(result.valid).toBe(false);
  });
});
