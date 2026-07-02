/**
 * Regression tests for LanceDB filter-DSL injection (GH #74).
 *
 * getById/getBySessionId/delete/deleteBySessionId interpolate their argument
 * into a Datafusion SQL filter literal. Datafusion escapes a single quote by
 * doubling it (backslash is not an escape char), so escapeFilterLiteral must
 * neutralize any embedded quote before interpolation.
 */

import { describe, test, expect } from 'bun:test';
import { escapeFilterLiteral } from '../infrastructure/vector/lancedb.adapter.js';

describe('LanceDB filter injection (GH #74)', () => {
  test('doubles single quotes so the payload cannot break out of the literal', () => {
    const malicious = "x' OR '1'='1";
    const escaped = escapeFilterLiteral(malicious);

    // Every quote is doubled → the whole value stays inside one string literal.
    expect(escaped).toBe("x'' OR ''1''=''1");

    const filter = `id = '${escaped}'`;
    // No lone (odd) quote remains that could terminate the literal early.
    const loneQuotes = filter.replace(/''/g, '').match(/'/g)?.length ?? 0;
    // Only the two delimiter quotes survive after collapsing escaped pairs.
    expect(loneQuotes).toBe(2);
    expect(filter).toBe("id = 'x'' OR ''1''=''1'");
  });

  test('leaves injection-free ids unchanged', () => {
    expect(escapeFilterLiteral('obs-123_ABC')).toBe('obs-123_ABC');
  });

  test('neutralizes a mass-delete payload', () => {
    // `' OR '1'='1` is the classic "delete everything" filter.
    const filter = `id = '${escapeFilterLiteral("' OR '1'='1")}'`;
    expect(filter).toBe("id = ''' OR ''1''=''1'");
    // The tautology `OR '1'='1'` no longer appears as evaluable SQL.
    expect(filter).not.toContain("OR '1'='1'");
  });

  test('handles empty and quote-only inputs', () => {
    expect(escapeFilterLiteral('')).toBe('');
    expect(escapeFilterLiteral("'")).toBe("''");
    expect(escapeFilterLiteral("''")).toBe("''''");
  });
});
