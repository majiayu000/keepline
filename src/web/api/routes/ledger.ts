import { Hono, type Context } from 'hono';
import { createHash } from 'node:crypto';
import { authMiddleware } from '../middleware/auth.js';
import { readJsonObject } from '../../../local-api/http.js';
import { config, mergeLedgerConfig, validateLedgerConfig } from '../../../lib/config.js';
import { logger } from '../../../lib/logger.js';
import { getLedger, ledgerDetail, ledgerOverview, acceptLedger, attributeLedger, carryOver, correctLedger, followUpPrompt, importLedgerRequirements, replaceLedgerItems, LedgerInputError } from '../../../services/ledger/service.js';
import { ledgerReview } from '../../../services/ledger/goals.js';
import { markLedgerViewed,ledgerNotificationTitle } from '../../../services/ledger/alerts.js';
import type { RequirementItem, LedgerRule, LedgerAcceptance } from '../../../domain/ledger/types.js';
import { getDatabase } from '../../../infrastructure/database/sqlite.js';
import { emit } from '../../../lib/events.js';

const app = new Hono(); app.use('*',authMiddleware);
function conditionalJson(c: Context, data: unknown) {
  const body = JSON.stringify({ success: true,data });
  const tag = `"${createHash('sha256').update(body).digest('hex')}"`;
  c.header('ETag',tag);
  c.header('Cache-Control','private, no-cache');
  c.header('Vary','Authorization');
  if (c.req.header('If-None-Match')?.split(/,\s*/).some(value => value === '*' || value.replace(/^W\//,'') === tag)) return c.body(null,304);
  c.header('Content-Type','application/json; charset=UTF-8');
  return c.body(body);
}
app.onError((error,c) => {
  if (error instanceof LedgerInputError) return c.json({ success: false,error: error.message },400);
  logger.error('Ledger request failed',error); return c.json({ success: false,error: 'Ledger request failed' },500);
});
app.get('/',async c => {
  const hours = Number(c.req.query('hours') ?? 24);
  if (!Number.isFinite(hours) || hours < 1 || hours > config.get().ledger.retentionDays * 24) throw new LedgerInputError('hours outside retention window');
  return conditionalJson(c,await ledgerOverview(hours));
});
app.post('/native-channel',async c => {
  markLedgerViewed('__native__',true); return c.json({ success: true });
});
app.get('/active-alerts',c => c.json({ success: true,data: getDatabase().query('SELECT id,kind FROM ledger_alerts WHERE cleared_at IS NULL').all().filter(row => config.get().ledger.alerts[(row as { kind: 'needs_input' | 'off_plan' | 'claimed_unverified' | 'stalled' | 'limited' }).kind]) }));
app.get('/notifications',c => {
  const data = getDatabase().query(`SELECT l.*,a.runtime_session_id AS sessionId,a.title,a.project_root AS projectRoot FROM ledger_alerts l JOIN agent_sessions a ON a.id=l.agent_session_id WHERE l.notified=2 AND l.cleared_at IS NULL`).all();
  // One native delivery for concurrent permission requests or a focus backlog.
  const alerts = (data as Array<Record<string,unknown>>).filter(a => config.get().ledger.alerts[a.kind as 'needs_input' | 'off_plan' | 'claimed_unverified' | 'stalled' | 'limited']);
  const grouped = [alerts.filter(a => a.kind === 'needs_input'),alerts.filter(a => a.kind !== 'needs_input')].filter(a => a.length).map(rows => ({
    ...rows[0], bundledIds: rows.map(a => a.id),
    detail: rows.length === 1 ? `${ledgerNotificationTitle(String(rows[0].sessionId),String(rows[0].title),String(rows[0].projectRoot))}：${rows[0].detail}` : `${rows.length} 条提醒：${rows.map(a => a.detail).join('；').slice(0,600)}`,
  }));
  return c.json({ success: true,data: grouped });
});
app.post('/notifications/:id/delivered',c => {
  getDatabase().query('UPDATE ledger_alerts SET notified=1 WHERE id=? AND notified=2').run(c.req.param('id'));
  return c.json({ success: true });
});
app.get('/review',async c => {
  const date = c.req.query('date') ?? c.req.query('week') ?? new Date().toISOString().slice(0,10);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new LedgerInputError('Use YYYY-MM-DD');
  return c.json({ success: true,data: await ledgerReview(date,Boolean(c.req.query('week'))) });
});
app.get('/:sessionId',async c => {
  const detail = await ledgerDetail(c.req.param('sessionId'));
  return detail ? conditionalJson(c,detail) : c.json({ success: false,error: 'Ledger not found or excluded' },404);
});
app.put('/:sessionId/items',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  const items = body.data!.items;
  if (!Array.isArray(items) || items.length > 100) throw new LedgerInputError('items must be an array of at most 100');
  for (const item of items) {
    if (!item || typeof item.title !== 'string' || !item.title.trim() || item.title.length > 2000 || !['todo','doing','done','unverified'].includes(item.status)) throw new LedgerInputError('Invalid requirement item');
    if (!item.anchors || !['paths','commands','keywords'].every(k => Array.isArray(item.anchors[k]) && item.anchors[k].every((v: unknown) => typeof v === 'string'))) throw new LedgerInputError('Invalid anchors');
    if (item.anchors.commandFormat !== undefined && !['legacy-unconfirmed','literal-v2'].includes(item.anchors.commandFormat) || item.anchors.legacyCommands !== undefined && (!Array.isArray(item.anchors.legacyCommands) || !item.anchors.legacyCommands.every((v: unknown) => typeof v === 'string'))) throw new LedgerInputError('Invalid command format');
    if (!Array.isArray(item.constraints) || !item.constraints.every((r: Record<string,unknown>) => r && (r.kind === 'no_public_api_change' || ['path_forbidden','preserve_text'].includes(String(r.kind)) && typeof r.value === 'string'))) throw new LedgerInputError('Invalid constraints');
    if (item.id !== undefined && typeof item.id !== 'string' || item.evidenceIds !== undefined && (!Array.isArray(item.evidenceIds) || !item.evidenceIds.every((id: unknown) => typeof id === 'string'))) throw new LedgerInputError('Invalid item identity or evidence');
  }
  return c.json({ success: true,data: await replaceLedgerItems(c.req.param('sessionId'),items as RequirementItem[]) });
});
app.post('/:sessionId/corrections',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  const data = body.data!;
  const callIds = data.callIds ?? [data.callId];
  if (!Array.isArray(callIds) || !callIds.every(v => typeof v === 'string') || data.itemId !== undefined && typeof data.itemId !== 'string' || data.acceptOffPlan !== undefined && typeof data.acceptOffPlan !== 'boolean') throw new LedgerInputError('Invalid correction');
  const rule = data.rule as LedgerRule | undefined;
  if (rule && (!rule.matcher || !['paths','commands'].every(k => Array.isArray((rule.matcher as unknown as Record<string,unknown[]>)[k]) && (rule.matcher as unknown as Record<string,unknown[]>)[k].every(v => typeof v === 'string')))) throw new LedgerInputError('Invalid rule matcher');
  return c.json({ success: true,data: await correctLedger(c.req.param('sessionId'),callIds,data.itemId as string | undefined,data.acceptOffPlan === true,rule) });
});
app.post('/:sessionId/redecompose',async c => {
  const detail = await getLedger(c.req.param('sessionId')); if (!detail) return c.json({ success: false,error: 'Ledger not found' },404);
  const db = getDatabase();
  // A corrected target is a user choice; keep it and its rules when replacing model items.
  db.query(`UPDATE requirement_items SET anchors=CASE WHEN source<>'user' THEN json_set(anchors,'$.commandFormat','literal-v2') ELSE anchors END,source='user' WHERE agent_session_id=? AND id IN
    (SELECT requirement_item_id FROM ledger_corrections WHERE agent_session_id=? UNION SELECT requirement_item_id FROM ledger_rules WHERE agent_session_id=?)`).run(detail.agentSessionId,detail.agentSessionId,detail.agentSessionId);
  db.query("DELETE FROM requirement_items WHERE agent_session_id = ? AND source <> 'user' AND deleted_by_user = 0").run(detail.agentSessionId);
  getDatabase().query('DELETE FROM ledger_judgments WHERE agent_session_id = ?').run(detail.agentSessionId);
  return c.json({ success: true,data: await getLedger(detail.sessionId) });
});
app.post('/:sessionId/acceptances',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  const data = body.data!;
  if (!['accepted','accepted_with_gaps','follow_up'].includes(String(data.decision)) || data.reason !== undefined && typeof data.reason !== 'string' || data.droppedItemIds !== undefined && (!Array.isArray(data.droppedItemIds) || !data.droppedItemIds.every(v => typeof v === 'string'))) throw new LedgerInputError('Invalid acceptance');
  return c.json({ success: true,data: await acceptLedger(c.req.param('sessionId'),{ decision: data.decision as LedgerAcceptance['decision'],droppedItemIds: data.droppedItemIds as string[] ?? [],reason: data.reason as string | undefined }) });
});
app.get('/:sessionId/follow-up',async c => {
  const detail = await getLedger(c.req.param('sessionId')); if (!detail) return c.json({ success: false,error: 'Ledger not found' },404);
  return c.json({ success: true,data: { text: followUpPrompt(detail) } });
});
app.get('/:sessionId/correction',async c => {
  const detail = await getLedger(c.req.param('sessionId')); if (!detail) return c.json({ success: false,error: 'Ledger not found' },404);
  const runId = c.req.query('runId'); if (!runId) throw new LedgerInputError('runId is required');
  return c.json({ success: true,data: { text: followUpPrompt(detail,runId) } });
});
app.post('/:sessionId/attribution',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  const id = body.data!.workItemId;
  if (id !== null && typeof id !== 'string') throw new LedgerInputError('workItemId must be a todo id or null');
  return c.json({ success: true,data: await attributeLedger(c.req.param('sessionId'),id) });
});
app.post('/:sessionId/carry-over',async c => c.json({ success: true,data: await carryOver(c.req.param('sessionId')) }));
app.post('/:sessionId/import-requirements',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  if (typeof body.data!.fromSessionId !== 'string') throw new LedgerInputError('fromSessionId is required');
  return c.json({ success: true,data: await importLedgerRequirements(c.req.param('sessionId'),body.data!.fromSessionId) });
});
app.post('/:sessionId/viewing',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  if (typeof body.data!.viewed !== 'boolean') throw new LedgerInputError('viewed must be boolean');
  const sessionId = c.req.param('sessionId');
  const detail = body.data!.viewed ? await getLedger(sessionId) : null;
  const turnId = detail?.turns.find(t => t.id === detail.turnId)?.phase === 'completed' ? detail.turnId : undefined;
  markLedgerViewed(sessionId,body.data!.viewed,Date.now(),turnId);
  if (detail?.unread) emit('ledger:update',{ sessionId });
  return c.json({ success: true });
});
export const ledgerSettings = new Hono(); ledgerSettings.use('*',authMiddleware);
ledgerSettings.get('/',c => c.json({ success: true,data: config.get().ledger }));
ledgerSettings.put('/',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  const next = mergeLedgerConfig(body.data!); const errors = validateLedgerConfig(next);
  if (errors.length) return c.json({ success: false,error: errors.join('; ') },400);
  config.set('ledger',next); return c.json({ success: true,data: next });
});
export default app;
