/**
 * Session ID validation shared by scanners, persistence, and recovery sinks.
 */

const SESSION_ID_PATTERN = /^[a-zA-Z0-9-_]{8,64}$/;

/** Validate session ID format used by Claude Hub recovery APIs. */
export function isValidSessionId(id: unknown): id is string {
  return typeof id === 'string' && SESSION_ID_PATTERN.test(id);
}

/** Throw when a value cannot safely be used as a Claude session ID. */
export function assertValidSessionId(id: unknown): asserts id is string {
  if (!isValidSessionId(id)) {
    throw new Error('Invalid session ID format');
  }
}
