import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { startLifecycleReceiver } from '../../adapters/hook/completion-receiver.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { getLedger,ingestLedger,replaceLedgerItems } from '../../services/ledger/service.js';
import { closeDatabase } from '../../infrastructure/database/sqlite.js';
import { TranscriptFacts } from '../../adapters/transcript-facts.js';
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
      const questionTool = runtime === 'codex' ? 'functions.request_user_input' : 'AskUserQuestion';
      await hook('PreToolUse',{ tool_name: questionTool,tool_input: {} });
      expect((await getLedger(id))?.state).toBe('needs_input');
      await hook('PostToolUse',{ tool_name: questionTool,tool_input: {} });
      expect((await getLedger(id))?.state).not.toBe('needs_input');
    } finally { events.off('ledger:alert',observe); receiver.stop(); }
  });
  test('timing-based waiting is display-only',async () => {
    const row = await seededLedger(); sessionRepository.upsert({ sessionId: row.sessionId,status: 'waiting',statusSource: 'scan' });
    const detail = await getLedger(row.sessionId); expect(detail?.possiblyWaiting).toBe(true); expect(detail?.state).not.toBe('needs_input');
    expect(getDatabase().query("SELECT COUNT(*) AS count FROM ledger_alerts WHERE kind='needs_input'").get()).toMatchObject({ count: 0 });
  });

  test('reading an ordinary reply clears attention durably, without accepting work; next reply returns',async () => {
    const row = await seededLedger();
    expect(row).toMatchObject({ state: 'review',unread: true,progress: { done: 0,total: 0 } });
    markLedgerViewed(row.sessionId,true,Date.now(),row.turnId);
    markLedgerViewed(row.sessionId,false);
    closeDatabase();
    const seen = (await getLedger(row.sessionId))!;
    expect(seen).toMatchObject({ state: 'ended',unread: false,acceptances: [] });
    const facts = sampleFacts().concat([
      { kind: 'turn',phase: 'started',turnId: 'turn-2',at: new Date().toISOString() },
      { kind: 'turn',phase: 'completed',turnId: 'turn-2',at: new Date().toISOString() },
    ]);
    const session = sessionRepository.findBySessionId(row.sessionId)!;
    const next = await ingestLedger(session,{ sessionId: row.sessionId,directory: '/project',lastActiveAt: new Date(),messageCount: 3,toolCount: 1 },facts);
    expect(next).toMatchObject({ state: 'review',unread: true });
  });

  test('viewing a confirmed review or explicit approval never resolves it',async () => {
    const raw = await seededLedger();
    const row = (await replaceLedgerItems(raw.sessionId,raw.items))!;
    markLedgerViewed(row.sessionId,true,Date.now(),row.turnId);
    expect((await getLedger(row.sessionId))?.state).toBe('review');
    sessionRepository.upsert({ sessionId: row.sessionId,status: 'needs_input',statusSource: 'hook' });
    expect((await getLedger(row.sessionId))?.state).toBe('needs_input');
  });

  for (const runtime of ['codex','claude'] as const) test(`${runtime}: unanswered structured questions work without hooks and clear on result`,async () => {
    const parser = new TranscriptFacts(runtime),timestamp = new Date().toISOString();
    if (runtime === 'codex') {
      parser.add({ type: 'event_msg',timestamp,payload: { type: 'task_started',turn_id: 'question-turn' } });
      parser.add({ type: 'response_item',timestamp,payload: { type: 'function_call',call_id: 'ask',name: 'functions.request_user_input',arguments: '{}' } });
    } else {
      parser.add({ type: 'user',uuid: 'question-turn',timestamp,message: { content: 'Fix widget' } });
      parser.add({ type: 'assistant',timestamp,message: { content: [{ type: 'tool_use',id: 'ask',name: 'AskUserQuestion',input: {} }] } });
    }
    const row = await seededLedger(parser.facts);
    expect(row).toMatchObject({ state: 'needs_input',pendingInput: true });
    expect((await getLedger(row.sessionId))?.state).toBe('needs_input');
    parser.add(runtime === 'codex'
      ? { type: 'response_item',timestamp,payload: { type: 'function_call_output',call_id: 'ask',output: '{}' } }
      : { type: 'user',timestamp,message: { content: [{ type: 'tool_result',tool_use_id: 'ask',content: 'answer' }] } });
    const next = await seededLedger(parser.facts);
    expect(next).toMatchObject({ state: 'running',pendingInput: false });
  });
});
