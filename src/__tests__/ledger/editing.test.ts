import { expect, test } from 'bun:test';
import { clampOverviewHours, cleanRequirementAnchors, reconcileChecklist } from '../../web/client/src/pages/ledger/editing.js';
import { workspaceLocation } from '../../web/client/src/pages/ledger/navigation.js';

test('inserting, moving and editing checklist text never transfers existing completion evidence', () => {
  const original = [{ id: 'a', text: 'Test API', completed: true }, { id: 'b', text: 'Test UI', completed: false }];
  const result = reconcileChecklist('New security check\nTest UI\nTest API', original);
  expect(result.slice(1)).toEqual([original[1], original[0]]);
  expect(result[0].completed).toBe(false);
  expect(original.map(item => item.id)).not.toContain(result[0].id);
  const changed = reconcileChecklist('Test changed API', original);
  expect(changed[0].id).not.toBe('a');
  expect(changed[0].completed).toBe(false);
  expect(original).toHaveLength(2);
});

test('duplicate checklist text consumes original identities once and new copies start unverified', () => {
  const result = reconcileChecklist('Same\nSame\nSame', [{ id: 'a', text: 'Same', completed: true }, { id: 'b', text: 'Same', completed: false }]);
  expect(result.map(item => item.completed)).toEqual([true, false, false]);
  expect(new Set(result.map(item => item.id)).size).toBe(3);
  expect(result.slice(0, 2).map(item => item.id)).toEqual(['a', 'b']);
});

test('requirement editor keeps command format and server-owned legacy history as metadata', () => {
  expect(cleanRequirementAnchors({ paths: [' src/** ', ''], commands: [' bun test '], keywords: [], commandFormat: 'legacy-unconfirmed', legacyCommands: ['bun test.*'] }))
    .toEqual({ paths: ['src/**'], commands: ['bun test'], keywords: [], commandFormat: 'legacy-unconfirmed', legacyCommands: ['bun test.*'] });
});

test('overview hours respect the loaded retention limit after startup and settings changes', () => {
  expect(clampOverviewHours(720, 7)).toBe(168);
  expect(clampOverviewHours(168, 1)).toBe(24);
  expect(clampOverviewHours(7, 7)).toBe(7);
  expect(clampOverviewHours(8760, 365)).toBe(8760);
  expect(clampOverviewHours(Number.NaN, 7)).toBe(24);
});

test('leaving the ledger clears its panel URL while preserving unrelated navigation parameters', () => {
  const original = 'http://localhost/?view=overview&sessionId=old&anchor=item-1&detail=full&token=example';
  const dashboard = workspaceLocation(original, 'sessions');
  expect(dashboard.searchParams.get('view')).toBe('sessions');
  expect(dashboard.searchParams.get('token')).toBe('example');
  for (const key of ['sessionId', 'anchor', 'detail']) expect(dashboard.searchParams.has(key)).toBe(false);
  const back = workspaceLocation(dashboard.href, 'overview');
  expect(back.searchParams.has('sessionId')).toBe(false);
  expect(workspaceLocation(original, 'todos').searchParams.get('sessionId')).toBe('old');
});
