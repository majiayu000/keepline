import { writeCachedLedgerFacts,ledgerFactFingerprint } from '../../infrastructure/session-summary-cache.js';
import { mkdtempSync,writeFileSync,rmSync,statSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { ledgerFactCacheStats } from '../../services/ledger/facts.js';
import { describe,test,expect,spyOn } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { ingestLedger,getLedger,ledgerDetail,replaceLedgerItems,ledgerOverview,acceptLedger } from '../../services/ledger/service.js';
import { evaluateLedgerAlerts } from '../../services/ledger/alerts.js';
import { config } from '../../lib/config.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { DEFAULT_LEDGER_CONFIG } from '../../domain/ledger/types.js';

describe('default activity monitoring',() => {
  setupLedgerTest();
  test('7 and 24 hour windows use last activity rather than the session start', async () => {
    const now = Date.now();
    for (const hours of [2, 8, 26]) {
      const sessionId = `window-${hours}`;
      await seededLedger(sampleFacts().map(f => ({ ...f, at: new Date(now - hours * 3600000).toISOString() })), sessionId);
      sessionRepository.upsert({ sessionId, startedAt: new Date(now - 40 * 3600000), lastActiveAt: new Date(now - hours * 3600000) });
    }
    expect((await ledgerOverview(7)).map(row => row.sessionId)).toEqual(['window-2']);
    expect((await ledgerOverview(24)).map(row => row.sessionId).sort()).toEqual(['window-2', 'window-8']);
  });
  test('raw candidates retain evidence and unread replies without inventing progress or deviations',async () => {
    config.set('ledger',{ ...config.get().ledger,deviation: 'sensitive' });
    const facts = sampleFacts();
    for (let i = 0; i < 9; i++) facts.splice(3,0,{ kind: 'tool',name: 'Write',callId: `unmatched-${i}`,input: { path: `/other/${i}` },at: new Date().toISOString(),turnId: 'turn-1',mutating: true });
    const d = await seededLedger(facts);
    expect(d.items[0].source).toBe('fallback'); expect(d.progress).toEqual({ done: 0,total: 0 });
    expect(d.state).toBe('review'); expect(d.unread).toBe(true); expect(d.offPlan).toEqual([]);
    expect(d.trail.length).toBe(10); expect(d.activity?.evidence.some(e => e.kind === 'test' && e.exitCode === 0)).toBe(true);
    expect(d.activity?.lastMessage).toContain('Completed');
    let calls = 0;
    await evaluateLedgerAlerts(d,{ ...config.get().ledger,nativeNotifications: true },new Date(),async () => { calls++; });
    expect(calls).toBe(0); expect(getDatabase().query("SELECT COUNT(*) AS n FROM ledger_alerts WHERE kind IN ('off_plan','claimed_unverified') AND cleared_at IS NULL").get()).toMatchObject({ n: 0 });
  });
  test('explicit confirmation enables evidence matching and review',async () => {
    const raw = await seededLedger(); const confirmed = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    expect(confirmed.items[0].source).toBe('user'); expect(confirmed.progress).toEqual({ done: 1,total: 1 });
    expect(confirmed.state).toBe('review');
  });
  test('a newly added confirmed criterion follows successful checks automatically',async () => {
    const raw = await seededLedger();const d = (await replaceLedgerItems(raw.sessionId,[{ ...raw.items[0],id: '',title: 'Run bun test src/widget.test.ts',status: 'todo' }]))!;
    expect(d.items[0].statusSource).toBe('rule');expect(d.progress.done).toBe(1);
  });
  test('unknown titles use authored intent and remain usable on later reads',async () => {
    const d = await seededLedger();sessionRepository.upsert({ sessionId: d.sessionId,title: 'Unknown task' });
    const next = (await getLedger(d.sessionId))!;expect(next.title).toContain('Run');expect(next.title).not.toContain('Unknown task');
  });
  test('overdue review becomes ended without creating acceptance or changing completion',async () => {
    const at = new Date(Date.now()-13*3600000).toISOString();
    const raw = await seededLedger(sampleFacts().map(f => ({ ...f,at })));
    const d = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    expect(d.state).toBe('ended'); expect(d.progress.done).toBe(1); expect(d.acceptances).toEqual([]);
    expect((await acceptLedger(d.sessionId,{ decision: 'accepted',droppedItemIds: [] }))?.state).toBe('accepted');
  });
  test('cached review ages out without rereading unchanged transcript facts',async () => {
    const root = mkdtempSync(join(tmpdir(),'ledger-expiry-'));
    const path = join(root,'session.jsonl'),facts = sampleFacts();
    writeFileSync(path,'{}\n');
    const session = sessionRepository.upsert({ sessionId: 'expiry',client: 'claude',directory: '/project',title: 'Review expiry',lastActiveAt: new Date(),status: 'idle' });
    const d = (await ingestLedger(session,{ sessionId: 'expiry',directory: '/project',lastActiveAt: new Date(),messageCount: 1,toolCount: 0,sourcePath: path,transcriptFacts: facts }))!;
    // Keep the actual records in the fact cache when confirming the requirements.
    const fp = ledgerFactFingerprint(statSync(path));
    writeCachedLedgerFacts('claude:'+path,fp.fingerprint,{ fingerprint: fp.fingerprint,facts,unknownRecords: 0 });
    const confirmed = (await replaceLedgerItems(d.sessionId,d.items))!;expect(confirmed.state).toBe('review');
    const reads = ledgerFactCacheStats().reads;const clock = spyOn(Date,'now').mockReturnValue(Date.now()+13*3600000);
    try { expect((await getLedger(d.sessionId))?.state).toBe('ended');expect(ledgerFactCacheStats().reads).toBe(reads); }
    finally { clock.mockRestore();rmSync(root,{ recursive: true,force: true }); }
  });
  test('only permission requests notify by default; other conditions remain in the panel',async () => {
    expect(DEFAULT_LEDGER_CONFIG.alerts).toEqual({ needs_input: true,off_plan: false,claimed_unverified: false,stalled: false,limited: false });
    const raw = await seededLedger(); const d = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    d.state = 'running';d.offPlan = [{ id: 'off',callIds: ['off'],at: new Date().toISOString() }];
    const calls: Array<{ text: string; sound: boolean }> = [];
    const notify = async (text: string,sound = false) => { calls.push({ text,sound }); };
    const cfg = { ...config.get().ledger,nativeNotifications: true };
    await evaluateLedgerAlerts(d,cfg,new Date(),notify); expect(calls).toEqual([]);
    expect(getDatabase().query("SELECT COUNT(*) AS n FROM ledger_alerts WHERE kind='off_plan' AND cleared_at IS NULL").get()).toMatchObject({ n: 1 });
    d.state = 'needs_input';await evaluateLedgerAlerts(d,cfg,new Date(),notify);
    expect(calls).toHaveLength(1); expect(calls[0].sound).toBe(true); expect(calls[0].text).toContain('审批');
  });
  test('children appear under their parent, including nested children and approvals',async () => {
    await seededLedger(sampleFacts(),'codex_parent');
    await seededLedger(sampleFacts(),'codex_child');
    await seededLedger(sampleFacts(),'codex_grandchild');
    sessionRepository.upsert({ sessionId: 'codex_child',client: 'codex',parentSessionId: 'parent' });
    sessionRepository.upsert({ sessionId: 'codex_grandchild',client: 'codex',parentSessionId: 'child',status: 'needs_input',statusSource: 'hook' });
    const rows = await ledgerOverview(); expect(rows).toHaveLength(1);
    expect(rows[0].sessionId).toBe('codex_parent'); expect(rows[0].state).toBe('needs_input');
    expect(rows[0].subagents?.map(s => s.sessionId).sort()).toEqual(['codex_child','codex_grandchild']);
    expect((await getLedger('codex_child'))?.parentSessionId).toBe('codex_parent');
    expect((await ledgerDetail('codex_parent'))?.subagents).toHaveLength(2);
  });
});
