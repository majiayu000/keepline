import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { correctLedger,getLedger } from '../../services/ledger/service.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';
import { matchLedger } from '../../domain/ledger/matcher.js';
import { DEFAULT_LEDGER_CONFIG } from '../../domain/ledger/types.js';
describe('durable corrections and rules',() => {
  setupLedgerTest();
  test('corrected steps and a session rule win over anchors',async () => {
    const detail = await seededLedger(); const itemId = detail.items[0].id;
    await correctLedger(detail.sessionId,['call-1'],itemId,false,{ itemId,matcher: { paths: ['Cargo.*'],commands: [] } });
    const facts = sampleFacts(); facts.push({ kind: 'tool',name: 'Edit',callId: 'cargo-edit',input: { path: 'Cargo.toml' },at: new Date().toISOString(),mutating: true });
    const result = matchLedger(facts,detail.items,ledgerRepository.corrections(detail.agentSessionId),ledgerRepository.rules(detail.agentSessionId),DEFAULT_LEDGER_CONFIG);
    expect(result.trail.at(-1)?.itemId).toBe(itemId);
    expect((await getLedger(detail.sessionId))?.trail[0].itemId).toBe(itemId);
  });
  test('foreign item and step identifiers are rejected',async () => {
    const detail = await seededLedger();
    expect(correctLedger(detail.sessionId,['foreign'],'foreign')).rejects.toThrow('Unknown step');
    expect(correctLedger(detail.sessionId,['call-1'],'foreign')).rejects.toThrow('Invalid correction');
  });
});
