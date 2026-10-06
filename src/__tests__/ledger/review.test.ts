import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { ledgerReview } from '../../services/ledger/goals.js';
import { carryOver,acceptLedger } from '../../services/ledger/service.js';
describe('daily and weekly review',() => {
  setupLedgerTest();
  test('remaining items carry forward idempotently and accepted work appears in review',async () => {
    const d = await seededLedger(sampleFacts().filter(f => f.kind !== 'tool'));
    const first = await carryOver(d.sessionId); const second = await carryOver(d.sessionId);
    expect(first.id).toBe(second.id); expect(second.acceptance).toHaveLength(1);
    await acceptLedger(d.sessionId,{ decision: 'accepted_with_gaps',droppedItemIds: d.items.map(i => i.id),reason: 'Carry to tomorrow' });
    const review = await ledgerReview(new Date().toISOString().slice(0,10)); expect(review.accepted).toHaveLength(1);
  });
  test('weekly runtime clips turn duration to period and reports unattributed share',async () => {
    const facts = sampleFacts(); const today = new Date().toISOString().slice(0,10);
    facts[0].at = `${today}T01:00:00.000Z`; facts.at(-1)!.at = `${today}T01:10:00.000Z`;
    await seededLedger(facts);
    const review = await ledgerReview(today,true); expect(review.totalRuntimeMs).toBe(600000); expect(review.unattributedRuntimeShare).toBe(1);
  });
});
