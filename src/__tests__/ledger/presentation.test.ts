import { describe, expect, test } from 'bun:test';
import { blankDetail } from './helpers.js';
import { groupRows, rowPresentation } from '../../web/client/src/pages/ledger/presentation.js';
import { ledgerNeedsAttention } from '../../domain/ledger/types.js';

describe('dashboard attention grouping', () => {
  test('recent activity is distinct from the total span of an old session', () => {
    const now = Date.parse('2026-10-07T12:00:00Z');
    const row = { ...blankDetail(), lastActiveAt: new Date(now - 2 * 3600000).toISOString(), asks: [{ id: 'ask', text: 'Continue work', authoredText: 'Continue work', kind: 'initial' as const, at: new Date(now - 40 * 3600000).toISOString() }] };
    expect(rowPresentation(row, now)).toMatchObject({ elapsed: '40 小时', activeAgo: '2 小时前', lastAgo: '2 小时前' });
    expect(rowPresentation(row, now).activityTitle).toStartWith('最后活动：');
    expect(groupRows([{ ...row, state: 'ended' }], 'urgency', now).find(group => group.key === 'ended')?.label).toBe('已结束');
  });
  test('a complete review still needs acceptance and never appears as running', () => {
    const row = { ...blankDetail(), state: 'review' as const, progress: { done: 1, total: 1 } };
    expect(rowPresentation(row)).toMatchObject({ need: true, action: '验收 (A)' });
    const groups = groupRows([row], 'urgency');
    expect(groups.find(group => group.key === 'need')?.rows).toEqual([row]);
    expect(groups.find(group => group.key === 'running')?.rows).toEqual([]);
    expect(groupRows([row], 'project')[0].hint).toBe('1 需要你');
  });

  test('reviews with gaps retain follow-up and all states agree with the backend', () => {
    const row = { ...blankDetail(), state: 'review' as const, progress: { done: 0, total: 1 } };
    expect(rowPresentation(row)).toMatchObject({ need: true, action: '生成追问' });
    for (const state of ['running', 'needs_input', 'review', 'accepted', 'stopped', 'ended'] as const) {
      const candidate = { ...row, state };
      expect(rowPresentation(candidate).need).toBe(ledgerNeedsAttention(candidate));
    }
    const offPlan = { ...row, state: 'running' as const, offPlan: [{ id: 'off', callIds: ['call'], at: row.lastActiveAt }] };
    expect(rowPresentation(offPlan).need).toBe(true);
  });

  test('a reply without a checklist offers reading instead of invalid acceptance', () => {
    const row = { ...blankDetail(), state: 'review' as const, unread: true };
    expect(rowPresentation(row)).toMatchObject({ need: true, tag: '◐ 新回复', action: '查看回复' });
  });
  test('explicit input requests precede older unread replies', () => {
    const reply = { ...blankDetail(), sessionId: 'reply', state: 'review' as const, lastActiveAt: new Date(0).toISOString() };
    const blocked = { ...blankDetail(), sessionId: 'blocked', state: 'needs_input' as const };
    expect(groupRows([reply,blocked], 'urgency')[0].rows.map(row => row.sessionId)).toEqual(['blocked','reply']);
  });
});
