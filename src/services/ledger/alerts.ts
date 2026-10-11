import { basename } from 'path';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { randomUUID } from 'crypto';
import { config } from '../../lib/config.js';
import { emit } from '../../lib/events.js';
import { logger } from '../../lib/logger.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { macOSNotification } from '../../infrastructure/notify/macos.js';
import type { LedgerDetail, LedgerConfig } from '../../domain/ledger/types.js';

type AlertDetail = Pick<LedgerDetail,'state' | 'statusReason' | 'offPlan' | 'claims' | 'progress' | 'limited' | 'agentSessionId' | 'sessionId' | 'title' | 'projectRoot' | 'parentSessionId' | 'turnId'>;
type AlertKind = keyof LedgerConfig['alerts'];
export interface LedgerAlert { id: string; agent_session_id: string; kind: AlertKind; detail: string; raised_at: string; cleared_at: string | null; notified: number }
export function markLedgerViewed(sessionId: string, viewed: boolean, now = Date.now(), turnId?: string) {
  const db = getDatabase();
  if (viewed) db.query(`INSERT INTO ledger_views(session_id,expires_at,last_viewed_turn_id) VALUES(?,?,?) ON CONFLICT(session_id) DO UPDATE SET
    expires_at=excluded.expires_at,last_viewed_turn_id=COALESCE(excluded.last_viewed_turn_id,ledger_views.last_viewed_turn_id)`).run(sessionId,new Date(now+45000).toISOString(),turnId ?? null);
  else db.query('UPDATE ledger_views SET expires_at=? WHERE session_id=?').run(new Date(now).toISOString(),sessionId);
}
export function ledgerAlertConditions(d: AlertDetail): Partial<Record<AlertKind,string>> {
  const conditions: Partial<Record<AlertKind,string>> = {};
  if (d.state === 'needs_input') conditions.needs_input = '正在等待你审批或补充信息';
  if (!d.parentSessionId && d.progress.total > 0 && ['running','needs_input'].includes(d.state) && d.offPlan.length) conditions.off_plan = `发现 ${d.offPlan.length} 段可能偏离要求的操作`;
  if (!d.parentSessionId && d.progress.total > 0 && d.state === 'review' && d.claims.some(c => c.turnId === d.turnId) && d.progress.done < d.progress.total) conditions.claimed_unverified = '已确认要求的完成说法还需要证据';
  if (!d.parentSessionId && d.state === 'stopped') conditions.stalled = '会话已停下，可查看执行记录';
  if (!d.parentSessionId && d.limited) conditions.limited = d.statusReason?.startsWith('Codex 目标') ? d.statusReason : '会话额度或预算已用尽';
  return conditions;
}
export async function evaluateLedgerAlerts(d: AlertDetail, cfg = config.get().ledger, now = new Date(), notify = macOSNotification) {
  const db = getDatabase(); const at = now.toISOString(); const nowMs = now.getTime();
  const conditions = cfg.enabled ? ledgerAlertConditions(d) : {};
  const alerts = db.query('SELECT * FROM ledger_alerts WHERE agent_session_id = ? ORDER BY raised_at DESC').all(d.agentSessionId) as LedgerAlert[];
  for (const alert of alerts) if (!alert.cleared_at && !conditions[alert.kind]) {
    db.query('UPDATE ledger_alerts SET cleared_at = ? WHERE id = ?').run(at,alert.id);
    emit('ledger:alert-cleared',{ id: alert.id, sessionId: d.sessionId });
  }
  const raised: LedgerAlert[] = [];
  for (const [kind, message] of Object.entries(conditions) as Array<[AlertKind,string]>) {
    const last = alerts.find(a => a.kind === kind);
    if (last && !last.cleared_at) continue;
    // Coalesce recurrence, but reopen it so withdrawal remains correct.
    if (last && nowMs - Date.parse(last.raised_at) < cfg.alertCoalesceSeconds * 1000) {
      db.query('UPDATE ledger_alerts SET cleared_at = NULL, detail = ? WHERE id = ?').run(message,last.id);
      emit('ledger:alert',{ ...last, detail: message, cleared_at: null, sessionId: d.sessionId }); continue;
    }
    const alert: LedgerAlert = { id: randomUUID(), agent_session_id: d.agentSessionId, kind, detail: message, raised_at: at, cleared_at: null, notified: 0 };
    db.query('INSERT INTO ledger_alerts VALUES(?,?,?,?,?,?,?)').run(alert.id,alert.agent_session_id,kind,message,at,null,0);
    emit('ledger:alert',{ ...alert, sessionId: d.sessionId }); raised.push(alert);
  }
  const focused = cfg.focus.until && Date.parse(cfg.focus.until) > nowMs;
  const viewed = !!db.query('SELECT 1 FROM ledger_views WHERE session_id = ? AND expires_at > ?').get(d.sessionId,at);
  const nativeChannel = !!db.query('SELECT 1 FROM ledger_views WHERE session_id = ? AND expires_at > ?').get('__native__',at);
  if (!nativeChannel) db.query('UPDATE ledger_alerts SET notified=0 WHERE notified=2 AND cleared_at IS NULL').run();
  const pending = db.query('SELECT * FROM ledger_alerts WHERE agent_session_id = ? AND cleared_at IS NULL AND notified IN (0,2)').all(d.agentSessionId) as LedgerAlert[];
  if (viewed || !cfg.nativeNotifications) {
    for (const alert of pending) db.query('UPDATE ledger_alerts SET notified = 1 WHERE id = ?').run(alert.id);
    return raised;
  }
  for (const alert of pending) {
    if (!cfg.alerts[alert.kind]) { db.query('UPDATE ledger_alerts SET notified = 1 WHERE id = ?').run(alert.id); continue; }
    if (focused && alert.kind !== 'needs_input' || alert.notified === 2) continue;
    // Merge simultaneous permission requests into one notification in a short time window.
    const simultaneous = alert.kind === 'needs_input' && db.query("SELECT 1 FROM ledger_alerts WHERE kind='needs_input' AND notified=1 AND raised_at >= ? AND id <> ?").get(new Date(nowMs - 1000).toISOString(),alert.id);
    try {
      if (nativeChannel) { db.query('UPDATE ledger_alerts SET notified = 2 WHERE id = ?').run(alert.id); continue; }
      if (!simultaneous) await notify(`${ledgerNotificationTitle(d.sessionId,d.title,d.projectRoot,d.parentSessionId)}：${alert.detail}`,alert.kind === 'needs_input');
      db.query('UPDATE ledger_alerts SET notified = 1 WHERE id = ?').run(alert.id);
    } catch (error) { logger.warn('Ledger notification failed', { error: error instanceof Error ? error.message : String(error) }); }
  }
  return raised;
}
export async function flushFocusSummary(cfg = config.get().ledger, now = new Date(), notify = macOSNotification) {
  if (!cfg.enabled || !cfg.focus.until || Date.parse(cfg.focus.until) > now.getTime()) return;
  const db = getDatabase();
  const pending = db.query("SELECT * FROM ledger_alerts WHERE notified = 0 AND cleared_at IS NULL AND kind <> 'needs_input'").all() as LedgerAlert[];
  const enabled = pending.filter(alert => cfg.alerts[alert.kind]);
  for (const alert of pending.filter(alert => !cfg.alerts[alert.kind])) db.query('UPDATE ledger_alerts SET notified=1 WHERE id=?').run(alert.id);
  if (enabled.length && cfg.nativeNotifications) {
    const nativeChannel = !!db.query('SELECT 1 FROM ledger_views WHERE session_id = ? AND expires_at > ?').get('__native__',now.toISOString());
    if (nativeChannel) {
      for (const alert of enabled) db.query('UPDATE ledger_alerts SET notified = 2 WHERE id = ?').run(alert.id);
      config.set('ledger',{ ...cfg,focus: { ...cfg.focus,until: null } }); return;
    }
    await notify(`${enabled.length} 条专注期间的提醒：${enabled.map(a => a.detail).join('; ').slice(0,600)}`,false);
    for (const alert of enabled) db.query('UPDATE ledger_alerts SET notified = 1 WHERE id = ?').run(alert.id);
  }
  config.set('ledger',{ ...cfg, focus: { ...cfg.focus, until: null } });
}

export function ledgerNotificationTitle(sessionId: string,title: string,projectRoot: string,parentSessionId?: string): string {
  const session = sessionRepository.findBySessionId(sessionId);
  let parentId = parentSessionId ?? session?.parentSessionId;
  let name = title, directory = projectRoot;
  const seen = new Set([sessionId]);
  while (parentId && !seen.has(parentId)) {
    seen.add(parentId);
    const parent = sessionRepository.findBySessionId(parentId) ?? sessionRepository.findBySessionId(`codex_${parentId}`);
    if (!parent) break;
    const persisted = getDatabase().query('SELECT title FROM agent_sessions WHERE runtime_session_id=?').get(parent.sessionId) as { title: string } | null;
    name = parent.title === 'Unknown task' ? persisted?.title ?? parent.title : parent.title;
    directory = parent.directory;parentId = parent.parentSessionId;
  }
  return name && name !== 'Unknown task' ? name : `未命名会话 · ${basename(directory)}`;

}
