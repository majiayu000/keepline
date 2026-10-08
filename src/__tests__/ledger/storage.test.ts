import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';
import { getDatabase,closeDatabase } from '../../infrastructure/database/sqlite.js';
import { getLedger,replaceLedgerItems,importLedgerRequirements } from '../../services/ledger/service.js';
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

describe('persisted command criteria compatibility',() => {
  setupLedgerTest();
  test('unversioned user commands require confirmation and persist across a database reopen',async () => {
    const detail = await seededLedger();
    const original = { paths: [],commands: ['bun test.*'],keywords: [] };
    getDatabase().query("UPDATE requirement_items SET source='user',status_source='rule',anchors=? WHERE id=?").run(JSON.stringify(original),detail.items[0].id);
    const row = ledgerRepository.items(detail.agentSessionId)[0];
    expect(row.anchors).toEqual({ ...original,commandFormat: 'legacy-unconfirmed',legacyCommands: ['bun test.*'] });
    // An old save client cannot silently replace or promote this pattern.
    ledgerRepository.saveItems(detail.agentSessionId,[{ ...row,anchors: { ...original,commands: ['bun test src/widget.test.ts'] } }]);
    closeDatabase();
    const reopened = ledgerRepository.items(detail.agentSessionId)[0];
    expect(reopened.anchors).toEqual(row.anchors);
    const stored = getDatabase().query('SELECT anchors FROM requirement_items WHERE id=?').get(row.id) as { anchors: string };
    expect(JSON.parse(stored.anchors)).toEqual(row.anchors);
  });
  test('only persisted user rows migrate; new confirmations save literal criteria',async () => {
    const raw = await seededLedger();
    expect(ledgerRepository.items(raw.agentSessionId)[0].anchors.commandFormat).toBeUndefined();
    const confirmed = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    expect(confirmed.items[0].anchors.commandFormat).toBe('literal-v2');
    expect(confirmed.items[0].anchors.legacyCommands).toBeUndefined();
    expect(confirmed.progress).toEqual({ done: 1,total: 1 });
    closeDatabase();
    expect(ledgerRepository.items(raw.agentSessionId)[0].anchors.commandFormat).toBe('literal-v2');
  });
  test('legacy attribution survives while only explicitly confirmed literal receipts complete',async () => {
    const raw = await seededLedger();
    getDatabase().query("UPDATE requirement_items SET source='user',status_source='rule',anchors=? WHERE id=?")
      .run(JSON.stringify({ paths: [],commands: ['bun test.*'],keywords: [] }),raw.items[0].id);
    const pending = (await getLedger(raw.sessionId))!;
    expect(pending.items[0].anchors.commandFormat).toBe('legacy-unconfirmed');
    expect(pending.trail[0].itemId).toBe(raw.items[0].id);
    expect(pending.progress).toEqual({ done: 0,total: 1 });
    expect(pending.items[0].evidenceIds).toEqual([]);
    const oldClient = (await replaceLedgerItems(raw.sessionId,[{ ...pending.items[0],anchors: { paths: [],commands: ['bun test src/widget.test.ts'],keywords: [] } }]))!;
    expect(oldClient.items[0].anchors.commandFormat).toBe('legacy-unconfirmed');
    expect(oldClient.items[0].anchors.commands).toEqual(['bun test.*']);
    const confirmed = (await replaceLedgerItems(raw.sessionId,[{ ...oldClient.items[0],anchors: { ...oldClient.items[0].anchors,commandFormat: 'literal-v2',commands: ['bun test src/widget.test.ts'],legacyCommands: ['forged'] } }]))!;
    expect(confirmed.items[0].anchors.legacyCommands).toEqual(['bun test.*']);
    expect(confirmed.items[0].statusSource).toBe('rule');
    expect(confirmed.progress).toEqual({ done: 1,total: 1 });
    expect(confirmed.items[0].evidenceIds).toHaveLength(2);
    const changed = sampleFacts().map(fact => fact.kind === 'tool'
      ? { ...fact,input: { command: 'bun test src/other.test.ts' },facts: [{ kind: 'command' as const,value: 'bun test src/other.test.ts',exitCode: 0 }] } : fact);
    const rescan = await seededLedger(changed,raw.sessionId);
    expect(rescan.trail[0].itemId).toBe(raw.items[0].id);
    expect(rescan.progress).toEqual({ done: 0,total: 1 });
    expect(rescan.items[0].evidenceIds).toEqual([]);
    expect(rescan.items[0].anchors.commands).toEqual(['bun test src/widget.test.ts']);
  });
  test('imports retain pending or confirmed command semantics and original attribution',async () => {
    const previous = await seededLedger(sampleFacts(),'legacy-import-source');
    getDatabase().query("UPDATE requirement_items SET source='user',status_source='rule',anchors=? WHERE id=?")
      .run(JSON.stringify({ paths: [],commands: ['bun test.*'],keywords: [] }),previous.items[0].id);
    const target = await seededLedger(sampleFacts().filter(fact => fact.kind !== 'user_message'),'legacy-import-target');
    expect(target.importSuggestions?.some(suggestion => suggestion.sessionId === previous.sessionId)).toBe(true);
    const imported = (await importLedgerRequirements(target.sessionId,previous.sessionId))!;
    expect(imported.items[0].id).not.toBe(previous.items[0].id);
    expect(imported.items[0].anchors.commandFormat).toBe('legacy-unconfirmed');
    expect(imported.items[0].anchors.legacyCommands).toEqual(['bun test.*']);
    expect(imported.progress).toEqual({ done: 0,total: 1 });
    const source = (await getLedger(previous.sessionId))!;
    await replaceLedgerItems(previous.sessionId,[{ ...source.items[0],anchors: { ...source.items[0].anchors,commandFormat: 'literal-v2',commands: ['bun test src/widget.test.ts'] } }]);
    const literalImport = (await importLedgerRequirements(target.sessionId,previous.sessionId))!;
    expect(literalImport.items[0].anchors.commandFormat).toBe('literal-v2');
    expect(literalImport.items[0].anchors.legacyCommands).toEqual(['bun test.*']);
    expect(literalImport.progress).toEqual({ done: 1,total: 1 });
    closeDatabase();
    expect(ledgerRepository.items(target.agentSessionId)[0].anchors).toEqual(literalImport.items[0].anchors);
  });

  test('unmarked literal-looking rows still wait for a decision and preserve manual overrides',async () => {
    const raw = await seededLedger();
    getDatabase().query("UPDATE requirement_items SET source='user',status_source='rule' WHERE id=?").run(raw.items[0].id);
    const pending = (await getLedger(raw.sessionId))!;
    expect(pending.items[0].anchors.commandFormat).toBe('legacy-unconfirmed');
    expect(pending.progress.done).toBe(0);
    expect(pending.items[0].evidenceIds).toEqual([]);
    getDatabase().query("UPDATE requirement_items SET status='unverified',status_source='user' WHERE id=?").run(raw.items[0].id);
    const manual = (await getLedger(raw.sessionId))!;
    const confirmed = (await replaceLedgerItems(raw.sessionId,[{ ...manual.items[0],anchors: { ...manual.items[0].anchors,commandFormat: 'literal-v2',commands: ['bun test src/widget.test.ts'] } }]))!;
    expect(confirmed.items[0].status).toBe('unverified');
    expect(confirmed.items[0].statusSource).toBe('user');
    expect(confirmed.progress.done).toBe(0);
  });
});
