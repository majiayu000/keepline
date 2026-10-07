import { describe,test,expect } from 'bun:test';
import { Hono } from 'hono';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import ledger,{ ledgerSettings } from '../../web/api/routes/ledger.js';
import goals from '../../web/api/routes/goals.js';
import workItems from '../../web/api/routes/work-items.js';
import { setupUser } from '../../services/auth.service.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';

const app = new Hono().route('/ledger',ledger).route('/goals',goals).route('/work-items',workItems).route('/settings',ledgerSettings);
async function request(token: string,path: string,method = 'GET',data?: unknown) {
  return app.request(path,{ method,headers: { Authorization: `Bearer ${token}`,'Content-Type': 'application/json' },...(data === undefined ? {} : { body: JSON.stringify(data) }) });
}
describe('ledger API',() => {
  setupLedgerTest();
  test('authentication and invalid input preserve HTTP error contracts',async () => {
    expect((await app.request('/ledger')).status).toBe(401);
    const { token } = await setupUser('ledger-api','password123');
    expect((await request(token,'/ledger?hours=NaN')).status).toBe(400);
    expect((await request(token,'/ledger/missing')).status).toBe(404);
    expect((await request(token,'/settings','PUT',{ retentionDays: 0 })).status).toBe(400);
    expect((await request(token,'/settings','PUT',{ alerts: { off_plan: false } })).status).toBe(200);
  });
  test('unchanged ledger responses use 304 and changes invalidate the browser cache',async () => {
    const { token } = await setupUser('ledger-conditional','password123');
    const row = await seededLedger();
    for (const path of ['/ledger',`/ledger/${row.sessionId}`]) {
      const first = await request(token,path);
      expect(first.status).toBe(200);
      const tag = first.headers.get('etag')!;
      expect(tag).toBeTruthy();
      expect(first.headers.get('cache-control')).toBe('private, no-cache');
      expect(first.headers.get('vary')).toBe('Authorization');
      const headers = { Authorization: `Bearer ${token}`,'If-None-Match': tag };
      const unchanged = await app.request(path,{ headers });
      expect(unchanged.status).toBe(304);
      expect(await unchanged.text()).toBe('');
      expect((await app.request(path,{ headers: { 'If-None-Match': tag } })).status).toBe(401);
      await request(token,`/ledger/${row.sessionId}/items`,'PUT',{ items: [{ ...row.items[0],title: `Changed ${path}` }] });
      const changed = await app.request(path,{ headers });
      expect(changed.status).toBe(200);
      expect(changed.headers.get('etag')).not.toBe(tag);
    }
    expect((await app.request('/ledger/missing',{ headers: { Authorization: `Bearer ${token}`,'If-None-Match': '*' } })).status).toBe(404);
  });
  test('goal, checklist, attribution, correction and acceptance through existing APIs',async () => {
    const { token } = await setupUser('ledger-api-flow','password123');
    const goalResponse = await request(token,'/work-items','POST',{ title: 'Ship widget',level: 'goal',outcome: 'Widget works' });
    expect(goalResponse.status).toBe(201); const goal = (await goalResponse.json() as { data: { item: { id: string } } }).data.item;
    const todoResponse = await request(token,'/work-items','POST',{ title: 'Widget tests',parentId: goal.id,acceptance: [{ id: 'verify',text: 'Run bun test src/widget.test.ts',completed: false }] });
    expect(todoResponse.status).toBe(201); const todo = (await todoResponse.json() as { data: { item: { id: string } } }).data.item;
    const detail = await seededLedger();
    const linked = await request(token,`/ledger/${detail.sessionId}/attribution`,'POST',{ workItemId: todo.id });
    expect(linked.status).toBe(200); const row = (await linked.json() as { data: import('../../domain/ledger/types.js').LedgerDetail }).data;
    expect(row.items[0].source).toBe('work_item');
    expect((await request(token,`/ledger/${detail.sessionId}/corrections`,'POST',{ callId: row.trail[0].callId,itemId: row.items[0].id,rule: { itemId: row.items[0].id,matcher: { paths: [],commands: ['bun test'] } } })).status).toBe(200);
    expect((await request(token,`/ledger/${detail.sessionId}/items`,'PUT',{ items: [{ ...row.items[0],title: 'Verify widget tests' }] })).status).toBe(200);
    expect((await request(token,`/ledger/${detail.sessionId}/acceptances`,'POST',{ decision: 'accepted' })).status).toBe(200);
    const rolled = (await (await request(token,'/goals')).json() as { data: Array<{ todos: Array<{ readyToComplete: boolean }>; progress: { done: number } }> }).data;
    expect(rolled[0].todos[0].readyToComplete).toBe(true);
    const map = (await (await request(token,`/goals?projectMap=${goal.id}`)).json() as { data: Array<{ recent: unknown[] }> }).data;
    expect(map[0].recent.length).toBeGreaterThan(0);
    expect((await (await request(token,'/goals')).json() as { data: Array<{ recent: unknown[] }> }).data[0].recent).toEqual([]);
    expect((await request(token,`/goals/todos/${todo.id}/complete`,'POST')).status).toBe(200);
    expect((await (await request(token,'/goals')).json() as { data: Array<{ todos: Array<{ readyToComplete: boolean }>; progress: { done: number } }> }).data[0].progress.done).toBe(1);
    expect((await request(token,'/ledger/review')).status).toBe(200);
  });
  test('same call IDs in separate sessions cannot replace another session evidence',async () => {
    const first = await seededLedger(sampleFacts(),'evidence-session-a');
    const second = await seededLedger(sampleFacts(),'evidence-session-b');
    expect(first.evidence[0].id).not.toBe(second.evidence[0].id);
    expect(ledgerRepository.items(first.agentSessionId)[0].evidenceIds).toEqual(first.items[0].evidenceIds);
  });
  test('opening and closing a reply records reading without acceptance',async () => {
    const { token } = await setupUser('ledger-reading','password123');
    const row = await seededLedger();
    expect(row.unread).toBe(true);
    expect((await request(token,`/ledger/${row.sessionId}/viewing`,'POST',{ viewed: true })).status).toBe(200);
    expect((await request(token,`/ledger/${row.sessionId}/viewing`,'POST',{ viewed: false })).status).toBe(200);
    const detail = await (await request(token,`/ledger/${row.sessionId}`)).json() as { data: { state: string; unread: boolean; acceptances: unknown[] } };
    expect(detail.data).toMatchObject({ state: 'ended',unread: false,acceptances: [] });
    expect((await request(token,`/ledger/${row.sessionId}/viewing`,'POST',{ viewed: 'yes' })).status).toBe(400);
  });
});

describe('ledger API re-decomposition',() => {
  setupLedgerTest();
  test('re-decomposition keeps corrected targets and their session rules',async () => {
    const { token } = await setupUser('ledger-redecompose','password123');
    const row = await seededLedger();
    expect((await request(token,`/ledger/${row.sessionId}/corrections`,'POST',{ callId: row.trail[0].callId,itemId: row.items[0].id,rule: { itemId: row.items[0].id,matcher: { paths: ['src/**'],commands: [] } } })).status).toBe(200);
    expect((await request(token,`/ledger/${row.sessionId}/redecompose`,'POST')).status).toBe(200);
    expect(ledgerRepository.corrections(row.agentSessionId)[0].itemId).toBe(row.items[0].id);
    expect(ledgerRepository.rules(row.agentSessionId)[0].itemId).toBe(row.items[0].id);
  });
});
