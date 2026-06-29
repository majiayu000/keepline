import { describe, expect, test } from 'bun:test';
import { parseOverviewOptions } from '../cli/overview.js';

describe('overview CLI options', () => {
  test('parses positive numeric options', () => {
    expect(parseOverviewOptions({
      all: true,
      limit: '5',
      highCostThreshold: '2.5',
      staleHours: '12',
    })).toEqual({
      includeCompleted: true,
      limit: 5,
      highCostThreshold: 2.5,
      staleHours: 12,
    });
  });

  test('rejects invalid numeric options', () => {
    expect(() => parseOverviewOptions({ limit: 'abc' })).toThrow(
      'limit must be a positive number'
    );
    expect(() => parseOverviewOptions({ highCostThreshold: '-1' })).toThrow(
      'high-cost-threshold must be a positive number'
    );
    expect(() => parseOverviewOptions({ staleHours: '0' })).toThrow(
      'stale-hours must be a positive number'
    );
  });
});
