import { test, expect, type Page } from '@playwright/test';
import type { Session, SessionStatus } from '../../src/types';
import { SESSION_STATUSES } from '../../../../domain/session/value-objects';
import { row, settings, now } from './fixtures';

async function mockSessions(page: Page, statuses: readonly SessionStatus[]) {
  await page.clock.install({ time: new Date(now) });
  await page.addInitScript(() => localStorage.setItem('terminal_token', 'ui-test'));
  await page.routeWebSocket('**/ws*', () => {});
  const sessions: Session[] = statuses.map((status) => ({
    id: status, sessionId: `navigation-${status}`, client: 'claude', runtimeId: 'claude-code',
    title: `Navigation ${status}`, directory: '/project/keepline', status,
    initialPrompt: 'Keep this task visible after navigation',
    lastActiveAt: now, createdAt: now, updatedAt: now, toolCount: 0, messageCount: 1,
  }));
  const ledger = { ...structuredClone(row), sessionId: sessions[0].sessionId, state: 'needs_input' };
  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());
    const path = url.pathname;
    let data: unknown = {};
    if (path === '/api/auth/status') data = { setupComplete: true, authenticated: true };
    else if (path === '/api/ledger') data = [ledger];
    else if (path === `/api/ledger/${ledger.sessionId}`) data = ledger;
    else if (path === '/api/ledger/outside-attention') data = [];
    else if (path === '/api/status/monitor') data = {
      scan: { running: false, failed: false, lastCompletedAt: now }, runtimes: [],
    };
    else if (path === '/api/settings/ledger') data = settings;
    else if (path === '/api/goals') data = [];
    else if (path === '/api/work-items') data = { items: [] };
    else if (path === '/api/projects') data = { projects: [], stats: { total: 0, active: 0, idle: 0 } };
    else if (path === '/api/sessions') {
      const query = url.searchParams.get('q') ?? '';
      const filtered = sessions.filter((session) => session.sessionId.includes(query));
      data = {
        sessions: filtered,
        stats: Object.fromEntries([
          ['total', sessions.length],
          ...SESSION_STATUSES.map((status) => [status, sessions.filter((s) => s.status === status).length]),
        ]),
        pagination: { total: filtered.length, limit: 50, offset: 0, hasMore: false },
      };
    } else if (path.endsWith('/full')) data = {
      details: { initialPrompt: 'Keep this task visible after navigation', lastMessage: 'Waiting for approval' },
      tools: { toolCalls: [], total: 0 },
      subAgents: { subAgents: [], count: 0 },
    };
    else if (path.endsWith('/subagents')) data = { subAgents: [], count: 0 };
    await route.fulfill({ json: { success: true, data } });
  });
  return sessions;
}

for (const status of ['needs_input', 'stalled', 'interrupted', 'completed'] as const) {
test(`jumping from overview shows and expands a ${status} session`, async ({ page }, info) => {
  const sessions = await mockSessions(page, [status, 'running']);
  await page.goto('/?view=overview');
  await page.getByRole('button', { name: '查看待响应会话', exact: true }).click();
  await expect(page).toHaveURL(/view=overview/);
  const drawer = page.getByRole('dialog', { name: `会话详情：${row.title}` });
  await expect(drawer).toBeVisible();
  await drawer.getByRole('button', { name: '查看会话记录', exact: true }).click();
  await expect(page).toHaveURL(/view=sessions/);
  const card = page.getByRole('article', { name: sessions[0].title, exact: true });
  await expect(card).toBeVisible();
  await expect(card.getByText('Keep this task visible after navigation')).toBeVisible();
  await expect(card.getByText('Waiting for approval', { exact: true })).toBeVisible();
  if (status === 'needs_input') {
    await expect(page.getByRole('button', { name: 'INPUT (1)', exact: true })).toBeVisible();
  }
  await expect(page.getByRole('article')).toHaveCount(1);
  await page.reload();
  await expect(card).toBeVisible();
  await expect(card.getByText('Waiting for approval', { exact: true })).toBeVisible();
  await expect(page.getByRole('article')).toHaveCount(1);
  await page.screenshot({ path: info.outputPath('session-after-jump.png'), fullPage: true });
});
}

test('every session status remains visible and completed groups still fold', async ({ page }) => {
  const sessions = await mockSessions(page, SESSION_STATUSES);
  await page.goto('/?view=sessions');
  for (const session of sessions.filter((s) => s.status !== 'completed')) {
    await expect(page.getByRole('article', { name: session.title, exact: true })).toBeVisible();
  }
  await expect(page.getByRole('article', { name: 'Navigation completed', exact: true })).toHaveCount(0);
  await page.getByRole('heading', { name: /Completed \(1\)/ }).click();
  await expect(page.getByRole('article')).toHaveCount(SESSION_STATUSES.length);
});
