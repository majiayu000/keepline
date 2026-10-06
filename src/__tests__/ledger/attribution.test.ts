import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { workItemRepository } from '../../infrastructure/database/repositories/work-item.repository.js';
import { attributeLedger,getLedger,suggestAttribution,importLedgerRequirements } from '../../services/ledger/service.js';
import { sampleFacts } from './helpers.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { closeDatabase } from '../../infrastructure/database/sqlite.js';
describe('attribution',() => {
  setupLedgerTest();
  test('a vague session offers previous requirements; import needs a user action and resets evidence',async () => {
    const previous = await seededLedger(sampleFacts(),'previous-session');
    sessionRepository.upsert({ sessionId: previous.sessionId,lastActiveAt: new Date(Date.now()-60000) });
    const vague = await seededLedger([{ kind: 'user_message',text: 'Fix it',at: new Date().toISOString() }],'vague-session');
    expect(vague.importSuggestions?.map(s => s.sessionId)).toContain(previous.sessionId);
    expect(vague.items[0].title).toBe('Fix it');
    const imported = await importLedgerRequirements(vague.sessionId,previous.sessionId);
    expect(imported?.items.map(i => i.title)).toEqual(previous.items.map(i => i.title));
    expect(imported?.items.every(i => i.status === 'todo' && i.evidenceIds.length === 0)).toBe(true);
    expect(imported?.asks[0].text).toBe('Fix it');
    await expect(importLedgerRequirements(vague.sessionId,'foreign-session')).rejects.toThrow('Choose a suggested previous session');
  });
  test('high-confidence suggestions have reasons, confirmation survives restart and no-goal suppresses suggestions',async () => {
    const todo = workItemRepository.create({ title: 'Widget tests',projectRoot: '/project',acceptance: [{ id: 'check',text: 'bun test src/widget.test.ts',completed: false }] });
    const detail = await seededLedger(); expect(detail.attribution?.[0].reasons).toContain('同一项目');
    expect(detail.workItemId).toBeUndefined(); await attributeLedger(detail.sessionId,todo.id); closeDatabase();
    expect((await getLedger(detail.sessionId))?.workItemId).toBe(todo.id);
    await attributeLedger(detail.sessionId,null); closeDatabase(); expect((await getLedger(detail.sessionId))?.attribution).toHaveLength(0);
  });
  test('low-confidence sessions stay unattributed',async () => {
    workItemRepository.create({ title: 'Unrelated todo',projectRoot: '/other' });
    const detail = await seededLedger(); const session = sessionRepository.findBySessionId(detail.sessionId)!;
    expect(suggestAttribution(session,'a vague request')).toEqual([]);
  });
});
