import { randomUUID } from 'crypto';
import { getDatabase, transaction } from '../sqlite.js';
import type { Ask, Correction, LedgerAcceptance, LedgerEvidence, LedgerRule, RequirementItem } from '../../../domain/ledger/types.js';

type Row = Record<string, any>;
export const ledgerRepository = {
  asks(sessionId: string): Ask[] {
    return (getDatabase().query('SELECT * FROM ledger_asks WHERE agent_session_id = ? ORDER BY ordinal').all(sessionId) as Row[])
      .map(r => ({ id: r.id, text: r.text, authoredText: r.authored_text, kind: r.kind, at: r.occurred_at, turnId: r.turn_id ?? undefined }));
  },
  saveAsks(sessionId: string, asks: Ask[]) {
    const db = getDatabase(); const now = new Date().toISOString();
    transaction(() => {
      db.query('DELETE FROM ledger_asks WHERE agent_session_id=?').run(sessionId);
      asks.forEach((a, i) => db.query(`INSERT OR IGNORE INTO ledger_asks(id,agent_session_id,ordinal,text,authored_text,kind,occurred_at,created_at,turn_id) VALUES (?,?,?,?,?,?,?,?,?)`).run(a.id,sessionId,i,a.text,a.authoredText,a.kind,a.at,now,a.turnId ?? null));
    });
  },
  items(sessionId: string, includeDeleted = false): RequirementItem[] {
    return (getDatabase().query(`SELECT * FROM requirement_items WHERE agent_session_id = ? ${includeDeleted ? '' : 'AND deleted_by_user = 0'} ORDER BY ordinal`).all(sessionId) as Row[])
      .map(r => ({ id: r.id, ordinal: r.ordinal, title: r.title, anchors: JSON.parse(r.anchors), constraints: JSON.parse(r.constraints), source: r.source,
        status: r.status, statusSource: r.status_source, evidenceIds: JSON.parse(r.evidence_ids), checklistId: r.checklist_id ?? undefined, dropped: Boolean(r.deleted_by_user) }));
  },
  saveItems(sessionId: string, items: RequirementItem[]) {
    const db = getDatabase(); const now = new Date().toISOString();
    transaction(() => {
      for (const i of items) db.query(`INSERT INTO requirement_items(id,agent_session_id,ordinal,title,anchors,constraints,source,status,status_source,evidence_ids,checklist_id,created_at,updated_at)
        VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET ordinal=excluded.ordinal,title=excluded.title,anchors=excluded.anchors,constraints=excluded.constraints,
        source=excluded.source,status=excluded.status,status_source=excluded.status_source,evidence_ids=excluded.evidence_ids,checklist_id=excluded.checklist_id,updated_at=excluded.updated_at
        WHERE requirement_items.agent_session_id=excluded.agent_session_id`).run(i.id,sessionId,i.ordinal,i.title,JSON.stringify(i.anchors),JSON.stringify(i.constraints),i.source,i.status,i.statusSource,JSON.stringify(i.evidenceIds),i.checklistId ?? null,now,now);
    });
  },
  replaceUserItems(sessionId: string, items: RequirementItem[]) {
    transaction(() => {
      const db = getDatabase();
      db.query('UPDATE requirement_items SET deleted_by_user = 1 WHERE agent_session_id = ?').run(sessionId);
      this.saveItems(sessionId, items);
      for (const i of items) db.query('UPDATE requirement_items SET deleted_by_user = 0 WHERE id = ? AND agent_session_id = ?').run(i.id,sessionId);
    });
  },
  corrections(sessionId: string): Correction[] {
    return (getDatabase().query('SELECT * FROM ledger_corrections WHERE agent_session_id = ?').all(sessionId) as Row[]).map(r => ({ callId: r.step_call_id, itemId: r.requirement_item_id ?? undefined, acceptOffPlan: !!r.accept_off_plan }));
  },
  rules(sessionId: string): LedgerRule[] {
    return (getDatabase().query('SELECT * FROM ledger_rules WHERE agent_session_id = ? ORDER BY created_at DESC').all(sessionId) as Row[]).map(r => ({ matcher: JSON.parse(r.matcher), itemId: r.requirement_item_id }));
  },
  correct(sessionId: string, correction: Correction, rule?: LedgerRule) {
    transaction(() => {
      const db = getDatabase(); const now = new Date().toISOString();
      db.query(`INSERT INTO ledger_corrections VALUES (?,?,?,?,?,?) ON CONFLICT(agent_session_id,step_call_id) DO UPDATE SET requirement_item_id=excluded.requirement_item_id,accept_off_plan=excluded.accept_off_plan`).run(randomUUID(),sessionId,correction.callId,correction.itemId ?? null,correction.acceptOffPlan ? 1 : 0,now);
      if (rule) db.query('INSERT INTO ledger_rules VALUES (?,?,?,?,?)').run(randomUUID(),sessionId,JSON.stringify(rule.matcher),rule.itemId,now);
    });
  },
  acceptances(sessionId: string): LedgerAcceptance[] {
    return (getDatabase().query('SELECT * FROM ledger_acceptances WHERE agent_session_id = ? ORDER BY created_at').all(sessionId) as Row[]).map(r => ({ turnId: r.turn_id, decision: r.decision, droppedItemIds: JSON.parse(r.dropped_item_ids), reason: r.reason ?? undefined, at: r.created_at }));
  },
  accept(sessionId: string, acceptance: LedgerAcceptance) {
    getDatabase().query(`INSERT INTO ledger_acceptances VALUES (?,?,?,?,?,?,?) ON CONFLICT(agent_session_id,turn_id) DO UPDATE SET decision=excluded.decision,dropped_item_ids=excluded.dropped_item_ids,reason=excluded.reason,created_at=excluded.created_at`).run(randomUUID(),sessionId,acceptance.turnId,acceptance.decision,JSON.stringify(acceptance.droppedItemIds),acceptance.reason ?? null,acceptance.at);
  },
  evidence(sessionId: string, evidence: LedgerEvidence[], runtimeId: string, workItemId?: string) {
    const db = getDatabase();
    transaction(() => {
      for (const e of evidence) db.query(`INSERT INTO progress_evidence(id,work_item_id,agent_session_id,runtime_id,kind,outcome,summary,occurred_at,confidence,metadata)
        VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET work_item_id=excluded.work_item_id,outcome=excluded.outcome,metadata=excluded.metadata`).run(
          e.id,workItemId ?? null,sessionId,runtimeId,e.kind === 'test' ? 'test_result' : e.kind === 'file' ? 'file_change' : 'tool_call',e.exitCode === undefined ? 'progress' : e.exitCode === 0 ? 'completed' : 'failed',e.value,e.at,'explicit',JSON.stringify({ source: 'ledger', ...e }));
    });
  },
  clean(retentionDays: number, now = new Date()): number {
    const db = getDatabase(); const cutoff = new Date(now.getTime() - retentionDays * 86400000).toISOString();
    return transaction(() => {
      let removed = 0;
      for (const table of ['ledger_asks','ledger_corrections','ledger_rules','ledger_acceptances','ledger_alerts','requirement_items','ledger_attributions','ledger_judgments']) removed += db.query(`DELETE FROM ${table} WHERE agent_session_id IN (SELECT id FROM agent_sessions WHERE last_active_at < ?)`).run(cutoff).changes;
      removed += db.query(`DELETE FROM progress_evidence WHERE json_extract(metadata,'$.source') = 'ledger' AND agent_session_id IN (SELECT id FROM agent_sessions WHERE last_active_at < ?)`).run(cutoff).changes;
      return removed;
    });
  },
};
