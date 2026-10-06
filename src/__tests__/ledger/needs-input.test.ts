import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { startLifecycleReceiver } from '../../adapters/hook/completion-receiver.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { getLedger } from '../../services/ledger/service.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { events } from '../../lib/events.js';
import { markLedgerViewed } from '../../services/ledger/alerts.js';
import { config } from '../../lib/config.js';
describe('hook needs input',() => {
  setupLedgerTest();
  for (const runtime of ['claude-code','codex']) test(`${runtime}: permission request raises once, next tool clears`,async () => {
    const id = runtime === 'codex' ? 'codex_permission-123' : 'permission-123';
    sessionRepository.upsert({ sessionId: id,client: runtime === 'codex' ? 'codex' : 'claude',directory: '/project',lastActiveAt: new Date(),status: 'running' });
    const receiver = startLifecycleReceiver(0);
    const hook = (event: string, extra = {}) => fetch(`http://127.0.0.1:${receiver.port}/hook?runtime=${runtime}`,{ method: 'POST',headers: { 'Content-Type': 'application/json' },body: JSON.stringify({ session_id: 'permission-123',cwd: '/project',hook_event_name: event,...extra }) });
    config.set('ledger',{ ...config.get().ledger,nativeNotifications: true });
    markLedgerViewed('__native__',true);
    const started = performance.now();
    let observedAt = 0;
    const observe = (alert: { sessionId?: string; kind?: string }) => { if (alert.sessionId === id && alert.kind === 'needs_input') observedAt = performance.now(); };
    events.on('ledger:alert',observe);
    try {
      expect((await hook('PermissionRequest',{ tool_name: 'Bash' })).status).toBe(200);
      expect(sessionRepository.findBySessionId(id)).toMatchObject({ status: 'needs_input',statusSource: 'hook',statusReason: 'Bash' });
      await hook('Notification',{ notification_type: 'permission_prompt',message: 'Approve command' });
      expect((await getLedger(id))?.state).toBe('needs_input');
      expect(observedAt).toBeGreaterThanOrEqual(started);
      // Native app polls every five seconds; queued permission alerts must fit the ten-second budget.
      expect(observedAt-started+5000).toBeLessThan(10000);
      expect(getDatabase().query("SELECT notified FROM ledger_alerts WHERE kind='needs_input' AND cleared_at IS NULL").get()).toMatchObject({ notified: 2 });
      expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input' AND cleared_at IS NULL").get()).toMatchObject({ count: 1 });
      await hook('PreToolUse',{ tool_name: 'Bash',tool_input: {} });
      expect(sessionRepository.findBySessionId(id)?.status).toBe('running');
      await getLedger(id);
      expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input' AND cleared_at IS NULL").get()).toMatchObject({ count: 0 });
    } finally { events.off('ledger:alert',observe); receiver.stop(); }
  });
  test('timing-based waiting is display-only',async () => {
    const row = await seededLedger(); sessionRepository.upsert({ sessionId: row.sessionId,status: 'waiting',statusSource: 'scan' });
    const detail = await getLedger(row.sessionId); expect(detail?.possiblyWaiting).toBe(true); expect(detail?.state).not.toBe('needs_input');
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input'").get()).toMatchObject({ count: 0 });
  });
});
