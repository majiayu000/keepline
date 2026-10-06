import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { evaluateLedgerAlerts,flushFocusSummary,markLedgerViewed } from '../../services/ledger/alerts.js';
import { config } from '../../lib/config.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
describe('ledger alerts',() => {
  setupLedgerTest();
  test('one continuing deviation produces one notification across repeated scans',async () => {
    const detail = await seededLedger(); detail.state = 'running'; detail.offPlan = [{ id: 'run',callIds: ['a','b','c'],at: new Date().toISOString() }];
    const cfg = { ...config.get().ledger,nativeNotifications: true }; let deliveries = 0;
    const notify = async () => { deliveries++; };
    for (const seconds of [0,1,10,600,1200]) await evaluateLedgerAlerts(detail,cfg,new Date(Date.now()+seconds*1000),notify);
    expect(deliveries).toBe(1);
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='off_plan'").get()).toMatchObject({ count: 1 });
    detail.offPlan = []; await evaluateLedgerAlerts(detail,cfg,new Date(Date.now()+1300000),notify);
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='off_plan' AND cleared_at IS NULL").get()).toMatchObject({ count: 0 });
  });
  test('coalesces and withdraws conditions; per-kind toggles change behavior',async () => {
    const d = await seededLedger(); d.state = 'needs_input'; const cfg = config.get().ledger;
    await evaluateLedgerAlerts(d,cfg); await evaluateLedgerAlerts(d,cfg);
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input' AND cleared_at IS NULL").get()).toMatchObject({ count: 1 });
    d.state = 'running'; await evaluateLedgerAlerts(d,cfg);
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input' AND cleared_at IS NOT NULL").get()).toMatchObject({ count: 1 });
    d.state = 'needs_input'; await evaluateLedgerAlerts(d,{ ...cfg,alerts: { ...cfg.alerts,needs_input: false } });
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input' AND cleared_at IS NULL").get()).toMatchObject({ count: 0 });
  });
  test('focus lets urgent alerts through, queues others, then emits one summary',async () => {
    const d = await seededLedger(); const at = new Date(); let calls: Array<{ body: string; sound: boolean }> = [];
    const notify = async (body: string,sound = false) => { calls.push({ body,sound }); };
    const cfg = { ...config.get().ledger,nativeNotifications: true,focus: { minutes: 30,until: new Date(at.getTime()+60000).toISOString() } };
    d.state = 'needs_input'; d.offPlan = [{ id: 'off',callIds: ['call-1'],at: at.toISOString() }];
    await evaluateLedgerAlerts(d,cfg,at,notify); expect(calls).toHaveLength(1); expect(calls[0].sound).toBe(true);
    await flushFocusSummary(cfg,new Date(at.getTime()+61000),notify); expect(calls).toHaveLength(2); expect(calls[1].body).toContain('during focus'); expect(calls[1].sound).toBe(false);
  });
  test('viewed sessions and disabled native channel never notify',async () => {
    const d = await seededLedger(); d.state = 'needs_input'; let calls = 0; const notify = async () => { calls++; };
    markLedgerViewed(d.sessionId,true);
    await evaluateLedgerAlerts(d,{ ...config.get().ledger,nativeNotifications: true },new Date(),notify); expect(calls).toBe(0);
  });
  test('app channel queues delivery rather than launching osascript',async () => {
    const d = await seededLedger(); d.state = 'needs_input'; let calls = 0;
    markLedgerViewed('__native__',true);
    await evaluateLedgerAlerts(d,{ ...config.get().ledger,nativeNotifications: true },new Date(),async () => { calls++; });
    expect(calls).toBe(0); expect(getDatabase().query("SELECT notified FROM ledger_alerts WHERE kind='needs_input'").get()).toMatchObject({ notified: 2 });
  });
});
