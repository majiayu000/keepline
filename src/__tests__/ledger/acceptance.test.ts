import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { acceptLedger,followUpPrompt,getLedger,replaceLedgerItems,ingestLedger } from '../../services/ledger/service.js';
import { closeDatabase } from '../../infrastructure/database/sqlite.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
describe('acceptance and follow-ups',() => {
  setupLedgerTest();
  test('turn completion means review; only acceptance changes it',async () => {
    const raw = await seededLedger(); expect(raw.state).toBe('review');
    const detail = (await replaceLedgerItems(raw.sessionId,[{ ...raw.items[0],title: 'Run `bun test src/widget.test.ts`' }]))!;
    expect((await acceptLedger(detail.sessionId,{ decision: 'accepted',droppedItemIds: [] }))?.state).toBe('accepted');
    closeDatabase(); expect((await getLedger(detail.sessionId))?.state).toBe('accepted');
    const facts = sampleFacts(); facts.push({ kind: 'turn',phase: 'started',turnId: 'turn-2',at: new Date().toISOString() });
    const session = sessionRepository.findBySessionId(detail.sessionId)!;
    const next = await ingestLedger(session,{ sessionId: session.sessionId,directory: session.directory,lastActiveAt: new Date(),messageCount: 3,toolCount: 1 },facts);
    expect(next?.state).toBe('running');
  });
  test('gaps require a reason; follow-ups are deterministic text',async () => {
    const raw = await seededLedger(sampleFacts().filter(f => f.kind !== 'tool'));
    const detail = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    expect(detail.progress.done).toBe(0);
    expect(acceptLedger(detail.sessionId,{ decision: 'accepted',droppedItemIds: [] })).rejects.toThrow('Remaining');
    const accepted = await acceptLedger(detail.sessionId,{ decision: 'accepted_with_gaps',droppedItemIds: detail.items.map(i => i.id),reason: 'Drop test until tomorrow' });
    expect(accepted?.state).toBe('accepted'); expect(accepted?.items[0].dropped).toBe(true);
    expect(followUpPrompt(detail)).toContain(detail.items[0].title);
  });
  test('item edits, additions and deletions survive future scans',async () => {
    const detail = await seededLedger();
    const edited = { ...detail.items[0],title: 'User edited criterion',status: 'unverified' as const };
    await replaceLedgerItems(detail.sessionId,[edited]); closeDatabase();
    expect((await getLedger(detail.sessionId))?.items[0]).toMatchObject({ title: 'User edited criterion',source: 'user',statusSource: 'user',status: 'unverified' });
    await replaceLedgerItems(detail.sessionId,[]); expect((await getLedger(detail.sessionId))?.items).toHaveLength(0);
  });
  test('a newer failed check blocks plain acceptance after a previous success',async () => {
    const facts = sampleFacts();
    const raw = await seededLedger(facts);
    const confirmed = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    expect(confirmed.progress.done).toBe(1);
    const failed = { ...facts.find(f => f.kind === 'tool')!,callId: 'failed-recheck',exitCode: 1,
      facts: [{ kind: 'command' as const,value: 'bun test src/widget.test.ts',exitCode: 1 },{ kind: 'test' as const,value: '1 fail',exitCode: 1 }] };
    facts.splice(facts.length-2,0,failed);
    const session = sessionRepository.findBySessionId(raw.sessionId)!;
    const updated = await ingestLedger(session,{ sessionId: session.sessionId,directory: session.directory,lastActiveAt: new Date(),messageCount: 2,toolCount: 2 },facts);
    expect(updated?.progress.done).toBe(0);
    await expect(acceptLedger(raw.sessionId,{ decision: 'accepted',droppedItemIds: [] })).rejects.toThrow('Remaining');
  });
});
