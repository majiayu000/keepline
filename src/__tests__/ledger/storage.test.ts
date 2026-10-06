import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { getLedger } from '../../services/ledger/service.js';
import { config,mergeLedgerConfig,validateLedgerConfig } from '../../lib/config.js';
describe('ledger storage and retention',() => {
  setupLedgerTest();
  test('derived items and evidence persist, rereads are idempotent',async () => {
    const detail = await seededLedger(); await getLedger(detail.sessionId);
    expect(ledgerRepository.items(detail.agentSessionId)).toHaveLength(1);
    expect(getDatabase().query('SELECT COUNT(*) AS count FROM progress_evidence').get()).toMatchObject({ count: 2 });
  });
  test('retention deletes only old ledger data and ledger evidence',async () => {
    const detail = await seededLedger(); const db = getDatabase();
    db.query("UPDATE agent_sessions SET last_active_at = '2000-01-01T00:00:00.000Z'").run();
    expect(ledgerRepository.clean(30)).toBeGreaterThan(0);
    expect(ledgerRepository.items(detail.agentSessionId)).toHaveLength(0); expect(ledgerRepository.asks(detail.agentSessionId)).toHaveLength(0);
    expect(db.query('SELECT COUNT(*) AS count FROM progress_evidence').get()).toMatchObject({ count: 0 });
  });
  test('config defaults disable judgment and deep merging preserves nested toggles',() => {
    expect(config.get().ledger.judge.enabled).toBe(false);
    const cfg = mergeLedgerConfig({ judge: { model: 'test-model' },alerts: { off_plan: false } });
    expect(cfg.judge.enabled).toBe(false); expect(cfg.judge.model).toBe('test-model'); expect(cfg.alerts.needs_input).toBe(true); expect(cfg.alerts.off_plan).toBe(false);
    expect(validateLedgerConfig(cfg)).toEqual([]);
    expect(validateLedgerConfig({ ...cfg,retentionDays: 0 })).not.toEqual([]);
    expect(validateLedgerConfig({ ...cfg,deviation: 'nope' as any })).not.toEqual([]);
  });
});
