import { workItemRepository } from '../../infrastructure/database/repositories/work-item.repository.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { config } from '../../lib/config.js';
import { ledgerOverview, LedgerInputError } from './service.js';

export function goalRows(area?: string, now = new Date(), projectMapGoalId?: string) {
  const items = workItemRepository.findAll({ includeArchived: true }); const db = getDatabase();
  const sessions = db.query(`SELECT a.*,l.work_item_id,s.status AS live_status,s.status_source,s.status_reason
    FROM agent_sessions a JOIN work_item_session_links l ON l.agent_session_id=a.id
    LEFT JOIN sessions s ON s.session_id=a.runtime_session_id WHERE l.acceptance_status='accepted'`).all() as Array<{ id: string; runtime_session_id: string; title: string; status: string; last_active_at: string; work_item_id: string; live_status: string | null; status_source: string | null; status_reason: string | null }>;
  const mappedSessions = sessions.map(s => ({ ...s, needsInput: s.live_status === 'needs_input' && s.status_source === 'hook', statusReason: s.status_reason }));
  return items.filter(i => i.level === 'goal' && i.status !== 'archived' && (!area || i.area === area)).map(goal => {
    const todos = items.filter(t => t.parentId === goal.id && t.status !== 'archived').map(todo => {
      const linked = mappedSessions.filter(s => s.work_item_id === todo.id).sort((a,b) => Date.parse(b.last_active_at) - Date.parse(a.last_active_at));
      const records = linked.map(s => ({ session: s, entries: ledgerRepository.items(s.id), acceptances: ledgerRepository.acceptances(s.id).filter(a => a.decision !== 'follow_up') }));
      const checklist = (todo.acceptance ?? []).map(c => {
        let evidenced = false, satisfied = false;
        for (const { session,entries,acceptances } of records) for (const i of entries) {
          if (i.checklistId !== c.id || i.status !== 'done' || !i.evidenceIds.length) continue;
          const evidence = i.evidenceIds.map(id => db.query('SELECT occurred_at FROM progress_evidence WHERE id = ? AND agent_session_id = ?').get(id,session.id) as { occurred_at: string } | null);
          if (!evidence.every(e => e !== null)) continue;
          evidenced = true;
          if (acceptances.some(a => !a.droppedItemIds.includes(i.id) && evidence.every(e => e!.occurred_at <= a.at))) satisfied = true;
        }
        return { ...c, evidenced, satisfied };
      });
      return { ...todo, checklist, readyToComplete: checklist.length > 0 && checklist.every(c => c.satisfied) && todo.status !== 'done', sessions: linked };
    });
    const recent = (projectMapGoalId === goal.id ? todos : []).flatMap(todo => {
      const completed = todo.status === 'done' && todo.completedAt ? [{ id: `todo:${todo.id}`, kind: 'completed', title: todo.title, at: todo.completedAt.toISOString(), todoId: todo.id, sessionId: null as string | null }] : [];
      const ids = todo.sessions.map(s => s.id);
      if (!ids.length) return completed;
      const placeholders = ids.map(() => '?').join(',');
      const records = db.query(`SELECT e.id, json_extract(e.metadata,'$.kind') AS kind, e.summary AS title, e.occurred_at AS at, ? AS todoId, a.runtime_session_id AS sessionId
        FROM progress_evidence e JOIN agent_sessions a ON a.id=e.agent_session_id
        WHERE e.agent_session_id IN (${placeholders}) AND e.confidence='explicit' AND json_extract(e.metadata,'$.source')='ledger'
        AND json_extract(e.metadata,'$.kind') IN ('test','file','commit','pr') AND e.outcome<>'failed' AND e.occurred_at>=? AND e.occurred_at<=?
        UNION ALL SELECT 'accept:'||l.id, l.decision, '会话验收', l.created_at, ?, a.runtime_session_id
        FROM ledger_acceptances l JOIN agent_sessions a ON a.id=l.agent_session_id
        WHERE l.agent_session_id IN (${placeholders}) AND l.decision<>'follow_up' AND l.created_at>=? AND l.created_at<=?
        ORDER BY at DESC LIMIT 6`).all(todo.id,...ids,new Date(now.getTime()-7*86400000).toISOString(),now.toISOString(),todo.id,...ids,new Date(now.getTime()-7*86400000).toISOString(),now.toISOString()) as Array<{ id: string; kind: string; title: string; at: string; todoId: string; sessionId: string | null }>;
      return [...completed,...records];
    }).filter(e => Date.parse(e.at) >= now.getTime()-7*86400000 && Date.parse(e.at) <= now.getTime()).sort((a,b) => Date.parse(b.at)-Date.parse(a.at)).slice(0,6);
    const lastMovement = Math.max(goal.updatedAt.getTime(), ...todos.map(t => t.updatedAt.getTime()), ...todos.flatMap(t => t.sessions.map(s => Date.parse(s.last_active_at))));
    return { ...goal, todos, recent, progress: { done: todos.filter(t => t.status === 'done').length, total: todos.length,
      active: todos.filter(t => t.status !== 'done' && t.sessions.some(s => s.status === 'running' || s.needsInput)).length },
      weeklyMovement: todos.filter(t => t.completedAt && t.completedAt.getTime() >= now.getTime() - 7 * 86400000).length,
      stale: now.getTime() - lastMovement >= config.get().ledger.staleGoalDays * 86400000 };
  });
}
export function completeTodo(id: string) {
  const todo = goalRows().flatMap(g => g.todos).find(t => t.id === id);
  if (!todo) throw new LedgerInputError('Todo not found');
  if (!todo.readyToComplete) throw new LedgerInputError('Checklist is not satisfied by accepted evidence');
  return workItemRepository.update(id,{ status: 'done', statusSource: 'user', acceptance: (todo.acceptance ?? []).map(c => ({ ...c,completed: true })) });
}
export async function ledgerReview(date: string, weekly = false) {
  const start = new Date(`${date}T00:00:00`); if (!Number.isFinite(start.getTime())) throw new LedgerInputError('Invalid review date');
  const end = new Date(start.getTime() + (weekly ? 7 : 1) * 86400000);
  const rows = await ledgerOverview(config.get().ledger.retentionDays * 24,true);
  const inPeriod = (at: string) => Date.parse(at) >= start.getTime() && Date.parse(at) < end.getTime();
  let totalRuntime = 0, unattributedRuntime = 0;
  for (const row of rows) for (const turn of row.turns) {
    const duration = Math.max(0,Math.min(Date.parse(turn.at) + turn.durationMs,end.getTime()) - Math.max(Date.parse(turn.at),start.getTime()));
    totalRuntime += duration; if (!row.workItemId) unattributedRuntime += duration;
  }
  return { start: start.toISOString(),end: end.toISOString(),
    open: rows.filter(r => r.state === 'review' || r.state === 'needs_input' || r.state === 'running' && r.progress.total > 0).map(r => ({ ...r, remaining: r.items.filter(i => i.status !== 'done' || !i.evidenceIds.length) })),
    accepted: rows.filter(r => r.acceptances.some(a => a.decision !== 'follow_up' && inPeriod(a.at))),
    offPlan: rows.flatMap(r => r.offPlan.filter(run => inPeriod(run.at)).map(run => ({ sessionId: r.sessionId,title: r.title,...run }))),
    corrections: getDatabase().query(`SELECT c.*,a.title FROM ledger_corrections c JOIN agent_sessions a ON a.id=c.agent_session_id WHERE c.created_at >= ? AND c.created_at < ?`).all(start.toISOString(),end.toISOString()),
    goals: goalRows(undefined,end),unattributedRuntimeShare: totalRuntime ? unattributedRuntime / totalRuntime : 0,totalRuntimeMs: totalRuntime };
}
