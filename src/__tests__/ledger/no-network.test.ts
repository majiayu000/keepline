import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { judgeLedger } from '../../services/ledger/judge.js';
import { getLedger,followUpPrompt } from '../../services/ledger/service.js';
describe('zero model calls by default',() => {
  setupLedgerTest();
  test('disabled judge, asks, follow-up and ledger rebuild perform no provider request',async () => {
    const original = globalThis.fetch, spawn = Bun.spawn, spawnSync = Bun.spawnSync; let requests = 0,backendCalls = 0,processes = 0;
    globalThis.fetch = (() => { requests++; throw new Error('Network must remain off'); }) as unknown as typeof fetch;
    Bun.spawn = (() => { processes++; throw new Error('No model subprocess allowed'); }) as typeof Bun.spawn;
    Bun.spawnSync = (() => { processes++; throw new Error('No model subprocess allowed'); }) as typeof Bun.spawnSync;
    try {
      const detail = await seededLedger();
      await judgeLedger(detail.agentSessionId,detail.asks,async () => { backendCalls++; return {}; });
      followUpPrompt(detail); await getLedger(detail.sessionId);
      expect(requests).toBe(0); expect(backendCalls).toBe(0); expect(processes).toBe(0);
    } finally { globalThis.fetch = original; Bun.spawn = spawn; Bun.spawnSync = spawnSync; }
  });
});
