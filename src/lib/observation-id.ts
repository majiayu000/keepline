/**
 * Observation ID validation shared by memory routes and vector stores.
 */

const OBSERVATION_ID_PATTERN = /^[a-zA-Z0-9-_]{8,64}$/;

/** Validate observation ID format before it is used in vector-store predicates. */
export function isValidObservationId(id: unknown): id is string {
  return typeof id === 'string' && OBSERVATION_ID_PATTERN.test(id);
}

/** Throw when a value cannot safely be used as an observation ID. */
export function assertValidObservationId(id: unknown): asserts id is string {
  if (!isValidObservationId(id)) {
    throw new Error('Invalid observation ID format');
  }
}
