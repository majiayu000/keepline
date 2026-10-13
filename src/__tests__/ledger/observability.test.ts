import { describe, expect, test, spyOn } from 'bun:test';
import { mkdirSync, writeFileSync } from 'fs';
import { join } from 'path';
import { getKeeplineHome } from '../../lib/paths.js';
import { Hono } from 'hono';
import { setupLedgerTest, sampleFacts, seededLedger } from './helpers.js';
import { getLedger, ledgerDetail, ledgerOverview } from '../../services/ledger/service.js';
import { markLedgerViewed } from '../../services/ledger/alerts.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { rowPresentation, attentionSummary, monitorPresentation } from '../../web/client/src/pages/ledger/presentation.js';
import { clearRuntimeScanStatus, recordRuntimeScanFailures } from '../../services/runtime-status.js';
import { recordHookReceipt } from '../../adapters/hook/observation.js';
import * as availability from '../../adapters/hook/availability.js';
import * as installer from '../../adapters/hook/installer.js';
import status from '../../web/api/routes/status.js';
import { setupUser } from '../../services/auth.service.js';
import type { LedgerMonitorStatus, TranscriptFact } from '../../domain/ledger/types.js';

describe('observability', () => {
  setupLedgerTest();
  test('hook installation requires authentication, reads back status and surfaces write failures', async () => {
    const install = spyOn(installer, 'installHooks').mockImplementation(() => {});
    const health = availability.buildHookAvailability({ installed: true, receiverRunning: true, settingsPath: '', hookCommand: '', hookServerUrl: '' });
    const read = spyOn(availability, 'getHookAvailability').mockResolvedValue(health);
    const app = new Hono().route('/status', status);
    try {
      expect((await app.request('/status/hooks/install', { method: 'POST' })).status).toBe(401);
      expect(install).not.toHaveBeenCalled();
      const { token } = await setupUser('hook-install-api', 'password123');
      const init = { method: 'POST', headers: { Authorization: `Bearer ${token}` } };
      expect(await (await app.request('/status/hooks/install', init)).json()).toEqual({ success: true, data: health });
      expect(install).toHaveBeenCalledTimes(1);
      install.mockImplementation(() => { throw new Error('permission denied'); });
      expect((await app.request('/status/hooks/install', init)).status).toBe(500);
    } finally { install.mockRestore(); read.mockRestore(); }
  });
  test('exit preserves unread completion and prevents stale input requests and alerts', async () => {
    const reply = await seededLedger(sampleFacts(), 'closed-reply');
    sessionRepository.upsert({ sessionId: reply.sessionId, processState: 'exited', pid: undefined });
    expect(await getLedger(reply.sessionId)).toMatchObject({ processState: 'exited', state: 'review', unread: true });
    expect((await ledgerOverview())[0]).toMatchObject({ processState: 'exited', state: 'review', unread: true });
    const at = new Date().toISOString();
    const request = await seededLedger([
      { kind: 'turn', phase: 'started', turnId: 'ask-turn', at },
      { kind: 'tool', callId: 'ask', name: 'AskUserQuestion', input: { questions: [{ question: '继续吗？' }] }, mutating: false, turnId: 'ask-turn', at },
    ], 'closed-question');
    sessionRepository.upsert({ sessionId: request.sessionId, processState: 'exited' });
    expect(await getLedger(request.sessionId)).toMatchObject({ processState: 'exited', state: 'stopped', pendingInput: true });
    sessionRepository.upsert({ sessionId: request.sessionId, processState: 'online' });
    expect(await getLedger(request.sessionId)).toMatchObject({ processState: 'online', state: 'needs_input' });
  });
  test('real context usage reaches overview and details; absent usage stays unknown', async () => {
    const facts = sampleFacts();
    const contextDir = join(getKeeplineHome(), 'context', 'claude-code');
    mkdirSync(contextDir, { recursive: true });
    writeFileSync(join(contextDir, 'context-session.json'), JSON.stringify({ sessionId: 'context-session', at: new Date().toISOString(), contextWindow: {
      context_window_size: 10000, current_usage: { input_tokens: 100, cache_creation_input_tokens: 1000, cache_read_input_tokens: 8000 },
    } }));
    const row = await seededLedger(facts,'context-session');
    expect(row.contextUsage).toMatchObject({ usedTokens: 9100,windowTokens: 10000 });
    expect((await ledgerOverview())[0].contextUsage).toEqual(row.contextUsage);
    expect((await ledgerDetail(row.sessionId))?.contextUsage).toEqual(row.contextUsage);
    expect(rowPresentation(row).context).toMatchObject({ percent: 91,detail: '91% 已用 · 接近上限，长任务可能被截断' });
    expect(rowPresentation(await seededLedger(sampleFacts(),'unknown-context')).context).toBeUndefined();
  });
  test('old unread replies stay in history without becoming outside attention', async () => {
    const at = new Date(Date.now() - 25 * 3600000);
    const row = await seededLedger(sampleFacts().map(f => ({ ...f, at: at.toISOString() })), 'old-reply');
    sessionRepository.upsert({ sessionId: row.sessionId, lastActiveAt: at });
    expect(await getLedger(row.sessionId)).toMatchObject({ state: 'review', unread: true });
    expect(await ledgerOverview(7)).toEqual([]);
    expect(await ledgerOverview(7, true, true)).toEqual([]);
    expect((await ledgerOverview(48)).map(r => r.sessionId)).toEqual(['old-reply']);
    expect(await getLedger(row.sessionId)).toMatchObject({ state: 'review', unread: true });
    sessionRepository.upsert({ sessionId: row.sessionId, status: 'needs_input', statusSource: 'hook' });
    expect((await ledgerOverview(7, true, true)).map(r => r.sessionId)).toEqual(['old-reply']);
    markLedgerViewed(row.sessionId, true, Date.now(), row.turnId);
    expect((await ledgerOverview(7, true, true)).map(r => r.sessionId)).toEqual(['old-reply']);
    sessionRepository.upsert({ sessionId: row.sessionId, status: 'idle', statusSource: 'scan' });
    expect(await ledgerOverview(7, true, true)).toEqual([]);
  });
  test('outside input requests include child tasks under their parent', async () => {
    const parent = await seededLedger(sampleFacts(), 'parent-session');
    const at = new Date(Date.now() - 25 * 3600000);
    const child = await seededLedger([
      { kind: 'turn', phase: 'started', turnId: 'child-turn', at: at.toISOString() },
      { kind: 'tool', callId: 'child-question', name: 'AskUserQuestion', input: { questions: [{ question: '允许执行吗？' }] }, mutating: false, at: at.toISOString(), turnId: 'child-turn' },
    ], 'old-child');
    sessionRepository.upsert({ sessionId: child.sessionId, parentSessionId: parent.sessionId, lastActiveAt: at });
    const outside = await ledgerOverview(7, true, true);
    expect(outside).toHaveLength(1);
    expect(outside[0]).toMatchObject({ sessionId: parent.sessionId, state: 'needs_input', inputRequest: { question: '允许执行吗？' } });
    expect(outside[0].subagents?.map(row => row.sessionId)).toEqual([child.sessionId]);
  });
  test('questions show actual text and provenance; finished tools remain past activity', async () => {
    const at = new Date().toISOString();
    const facts: TranscriptFact[] = [
      { kind: 'turn', phase: 'started', turnId: 'question-turn', at },
      { kind: 'tool', callId: 'question-call', name: 'AskUserQuestion', input: { questions: [{ question: '部署到哪个环境？' }] }, mutating: false, at, turnId: 'question-turn' },
    ];
    const row = await seededLedger(facts);
    expect(row).toMatchObject({ state: 'needs_input', inputRequest: { question: '部署到哪个环境？' }, observation: { source: 'transcript' } });
    expect(rowPresentation(row).now).toBe('部署到哪个环境？');
    const done = await seededLedger();
    expect(rowPresentation(done).now).toStartWith('上次动作：');
    const running = await seededLedger([{ kind: 'turn', phase: 'started', turnId: 'run', at }, { kind: 'tool', callId: 'run', name: 'Bash', input: { command: 'bun test' }, at, turnId: 'run', mutating: true }], 'active-tool');
    expect(rowPresentation(running).now).toContain('尚未观察到结果');
    expect(attentionSummary([row, done])).toBe('1 个等你回答或审批 · 1 条新回复');
  });
  test('changes use the last completed viewing visit, not the current heartbeat', async () => {
    const baseline = Date.now() - 10 * 60000;
    const row = await seededLedger();
    markLedgerViewed(row.sessionId, true, baseline - 1000, row.turnId);
    markLedgerViewed(row.sessionId, false, baseline);
    markLedgerViewed(row.sessionId, true);
    const detail = (await ledgerDetail(row.sessionId))!;
    expect(detail.changes?.since).toBe(new Date(baseline).toISOString());
    expect(detail.changes?.checks.some(e => e.kind === 'test' && e.exitCode === 0)).toBe(true);
    expect(detail.changes?.steps.length).toBeGreaterThan(0);
    const first = await seededLedger(sampleFacts(), 'first-view');
    expect((await ledgerDetail(first.sessionId))?.changes?.since).toBeNull();
  });
  test('health reports missing scans, stale scans and failures without calling them zero work', () => {
    const now = Date.now();
    const health: LedgerMonitorStatus = { scan: { running: false, failed: false, intervalMs: 60000 }, runtimes: [] };
    expect(monitorPresentation(health, now)).toMatchObject({ warning: true, text: '尚未观察到扫描，不能确认待处理数量' });
    health.scan.lastCompletedAt = new Date(now - 5 * 60000).toISOString();
    expect(monitorPresentation(health, now).text).toContain('数据已过期');
    health.scan.failed = true;
    expect(monitorPresentation(health, now).text).toContain('扫描失败');
  });
  test('monitor API distinguishes installed hooks from actual received events', async () => {
    const hook = spyOn(availability, 'getHookAvailability').mockResolvedValue(availability.buildHookAvailability({
      installed: true, receiverRunning: true, settingsPath: '', hookCommand: '', hookServerUrl: '',
      targets: ['claude-code', 'codex'].map(runtimeId => ({ runtimeId: runtimeId as 'claude-code' | 'codex', installed: true, settingsPath: '', trustStatus: 'runtime-managed' })),
    }));
    const app = new Hono().route('/status', status);
    clearRuntimeScanStatus();
    try {
      expect((await app.request('/status/monitor')).status).toBe(401);
      const { token } = await setupUser('monitor-api', 'password123');
      recordRuntimeScanFailures('claude-code', []);
      recordHookReceipt('claude');
      const result = await app.request('/status/monitor', { headers: { Authorization: `Bearer ${token}` } });
      const payload = await result.json() as { data: LedgerMonitorStatus };
      expect(payload.data.runtimes.find(r => r.runtimeId === 'claude-code')).toMatchObject({ runtimeId: 'claude-code', hookInstalled: true, receiverRunning: true });
      expect(payload.data.runtimes.find(r => r.runtimeId === 'claude-code')?.lastHookAt).toBeDefined();
    } finally { hook.mockRestore(); clearRuntimeScanStatus(); }
  });
});
