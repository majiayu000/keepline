import { statSync, existsSync } from 'fs';
import { createHash } from 'crypto';
import { readLedgerComputation, writeLedgerComputation } from '../../infrastructure/session-summary-cache.js';
import { events } from '../../lib/events.js';
import { logger } from '../../lib/logger.js';
import { randomUUID } from 'crypto';
import { config } from '../../lib/config.js';
import { getDatabase, transaction } from '../../infrastructure/database/sqlite.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { ledgerRepository } from '../../infrastructure/database/repositories/ledger.repository.js';
import { workItemRepository } from '../../infrastructure/database/repositories/work-item.repository.js';
import { workItemEvidenceRepository } from '../../infrastructure/database/repositories/work-item-evidence.repository.js';
import type { ParsedSessionData, Session } from '../../domain/session/index.js';
import type { LedgerAcceptance, LedgerDetail, LedgerRule, RequirementItem, TranscriptFact } from '../../domain/ledger/types.js';
import { anchorsFromText, decompose, extractAsks, followUpSuggestions, ledgerId, matchLedger } from '../../domain/ledger/matcher.js';
import { readCodexMetadata } from '../../adapters/codex/liveness.js';
import { readTranscriptFacts } from './facts.js';
import { getCodexSessionById } from '../../adapters/codex/scanner.js';
import { getSessionById } from '../../adapters/claude/scanner.js';
import { evaluateLedgerAlerts } from './alerts.js';
import { judgeLedger } from './judge.js';

export class LedgerInputError extends Error {}
const parsedSnapshots = new Map<string, { path?: string; facts?: TranscriptFact[]; unknownRecords: number }>();
export function clearLedgerCache() { parsedSnapshots.clear(); }
export function ledgerEnabled(session: Pick<Session, 'client' | 'directory' | 'lastActiveAt'>) {
  const cfg = config.get().ledger;
  return cfg.enabled && !cfg.exclude.runtimes.includes(session.client === 'codex' ? 'codex' : 'claude-code') &&
    !cfg.exclude.projects.some(p => session.directory === p || session.directory.startsWith(`${p}/`)) &&
    session.lastActiveAt.getTime() >= Date.now() - cfg.retentionDays * 86400000;
}
type LedgerStatus = Pick<LedgerDetail,'sessionId' | 'agentSessionId' | 'title' | 'state' | 'statusReason' | 'possiblyWaiting' | 'limited' | 'lastActiveAt' | 'turnId' | 'acceptances' | 'claims' | 'progress' | 'offPlan' | 'turns'>;
interface LedgerScanSnapshot { path: string; transcript: string; signature: string; statusKey: string; transcriptLimited: boolean; summary: LedgerStatus; lastTurn?: Extract<TranscriptFact,{ kind: 'turn' }> }
function transcriptFingerprint(path: string): string {
  const info = statSync(path);
  return `ledger-9:${config.get().ledger.retentionDays}:${new Date().toISOString().slice(0,10)}:${info.size}:${info.mtimeMs}:${info.ctimeMs}`;
}
export function ledgerScanSnapshot(sessionId: string, path: string): LedgerScanSnapshot | undefined {
  const row = readLedgerComputation<LedgerScanSnapshot>(`scan:${sessionId}`);
  return row?.transcript === transcriptFingerprint(path) ? row : undefined;
}
function computationSignature(session: Session, transcript: string): string {
  const db = getDatabase();
  const agent = db.query('SELECT id FROM agent_sessions WHERE runtime_session_id=? AND runtime_id=?').get(session.sessionId,session.client === 'codex' ? 'codex' : 'claude-code') as { id: string } | null;
  const id = agent?.id ?? '';
  const edits = ['ledger_corrections','ledger_rules','ledger_acceptances','ledger_attributions','ledger_judgments'].map(table => db.query(`SELECT * FROM ${table} WHERE agent_session_id=?`).all(id));
  const items = db.query("SELECT id,title,anchors,constraints,source,status,status_source,deleted_by_user FROM requirement_items WHERE agent_session_id=?").all(id);
  const links = db.query("SELECT l.work_item_id,w.acceptance FROM work_item_session_links l JOIN work_items w ON w.id=l.work_item_id WHERE l.agent_session_id=? AND l.acceptance_status='accepted'").all(id);
  return `${transcript}:` + createHash('sha256').update(JSON.stringify([transcript,id,session.directory,config.get().ledger,edits,items,links])).digest('hex');
}
function currentGoalStatus(session: Session) { return session.client === 'codex' ? readCodexMetadata(session.sessionId.replace(/^codex_/, '')).limited : undefined; }
function statusKey(session: Session, goalStatus?: string) { return JSON.stringify([session.title,session.status,session.statusSource,session.statusReason,session.lastActiveAt,goalStatus]); }
function refreshStatus(detail: LedgerStatus,session: Session,goalStatus: string | undefined,transcriptLimited: boolean) {
  const turn = detail.turns.find(t => t.id === detail.turnId);
  detail.state = turn?.phase === 'completed' ? 'review' : 'running';
  if (detail.acceptances.some(a => a.turnId === detail.turnId && a.decision !== 'follow_up')) detail.state = 'accepted';
  if (turn?.phase === 'aborted' || ['lost','stalled','interrupted'].includes(session.status)) detail.state = 'stopped';
  if (session.status === 'needs_input' && session.statusSource === 'hook') detail.state = 'needs_input';
  if (goalStatus) detail.state = 'stopped';
  detail.title = session.title; detail.lastActiveAt = session.lastActiveAt.toISOString();
  detail.statusReason = goalStatus ? `Codex goal ${goalStatus}` : session.statusReason;
  detail.possiblyWaiting = session.status === 'waiting' && session.statusSource !== 'hook';
  detail.limited = !!goalStatus || transcriptLimited;
}
function refreshImportSuggestions(detail: LedgerDetail,session: Session) {
  const candidates = detail.asks.filter(a => a.kind !== 'question');
  detail.importSuggestions = candidates.length === 0 || candidates.every(a => a.authoredText.length < 20)
    ? getDatabase().query(`SELECT DISTINCT a.runtime_session_id AS sessionId,a.title FROM agent_sessions a JOIN requirement_items r ON r.agent_session_id=a.id
      WHERE a.project_root=? AND a.runtime_id=? AND a.id<>? AND a.last_active_at<=? AND r.deleted_by_user=0 ORDER BY a.last_active_at DESC LIMIT 3`).all(session.directory,detail.runtimeId,detail.agentSessionId,session.startedAt?.toISOString() ?? session.lastActiveAt.toISOString()) as Array<{ sessionId: string; title: string }>
    : [];
}
export async function ingestLedger(session: Session, parsed: ParsedSessionData, factsOverride?: TranscriptFact[]): Promise<LedgerDetail | null> {
  if (!ledgerEnabled(session)) return null;
  const transcript = parsed.sourcePath && !factsOverride ? transcriptFingerprint(parsed.sourcePath) : undefined;
  const snapshot = transcript ? ledgerScanSnapshot(session.sessionId,parsed.sourcePath!) : undefined;
  if (snapshot && snapshot.signature === computationSignature(session,transcript!)) {
    const goalStatus = currentGoalStatus(session), key = statusKey(session,goalStatus);
    refreshStatus(snapshot.summary,session,goalStatus,snapshot.transcriptLimited);
    if (key !== snapshot.statusKey) {
      workItemEvidenceRepository.upsertAgentSession({ runtimeId: session.client === 'codex' ? 'codex' : 'claude-code',runtimeSessionId: session.sessionId,cwd: session.directory,projectRoot: session.directory,title: session.title,status: session.status,lastActiveAt: session.lastActiveAt });
      snapshot.statusKey = key; writeLedgerComputation(`scan:${session.sessionId}`,transcript!,snapshot);
    }
    await evaluateLedgerAlerts(snapshot.summary);
    return null;
  }
  const derived = factsOverride ? { facts: factsOverride, unknownRecords: 0 } : parsed.transcriptFacts ? { facts: parsed.transcriptFacts, unknownRecords: parsed.unknownRecords ?? 0 }
    : parsed.sourcePath ? await readTranscriptFacts(parsed.sourcePath, session.client) : { facts: [], unknownRecords: 0 };
  if (parsedSnapshots.size >= 100) parsedSnapshots.delete(parsedSnapshots.keys().next().value!);
  parsedSnapshots.set(session.sessionId,parsed.sourcePath ? { path: parsed.sourcePath,unknownRecords: derived.unknownRecords } : derived);
  const detail = await buildLedger(session, derived.facts, derived.unknownRecords);
  if (transcript && transcriptFingerprint(parsed.sourcePath!) === transcript) {
    const signature = computationSignature(session,transcript);
    writeLedgerComputation(`detail:${session.sessionId}`,signature,detail);
    const { sessionId,agentSessionId,title,state,statusReason,possiblyWaiting,limited,lastActiveAt,turnId,acceptances,claims,progress,offPlan } = detail;
    const summary: LedgerStatus = { sessionId,agentSessionId,title,state,statusReason,possiblyWaiting,limited,lastActiveAt,turnId,acceptances,claims,progress,offPlan,turns: detail.turns.filter(t => t.id === turnId) };
    writeLedgerComputation(`scan:${session.sessionId}`,transcript,{ path: parsed.sourcePath,transcript,signature,summary,transcriptLimited: derived.facts.some(f => f.kind === 'limit'),statusKey: statusKey(session,currentGoalStatus(session)),lastTurn: [...derived.facts].reverse().find(f => f.kind === 'turn') });
  }
  return detail;
}
export function suggestAttribution(session: Session, ask: string) {
  return workItemRepository.findAll().filter(t => t.level !== 'goal' && t.kind === 'todo' && !['done','archived'].includes(t.status)).map(todo => {
    const reasons: string[] = []; let score = 0;
    if (todo.projectRoot === session.directory) { score += 5; reasons.push('Same project'); }
    if (ask.toLowerCase().includes(todo.title.toLowerCase()) || ask.includes(todo.id)) { score += 6; reasons.push('Ask names this todo'); }
    const words = anchorsFromText(`${todo.title} ${(todo.acceptance ?? []).map(c => c.text).join(' ')}`).keywords;
    const hits = words.filter(w => ask.toLowerCase().includes(w));
    if (hits.length) { score += Math.min(3,hits.length); reasons.push(`Matching terms: ${hits.slice(0,4).join(', ')}`); }
    if (todo.status === 'active') { score++; reasons.push('Todo is active'); }
    return { workItemId: todo.id, title: todo.title, score, reasons };
  }).filter(t => t.score >= 6).sort((a,b) => b.score - a.score).slice(0,3);
}
async function buildLedger(session: Session, facts: TranscriptFact[], unknownRecords = 0): Promise<LedgerDetail> {
  const cfg = config.get().ledger;
  const runtimeId = session.client === 'codex' ? 'codex' : 'claude-code';
  const goalStatus = session.client === 'codex' ? readCodexMetadata(session.sessionId.replace(/^codex_/, '')).limited : undefined;
  const agent = workItemEvidenceRepository.upsertAgentSession({ runtimeId, runtimeSessionId: session.sessionId, cwd: session.directory,
    projectRoot: session.directory, title: session.title, status: session.status, lastActiveAt: session.lastActiveAt });
  const db = getDatabase();
  const link = workItemEvidenceRepository.findAcceptedSessionLinks(agent.id)[0];
  const workItem = link && workItemRepository.findById(link.workItemId);
  const asks = extractAsks(facts, session.sessionId);
  ledgerRepository.saveAsks(agent.id, asks);
  const existing = ledgerRepository.items(agent.id, true);
  const modelAsks = asks.filter(ask => !existing.some(item => item.id === ledgerId(agent.id,'item',ask.id) && (item.dropped || item.source === 'user')));
  const computed = workItem?.acceptance?.length ? decompose(asks,agent.id,workItem.acceptance)
    : (await judgeLedger(agent.id,modelAsks)) ?? decompose(asks,agent.id);
  const items = computed.filter(i => !existing.some(e => e.id === i.id && e.dropped)).map(i => {
    const old = existing.find(e => e.id === i.id);
    return old?.source === 'user' ? old : old && old.title === i.title && JSON.stringify(old.anchors) === JSON.stringify(i.anchors) ? { ...i, status: old.status, statusSource: old.statusSource, evidenceIds: old.evidenceIds } : i;
  });
  items.push(...existing.filter(i => i.source === 'user' && !i.dropped && !items.some(c => c.id === i.id)));
  const matched = matchLedger(facts, items, ledgerRepository.corrections(agent.id), ledgerRepository.rules(agent.id), cfg, agent.id);
  ledgerRepository.saveItems(agent.id, matched.items);
  // Preserve references to obsolete derived items without treating retirement as a user deletion.
  for (const old of existing) if (old.source !== 'user' && !matched.items.some(i => i.id === old.id)) db.query("UPDATE requirement_items SET status='unverified',evidence_ids='[]' WHERE id=?").run(old.id);
  ledgerRepository.evidence(agent.id, matched.evidence, runtimeId, link?.workItemId);
  const acceptances = ledgerRepository.acceptances(agent.id);
  const lastTurn = [...facts].reverse().find(f => f.kind === 'turn');
  const turnId = lastTurn?.kind === 'turn' ? lastTurn.turnId : undefined;
  const acceptance = acceptances.find(a => a.turnId === turnId);
  let state: LedgerDetail['state'] = lastTurn?.kind === 'turn' && lastTurn.phase === 'completed' ? 'review' : 'running';
  if (acceptance && acceptance.decision !== 'follow_up') state = 'accepted';
  if (lastTurn?.kind === 'turn' && lastTurn.phase === 'aborted' || ['lost','stalled','interrupted'].includes(session.status)) state = 'stopped';
  if (session.status === 'needs_input' && session.statusSource === 'hook') state = 'needs_input';
  if (goalStatus) state = 'stopped';
  const toolTurns = new Map<string | undefined,string | undefined>();
  const failedCalls = new Set<string>();
  const readOnlyByTurn = new Map<string | undefined,number>();
  for (const f of facts) if (f.kind === 'tool') {
    toolTurns.set(f.callId,f.turnId);
    if (f.facts?.some(e => e.kind === 'test' && e.exitCode !== 0)) failedCalls.add(f.callId);
    if (!f.mutating) readOnlyByTurn.set(f.turnId,(readOnlyByTurn.get(f.turnId) ?? 0)+1);
  }
  const evidenceByTurn = new Map<string | undefined,Array<{ evidence: typeof matched.evidence[number]; text: string }>>();
  for (const e of matched.evidence) if (e.exitCode === 0 && !failedCalls.has(e.callId)) {
    const turn = toolTurns.get(e.callId), rows = evidenceByTurn.get(turn) ?? [];
    rows.push({ evidence: e,text: e.value.toLowerCase() }); evidenceByTurn.set(turn,rows);
  }
  const claims = facts.filter((f): f is Extract<TranscriptFact,{kind:'agent_message'}> => f.kind === 'agent_message' && f.final)
    .flatMap(f => f.text.split(/\n+/).filter(t => /(?:\bdone\b|\bcompleted?\b|\bpassed\b|\bfixed\b|\bimplemented\b|完成|通过|已修复|已实现)/i.test(t)).map(text => {
      const words = anchorsFromText(text).keywords;
      return { text,turnId: f.turnId,evidenceIds: (evidenceByTurn.get(f.turnId) ?? []).filter(e => words.some(w => e.text.includes(w))).map(e => e.evidence.id) };
    }));
  const turnMap = new Map<string,LedgerDetail['turns'][number]>();
  for (const f of facts) if (f.kind === 'turn') {
    const turn = turnMap.get(f.turnId) ?? { id: f.turnId,at: f.at,phase: f.phase,readOnlyCount: 0,durationMs: 0 };
    turn.phase = f.phase;
    turn.durationMs = Math.max(0,Date.parse(f.at) - Date.parse(turn.at));
    turnMap.set(f.turnId,turn);
  }
  const turns = [...turnMap.values()];
  for (const turn of turns) {
    turn.readOnlyCount = readOnlyByTurn.get(turn.id) ?? 0;
    if (turn.phase === 'started') turn.durationMs = Math.max(0,session.lastActiveAt.getTime() - Date.parse(turn.at));
  }
  const noGoal = (db.query('SELECT no_goal FROM ledger_attributions WHERE agent_session_id = ?').get(agent.id) as { no_goal: number } | null)?.no_goal;
  const attribution = !link && !noGoal ? suggestAttribution(session, asks.filter(a => a.kind !== 'question').map(a => a.authoredText).join('\n')) : [];
  for (const suggestion of attribution) workItemEvidenceRepository.createSessionLink({ workItemId: suggestion.workItemId, agentSessionId: agent.id, linkSource: 'heuristic_suggestion' });
  const detail: LedgerDetail = {
    sessionId: session.sessionId, agentSessionId: agent.id, title: session.title, projectRoot: session.directory, runtimeId,
    state, statusReason: session.statusReason, possiblyWaiting: session.status === 'waiting' && session.statusSource !== 'hook',
    available: facts.length > 0 && unknownRecords < Math.max(10,facts.length), unavailableReason: facts.length ? unknownRecords >= Math.max(10,facts.length) ? 'Unrecognized transcript format; session status only' : undefined : 'Transcript ledger unavailable',
    asks: ledgerRepository.asks(agent.id), ...matched, turns, claims, acceptances, turnId, workItemId: link?.workItemId,
    lastActiveAt: session.lastActiveAt.toISOString(), attribution, limited: !!goalStatus || facts.some(f => f.kind === 'limit'),
    followUpSuggestions: followUpSuggestions(facts),
  };
  refreshImportSuggestions(detail,session);
  if (goalStatus) detail.statusReason = `Codex goal ${goalStatus}`;
  if (acceptance?.decision === 'accepted_with_gaps') for (const item of detail.items) item.dropped = acceptance.droppedItemIds.includes(item.id);
  await evaluateLedgerAlerts(detail);
  return detail;
}
export async function getLedger(sessionId: string): Promise<LedgerDetail | null> {
  const session = sessionRepository.findBySessionId(sessionId);
  if (!session || !ledgerEnabled(session)) return null;
  const persisted = readLedgerComputation<LedgerScanSnapshot>(`scan:${sessionId}`);
  if (persisted?.path && existsSync(persisted.path)) return ledgerFromPath(session,persisted.path);
  const snapshot = parsedSnapshots.get(sessionId);
  if (snapshot) {
    const derived = { facts: snapshot.facts ?? [],unknownRecords: snapshot.unknownRecords };
    if (snapshot.path) return ledgerFromPath(session,snapshot.path);
    return buildLedger(session,derived.facts,derived.unknownRecords);
  }
  const parsed = session.client === 'codex' ? await getCodexSessionById(sessionId,false) : await getSessionById(sessionId,false);
  if (parsed?.sourcePath) return ledgerFromPath(session,parsed.sourcePath);
  return ingestLedger(session,parsed ?? { sessionId: session.sessionId, directory: session.directory, lastActiveAt: session.lastActiveAt, messageCount: session.messageCount, toolCount: session.toolCount });
}
async function ledgerFromPath(session: Session, path: string): Promise<LedgerDetail> {
  const snapshot = ledgerScanSnapshot(session.sessionId,path);
  const signature = computationSignature(session,transcriptFingerprint(path));
  const detail = snapshot?.signature === signature ? readLedgerComputation<LedgerDetail>(`detail:${session.sessionId}`,signature) : undefined;
  if (detail) { refreshStatus(detail,session,currentGoalStatus(session),snapshot?.transcriptLimited ?? false); refreshImportSuggestions(detail,session); await evaluateLedgerAlerts(detail); return detail; }
  const parsed = { sessionId: session.sessionId,directory: session.directory,lastActiveAt: session.lastActiveAt,messageCount: session.messageCount,toolCount: session.toolCount,sourcePath: path };
  if (snapshot?.signature === signature) {
    const derived = await readTranscriptFacts(path,session.client);
    return buildLedger(session,derived.facts,derived.unknownRecords);
  }
  return (await ingestLedger(session,parsed))!;
}
export async function ledgerOverview(hours = 24) {
  const sessions = sessionRepository.findAll().filter(s => s.lastActiveAt.getTime() >= Date.now() - hours * 3600000 && ledgerEnabled(s));
  const rows: LedgerDetail[] = [];
  for (const session of sessions) { const detail = await getLedger(session.sessionId); if (detail) rows.push(detail); }
  const urgency = (d: LedgerDetail) => d.state === 'needs_input' || d.offPlan.length || d.state === 'review' && d.progress.done < d.progress.total ? 0 : d.state === 'accepted' ? 2 : 1;
  return rows.sort((a,b) => urgency(a) - urgency(b) || Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt));
}
export function followUpPrompt(detail: LedgerDetail, correctionRun?: string): string {
  if (correctionRun) {
    const run = detail.offPlan.find(r => r.id === correctionRun);
    if (!run) throw new LedgerInputError('Off-plan run not found');
    const steps = detail.trail.filter(s => run.callIds.includes(s.callId));
    return `Please return to the requested scope.\n\nOriginal asks:\n${detail.asks.map(a => a.text).join('\n\n')}\n\nReview these steps:\n${steps.map(s => `${s.summary}\n${s.violations.join('\n')}`).join('\n')}\n\nExplain why these steps are necessary, or correct the deviation. Cite verifiable evidence.`;
  }
  const remaining = detail.items.filter(i => i.status !== 'done' || !i.evidenceIds.length);
  return `Please finish or provide evidence for these requirements:\n${remaining.map(i => `- ${i.title}${i.dropped ? ' (previously dropped)' : ''}`).join('\n')}\n\nClaims needing evidence:\n${detail.claims.filter(c => !c.evidenceIds.length).map(c => `- ${c.text}`).join('\n')}\n\nConstraints:\n${detail.items.flatMap(i => i.constraints.map(c => `- ${c.kind}: ${c.value ?? ''}`)).join('\n')}\n\nReport the actual checks and their results. Do not claim completion without evidence.`;
}
export async function acceptLedger(sessionId: string, input: Omit<LedgerAcceptance, 'turnId' | 'at'>) {
  const detail = await getLedger(sessionId);
  if (!detail || !detail.turnId || !['review','accepted'].includes(detail.state)) throw new LedgerInputError('Only a finished turn can be accepted');
  if (input.decision === 'accepted' && detail.progress.done !== detail.progress.total) throw new LedgerInputError('Remaining items require accept-with-gaps or follow-up');
  const missing = detail.items.filter(i => i.status !== 'done' || !i.evidenceIds.length).map(i => i.id);
  if (input.decision === 'accepted_with_gaps' && (!input.reason?.trim() || missing.some(id => !input.droppedItemIds.includes(id)))) throw new LedgerInputError('Give a reason and drop every remaining item');
  if (input.droppedItemIds.some(id => !detail.items.some(i => i.id === id))) throw new LedgerInputError('Unknown dropped item');
  ledgerRepository.accept(detail.agentSessionId,{ ...input, turnId: detail.turnId, at: new Date().toISOString() });
  return getLedger(sessionId);
}
export async function correctLedger(sessionId: string, callIds: string[], itemId?: string, acceptOffPlan = false, rule?: LedgerRule) {
  const detail = await getLedger(sessionId); if (!detail) throw new LedgerInputError('Session not found');
  if (!callIds.length || callIds.some(id => !detail.trail.some(s => s.callId === id))) throw new LedgerInputError('Unknown step');
  if (itemId && !detail.items.some(i => i.id === itemId) || rule && (rule.itemId !== itemId || !rule.matcher.paths.length && !rule.matcher.commands.length)) throw new LedgerInputError('Invalid correction item or rule');
  transaction(() => callIds.forEach((callId,i) => ledgerRepository.correct(detail.agentSessionId,{ callId,itemId,acceptOffPlan },i === 0 ? rule : undefined)));
  return getLedger(sessionId);
}
export async function attributeLedger(sessionId: string, workItemId: string | null) {
  const detail = await getLedger(sessionId); if (!detail) throw new LedgerInputError('Session not found');
  const item = workItemId ? workItemRepository.findById(workItemId) : null;
  if (workItemId && (!item || item.level === 'goal' || item.kind !== 'todo')) throw new LedgerInputError('Choose a todo');
  transaction(() => {
    const db = getDatabase(); const now = new Date().toISOString();
    db.query("UPDATE work_item_session_links SET acceptance_status='rejected',updated_at=? WHERE agent_session_id=?").run(now,detail.agentSessionId);
    if (workItemId) {
      const existing = db.query('SELECT id FROM work_item_session_links WHERE agent_session_id = ? AND work_item_id = ?').get(detail.agentSessionId,workItemId) as { id: string } | null;
      if (existing) db.query("UPDATE work_item_session_links SET acceptance_status='accepted',link_source='user',accepted_at=?,updated_at=? WHERE id=?").run(now,now,existing.id);
      else workItemEvidenceRepository.createSessionLink({ agentSessionId: detail.agentSessionId, workItemId, linkSource: 'user' });
    }
    db.query('INSERT INTO ledger_attributions(agent_session_id,no_goal) VALUES(?,?) ON CONFLICT(agent_session_id) DO UPDATE SET no_goal=excluded.no_goal').run(detail.agentSessionId,workItemId ? 0 : 1);
    // Rebuild checklist from the confirmed link, preserving user-created items.
    db.query("DELETE FROM requirement_items WHERE agent_session_id = ? AND source <> 'user'").run(detail.agentSessionId);
  });
  return getLedger(sessionId);
}
export async function carryOver(sessionId: string) {
  const detail = await getLedger(sessionId); if (!detail) throw new LedgerInputError('Session not found');
  const remaining = detail.items.filter(i => i.status !== 'done' || !i.evidenceIds.length);
  if (!remaining.length) throw new LedgerInputError('No remaining items');
  const db = getDatabase();
  const row = db.query('SELECT carry_item_id FROM ledger_attributions WHERE agent_session_id = ?').get(detail.agentSessionId) as { carry_item_id: string | null } | null;
  const parent = detail.workItemId && workItemRepository.findById(detail.workItemId)?.parentId;
  const input = { title: `Continue: ${detail.title}`.slice(0,200), projectRoot: detail.projectRoot, parentId: parent || null, acceptance: remaining.map(i => ({ id: i.checklistId ?? i.id, text: i.title, completed: false })) };
  const item = row?.carry_item_id ? workItemRepository.update(row.carry_item_id,input) : null;
  const result = item ?? workItemRepository.create(input);
  db.query('INSERT INTO ledger_attributions(agent_session_id,carry_item_id) VALUES(?,?) ON CONFLICT(agent_session_id) DO UPDATE SET carry_item_id=excluded.carry_item_id').run(detail.agentSessionId,result.id);
  return result;
}
export async function importLedgerRequirements(sessionId: string, fromSessionId: string) {
  const target = await getLedger(sessionId);
  if (!target?.importSuggestions?.some(s => s.sessionId === fromSessionId)) throw new LedgerInputError('Choose a suggested previous session');
  const previous = await getLedger(fromSessionId);
  if (!previous?.items.length) throw new LedgerInputError('Previous session has no requirements');
  const items = previous.items.filter(i => !i.dropped).map((i,ordinal) => ({ ...i,id: randomUUID(),ordinal,source: 'user' as const,status: 'todo' as const,statusSource: 'rule' as const,evidenceIds: [],checklistId: undefined }));
  ledgerRepository.replaceUserItems(target.agentSessionId,items);
  return getLedger(sessionId);
}
export async function replaceLedgerItems(sessionId: string, values: RequirementItem[]) {
  const detail = await getLedger(sessionId); if (!detail) throw new LedgerInputError('Session not found');
  const ids = new Set<string>();
  const own = ledgerRepository.items(detail.agentSessionId,true);
  const items = values.map((value, ordinal) => {
    const id = value.id || randomUUID();
    if (ids.has(id)) throw new LedgerInputError('Duplicate item id'); ids.add(id);
    if (value.id && !own.some(i => i.id === id)) throw new LedgerInputError('Unknown item id');
    const old = own.find(i => i.id === id);
    return { ...value,id,ordinal,source: 'user' as const, statusSource: value.status !== old?.status ? 'user' as const : old?.statusSource ?? 'rule' as const, evidenceIds: (value.evidenceIds ?? []).filter(id => detail.evidence.some(e => e.id === id)), checklistId: old?.checklistId };
  });
  transaction(() => {
    // Record the correction against raw candidates too, so toggling AI cannot resurrect them.
    ledgerRepository.saveItems(detail.agentSessionId,decompose(detail.asks,detail.agentSessionId).filter(candidate => !own.some(item => item.id === candidate.id && item.source === 'user')));
    ledgerRepository.replaceUserItems(detail.agentSessionId,items);
  });
  return getLedger(sessionId);
}

// Hook handling remains at composition level, outside the adapter dependency boundary.
for (const event of ['session:discovered','session:updated'] as const) events.on(event,({ session }) => {
  if (session.statusSource !== 'hook' || !ledgerEnabled(session)) return;
  void getLedger(session.sessionId).then(() => {
    events.emit('ledger:update',{ sessionId: session.sessionId });
  }).catch(error => logger.warn('Ledger hook update failed',{ error: error instanceof Error ? error.message : String(error) }));
});
