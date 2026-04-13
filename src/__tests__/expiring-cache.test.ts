import { describe, expect, test } from 'bun:test';
import { ExpiringCache } from '../web/api/expiring-cache.js';

describe('ExpiringCache', () => {
  test('returns cached values before expiry', () => {
    const cache = new ExpiringCache<string>();
    cache.set('quota', 'cached-value', 1_000, 10_000);

    expect(cache.get('quota', 10_500)).toBe('cached-value');
  });

  test('expires entries after ttl elapses', () => {
    const cache = new ExpiringCache<string>();
    cache.set('usage', 'cached-value', 1_000, 10_000);

    expect(cache.get('usage', 11_000)).toBeNull();
    expect(cache.get('usage', 11_001)).toBeNull();
  });

  test('clear removes all cached entries', () => {
    const cache = new ExpiringCache<string>();
    cache.set('a', 'one', 1_000, 0);
    cache.set('b', 'two', 1_000, 0);

    cache.clear();

    expect(cache.get('a', 1)).toBeNull();
    expect(cache.get('b', 1)).toBeNull();
  });
});
