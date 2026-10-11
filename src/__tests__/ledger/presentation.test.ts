import { describe, expect, test } from 'bun:test';
import { blankDetail } from './helpers.js';
import { evidenceText, groupRows, reviewAcceptanceAt, rowPresentation } from '../../web/client/src/pages/ledger/presentation.js';
import { ledgerNeedsAttention } from '../../domain/ledger/types.js';

describe('dashboard attention grouping', () => {
  test('overview evidence uses the newest-first activity summary and the detail keeps chronological evidence', () => {
    const old = { id: 'old', callId: 'old', at: '2026-10-07T09:00:00Z', kind: 'test' as const, value: 'old test', exitCode: 0 };
    const latest = { ...old, id: 'new', callId: 'new', at: '2026-10-07T10:00:00Z', value: 'new test', exitCode: 1 };
    const row = { ...blankDetail(), evidence: [old, latest], activity: { action: 'Checking', evidence: [latest, old] } };
    expect(evidenceText(row)).toBe('new test · exit 1');
    expect(rowPresentation(row).evidenceSummary).toContain('本次执行失败');
    expect(evidenceText({ ...row, activity: undefined })).toBe('new test · exit 1');
  });
  test('historical review time belongs to an acceptance inside the selected period', () => {
    const row = { ...blankDetail(), lastActiveAt: '2026-10-09T14:00:00Z', acceptances: [
      { id: 'later', turnId: 't', decision: 'accepted' as const, at: '2026-10-09T13:00:00Z', droppedItemIds: [] },
      { id: 'selected', turnId: 't', decision: 'accepted' as const, at: '2026-10-07T10:30:00Z', droppedItemIds: [] },
      { id: 'follow', turnId: 't', decision: 'follow_up' as const, at: '2026-10-07T11:00:00Z', droppedItemIds: [] },
    ] };
    expect(reviewAcceptanceAt(row, '2026-10-07T00:00:00Z', '2026-10-08T00:00:00Z')).toBe('2026-10-07T10:30:00Z');
    expect(reviewAcceptanceAt(row, '2026-10-06T00:00:00Z', '2026-10-07T00:00:00Z')).toBeUndefined();
  });
  test('goal grouping follows todo parent links and retains unassigned sessions', () => {
    const rows = ['one', 'two', 'free'].map((sessionId, index) => ({ ...blankDetail(), sessionId, workItemId: index < 2 ? `todo-${index}` : undefined }));
    const groups = groupRows(rows, 'goal', Date.now(), { todos: [{ id: 'todo-0', parentId: 'goal-a' }, { id: 'todo-1', parentId: 'goal-b' }], goals: [{ id: 'goal-a', title: 'First outcome' }, { id: 'goal-b', title: 'Second outcome' }] });
    expect(groups.map(group => [group.label, group.rows.map(row => row.sessionId)])).toEqual([['First outcome', ['one']], ['Second outcome', ['two']], ['未关联目标', ['free']]]);
  });
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
