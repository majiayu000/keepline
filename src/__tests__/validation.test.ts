/**
 * Integration tests for API input validation
 *
 * These tests verify validation behavior using real data.
 * Following best practices:
 * - Test features, not implementation details
 * - Use real objects, no mocks
 * - Test observable behavior
 */

import { describe, test, expect } from 'bun:test';
import {
  isValidObservationId,
  isValidSessionId,
  validateRecoverRequest,
  validateStopRequest,
  VALID_RECOVERY_METHODS,
} from '../web/api/middleware/validation.js';

describe('isValidSessionId', () => {
  describe('valid session IDs', () => {
    test('accepts standard UUIDs', () => {
      expect(isValidSessionId('550e8400-e29b-41d4-a716-446655440000')).toBe(true);
    });

    test('accepts alphanumeric strings of minimum length (8)', () => {
      expect(isValidSessionId('abc12345')).toBe(true);
    });

    test('accepts alphanumeric strings of maximum length (64)', () => {
      expect(isValidSessionId('a'.repeat(64))).toBe(true);
    });

    test('accepts strings with underscores', () => {
      expect(isValidSessionId('session_id_123')).toBe(true);
    });

    test('accepts strings with hyphens', () => {
      expect(isValidSessionId('session-id-123')).toBe(true);
    });

    test('accepts mixed alphanumeric with hyphens and underscores', () => {
      expect(isValidSessionId('abc-123_DEF-456_xyz')).toBe(true);
    });
  });

  describe('invalid session IDs', () => {
    test('rejects strings shorter than 8 characters', () => {
      expect(isValidSessionId('abc1234')).toBe(false);
      expect(isValidSessionId('short')).toBe(false);
      expect(isValidSessionId('')).toBe(false);
    });

    test('rejects strings longer than 64 characters', () => {
      expect(isValidSessionId('a'.repeat(65))).toBe(false);
    });

    test('rejects strings with spaces', () => {
      expect(isValidSessionId('abc 12345')).toBe(false);
    });

    test('rejects strings with special characters', () => {
      expect(isValidSessionId('abc@12345')).toBe(false);
      expect(isValidSessionId('abc.12345')).toBe(false);
      expect(isValidSessionId('abc!12345')).toBe(false);
      expect(isValidSessionId('abc#12345')).toBe(false);
    });

    test('rejects non-string types', () => {
      expect(isValidSessionId(123 as unknown as string)).toBe(false);
      expect(isValidSessionId(null as unknown as string)).toBe(false);
      expect(isValidSessionId(undefined as unknown as string)).toBe(false);
      expect(isValidSessionId({} as unknown as string)).toBe(false);
    });
  });
});

describe('isValidObservationId', () => {
  describe('valid observation IDs', () => {
    test('accepts UUIDs generated for observations', () => {
      expect(isValidObservationId('550e8400-e29b-41d4-a716-446655440000')).toBe(true);
    });

    test('accepts safe alphanumeric IDs with hyphens and underscores', () => {
      expect(isValidObservationId('obs_12345')).toBe(true);
      expect(isValidObservationId('obs-12345_ABC')).toBe(true);
      expect(isValidObservationId('a'.repeat(64))).toBe(true);
    });
  });

  describe('invalid observation IDs', () => {
    test('rejects injection-shaped filter predicates', () => {
      expect(isValidObservationId("x' OR '1'='1")).toBe(false);
      expect(isValidObservationId("obs12345'; DELETE FROM observations; --")).toBe(false);
    });

    test('rejects unsafe characters and invalid lengths', () => {
      expect(isValidObservationId('abc1234')).toBe(false);
      expect(isValidObservationId('')).toBe(false);
      expect(isValidObservationId('a'.repeat(65))).toBe(false);
      expect(isValidObservationId('abc.12345')).toBe(false);
      expect(isValidObservationId('abc 12345')).toBe(false);
    });

    test('rejects non-string types', () => {
      expect(isValidObservationId(123 as unknown as string)).toBe(false);
      expect(isValidObservationId(null as unknown as string)).toBe(false);
      expect(isValidObservationId(undefined as unknown as string)).toBe(false);
      expect(isValidObservationId({} as unknown as string)).toBe(false);
    });
  });
});

describe('validateRecoverRequest', () => {
  describe('valid requests', () => {
    test('accepts empty object (defaults will be used)', () => {
      const result = validateRecoverRequest({});
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data).toEqual({});
      }
    });

    test('accepts null body (defaults will be used)', () => {
      const result = validateRecoverRequest(null);
      expect(result.valid).toBe(true);
    });

    test('accepts undefined body', () => {
      const result = validateRecoverRequest(undefined);
      expect(result.valid).toBe(true);
    });

    test.each([
      ['resume' as const],
      ['continue' as const],
      ['new' as const],
    ])('accepts method "%s"', (method) => {
      const result = validateRecoverRequest({ method });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.method).toBe(method);
      }
    });

    test('accepts boolean openTerminal: true', () => {
      const result = validateRecoverRequest({ openTerminal: true });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.openTerminal).toBe(true);
      }
    });

    test('accepts boolean openTerminal: false', () => {
      const result = validateRecoverRequest({ openTerminal: false });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.openTerminal).toBe(false);
      }
    });

    test('accepts boolean skipPermissions: true', () => {
      const result = validateRecoverRequest({ skipPermissions: true });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.skipPermissions).toBe(true);
      }
    });

    test('accepts full valid request with all fields', () => {
      const input = {
        method: 'resume' as const,
        openTerminal: true,
        skipPermissions: false,
      };
      const result = validateRecoverRequest(input);
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data).toEqual(input);
      }
    });
  });

  describe('invalid requests', () => {
    test('rejects invalid method string', () => {
      const result = validateRecoverRequest({ method: 'invalid' });
      expect(result.valid).toBe(false);
      if (!result.valid) {
        expect(result.error).toContain('Invalid method');
        expect(result.error).toContain('resume');
        expect(result.error).toContain('continue');
        expect(result.error).toContain('new');
      }
    });

    test('rejects non-string method', () => {
      const result = validateRecoverRequest({ method: 123 });
      expect(result.valid).toBe(false);
    });

    test('rejects string openTerminal', () => {
      const result = validateRecoverRequest({ openTerminal: 'true' });
      expect(result.valid).toBe(false);
      if (!result.valid) {
        expect(result.error).toContain('openTerminal');
        expect(result.error).toContain('boolean');
      }
    });

    test('rejects number openTerminal', () => {
      const result = validateRecoverRequest({ openTerminal: 1 });
      expect(result.valid).toBe(false);
    });

    test('rejects string skipPermissions', () => {
      const result = validateRecoverRequest({ skipPermissions: 'false' });
      expect(result.valid).toBe(false);
      if (!result.valid) {
        expect(result.error).toContain('skipPermissions');
        expect(result.error).toContain('boolean');
      }
    });

    test('rejects number skipPermissions', () => {
      const result = validateRecoverRequest({ skipPermissions: 0 });
      expect(result.valid).toBe(false);
    });
  });

  describe('edge cases', () => {
    test('ignores extra unknown properties', () => {
      const result = validateRecoverRequest({
        method: 'resume',
        unknownProp: 'value',
        anotherProp: 123,
      });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.method).toBe('resume');
        // Extra props should not be in data
        expect('unknownProp' in result.data).toBe(false);
      }
    });

    test('handles array body', () => {
      const result = validateRecoverRequest([]);
      // Arrays are objects, so should pass initial check but have no valid fields
      expect(result.valid).toBe(true);
    });
  });
});

describe('validateStopRequest', () => {
  describe('valid requests', () => {
    test('accepts empty object', () => {
      const result = validateStopRequest({});
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data).toEqual({});
      }
    });

    test('accepts null body', () => {
      const result = validateStopRequest(null);
      expect(result.valid).toBe(true);
    });

    test('accepts force: true', () => {
      const result = validateStopRequest({ force: true });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.force).toBe(true);
      }
    });

    test('accepts force: false', () => {
      const result = validateStopRequest({ force: false });
      expect(result.valid).toBe(true);
      if (result.valid) {
        expect(result.data.force).toBe(false);
      }
    });
  });

  describe('invalid requests', () => {
    test('rejects string force', () => {
      const result = validateStopRequest({ force: 'true' });
      expect(result.valid).toBe(false);
      if (!result.valid) {
        expect(result.error).toContain('force');
        expect(result.error).toContain('boolean');
      }
    });

    test('rejects number force', () => {
      const result = validateStopRequest({ force: 1 });
      expect(result.valid).toBe(false);
    });

    test('rejects null force', () => {
      const result = validateStopRequest({ force: null });
      expect(result.valid).toBe(false);
    });
  });
});

describe('VALID_RECOVERY_METHODS constant', () => {
  test('contains exactly three methods', () => {
    expect(VALID_RECOVERY_METHODS).toHaveLength(3);
  });

  test('contains resume, continue, and new', () => {
    expect(VALID_RECOVERY_METHODS).toContain('resume');
    expect(VALID_RECOVERY_METHODS).toContain('continue');
    expect(VALID_RECOVERY_METHODS).toContain('new');
  });

  test('is a const array (readonly at type level)', () => {
    // TypeScript enforces readonly via "as const", runtime array is not frozen
    // This test verifies the array structure is correct
    expect(Array.isArray(VALID_RECOVERY_METHODS)).toBe(true);
  });
});
