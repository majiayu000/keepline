import { describe, expect, test } from 'bun:test';
import {
  REGISTERED_RUNTIME_IDS,
  type RuntimeId,
} from '../domain/runtime/index.js';
import type { AgentClient } from '../domain/session/index.js';
import {
  clientForRuntimeId,
  isSessionRuntimeId,
  parseRuntimeFilter,
  runtimeIdForClient,
  SESSION_RUNTIME_IDS,
} from '../services/runtime-status.js';

describe('runtime identity status helpers', () => {
  test('session runtime IDs come from registered runtime IDs', () => {
    expect(SESSION_RUNTIME_IDS).toBe(REGISTERED_RUNTIME_IDS);
    expect(SESSION_RUNTIME_IDS).toEqual(['claude-code', 'codex']);
  });

  test('maps known legacy clients to registered runtime IDs', () => {
    expect(runtimeIdForClient('claude')).toBe('claude-code');
    expect(runtimeIdForClient('codex')).toBe('codex');
  });

  test('maps known runtime IDs to legacy clients', () => {
    expect(clientForRuntimeId('claude-code')).toBe('claude');
    expect(clientForRuntimeId('codex')).toBe('codex');
  });

  test('does not silently map unknown runtime IDs to Claude', () => {
    expect(isSessionRuntimeId('cursor')).toBe(false);
    expect(() => clientForRuntimeId('cursor' as RuntimeId)).toThrow('Unsupported runtime id: cursor');
  });

  test('does not silently map unknown clients to Claude Code', () => {
    expect(() => runtimeIdForClient('gemini' as AgentClient)).toThrow('Unsupported agent client: gemini');
  });

  test('runtime filter parser rejects unregistered runtime IDs', () => {
    expect(parseRuntimeFilter('claude-code')).toEqual({ runtimeId: 'claude-code' });
    expect(parseRuntimeFilter('codex')).toEqual({ runtimeId: 'codex' });
    expect(parseRuntimeFilter('cursor')).toEqual({ invalid: 'cursor' });
  });
});
