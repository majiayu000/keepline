import { workItemRepository } from '../../infrastructure/database/repositories/work-item.repository.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { config } from '../../lib/config.js';
import { ledgerOverview, LedgerInputError } from './service.js';

export function goalRows(area?: string, now = new Date()) {
  const items = workItemRepository.findAll({ includeArchived: true }); const db = getDatabase();
  const sessions = db.query(`SELECT a.*,l.work_item_id FROM agent_sessions a JOIN work_item_session_links l ON l.agent_session_id=a.id WHERE l.acceptance_status='accepted'`).all() as Array<{ id: string; runtime_session_id: string; title: string; status: string; last_active_at: string; work_item_id: string }>;
  return items.filter(i => i.level === 'goal' && i.status !== 'archived' && (!area || i.area === area)).map(goal => {
    const todos = items.filter(t => t.parentId === goal.id && t.status !== 'archived').map(todo => {
      const linked = sessions.filter(s => s.work_item_id === todo.id);
      const checklist = (todo.acceptance ?? []).map(c => {
        const satisfied = linked.some(s => {
          const acceptances = ledgerRepository.acceptances(s.id).filter(a => a.decision !== 'follow_up');
          const entries = ledgerRepository.items(s.id);
          return entries.some(i => i.checklistId === c.id && i.status === 'done' && i.evidenceIds.length && acceptances.some(a => !a.droppedItemIds.includes(i.id) && i.evidenceIds.every(id => {
            const evidence = db.query('SELECT occurred_at FROM progress_evidence WHERE id = ? AND agent_session_id = ?').get(id,s.id) as { occurred_at: string } | null;
            return evidence && evidence.occurred_at <= a.at;
          })));
        });
        return { ...c, satisfied };
      });
      return { ...todo, checklist, readyToComplete: checklist.length > 0 && checklist.every(c => c.satisfied) && todo.status !== 'done', sessions: linked };
    });
    const lastMovement = Math.max(goal.updatedAt.getTime(), ...todos.map(t => t.updatedAt.getTime()), ...todos.flatMap(t => t.sessions.map(s => Date.parse(s.last_active_at))));
    return { ...goal, todos, progress: { done: todos.filter(t => t.status === 'done').length, total: todos.length,
      active: todos.filter(t => t.sessions.some(s => s.status === 'running' || s.status === 'needs_input')).length },
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
  const rows = await ledgerOverview(config.get().ledger.retentionDays * 24);
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
