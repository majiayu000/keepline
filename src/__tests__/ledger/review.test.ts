import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { ledgerReview } from '../../services/ledger/goals.js';
import { carryOver,acceptLedger,replaceLedgerItems,getLedger } from '../../services/ledger/service.js';
describe('daily and weekly review',() => {
  setupLedgerTest();
  test('remaining items carry forward idempotently and accepted work appears in review',async () => {
    const raw = await seededLedger(sampleFacts().filter(f => f.kind !== 'tool'));
    const d = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    const first = await carryOver(d.sessionId); const second = await carryOver(d.sessionId);
    expect(first.id).toBe(second.id); expect(second.acceptance).toHaveLength(1);
    await acceptLedger(d.sessionId,{ decision: 'accepted_with_gaps',droppedItemIds: d.items.map(i => i.id),reason: 'Carry to tomorrow' });
    // The review API uses local calendar days, just like the dashboard picker.
    const review = await ledgerReview(new Date().toLocaleDateString('en-CA')); expect(review.accepted).toHaveLength(1);
    expect(review.accepted[0].asks).toEqual([]);
    expect(review.accepted[0].trail).toEqual([]);
    expect((await getLedger(d.sessionId))!.asks.length).toBeGreaterThan(0);
  });
  test('weekly runtime clips turn duration to period and reports unattributed share',async () => {
    const facts = sampleFacts(); const today = new Date().toISOString().slice(0,10);
    facts[0].at = `${today}T01:00:00.000Z`; facts.at(-1)!.at = `${today}T01:10:00.000Z`;
    await seededLedger(facts);
    const review = await ledgerReview(today,true); expect(review.totalRuntimeMs).toBe(600000); expect(review.unattributedRuntimeShare).toBe(1);
  });
});
