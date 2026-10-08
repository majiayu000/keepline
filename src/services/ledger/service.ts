import { statSync, existsSync } from 'fs';
import { basename } from 'path';
import { confirmedRequirement,ledgerNeedsAttention } from '../../domain/ledger/types.js';
import { createHash } from 'crypto';
import { LEDGER_COMPUTATION_VERSION, readLedgerComputation, writeLedgerComputation } from '../../infrastructure/session-summary-cache.js';
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
type LedgerStatus = Pick<LedgerDetail,'sessionId' | 'agentSessionId' | 'title' | 'state' | 'statusReason' | 'possiblyWaiting' | 'limited' | 'lastActiveAt' | 'turnId' | 'acceptances' | 'claims' | 'progress' | 'offPlan' | 'turns' | 'parentSessionId' | 'projectRoot'>;
interface LedgerScanSnapshot { path: string; transcript: string; signature: string; statusKey: string; transcriptLimited: boolean; summary: LedgerStatus; lastTurn?: Extract<TranscriptFact,{ kind: 'turn' }> }
function transcriptFingerprint(path: string): string {
  const info = statSync(path);
  return `ledger-${LEDGER_COMPUTATION_VERSION}:${config.get().ledger.retentionDays}:${new Date().toISOString().slice(0,10)}:${info.size}:${info.mtimeMs}:${info.ctimeMs}`;
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
function statusKey(session: Session, goalStatus?: string) { return JSON.stringify([session.title,session.status,session.statusSource,session.statusReason,session.lastActiveAt,session.parentSessionId,goalStatus]); }
function refreshStatus(detail: LedgerStatus,session: Session,goalStatus: string | undefined,transcriptLimited: boolean) {
  const turn = detail.turns.find(t => t.id === detail.turnId);
  const completed = turn?.phase === 'completed';
  const reviewExpired = completed && Date.now() - Date.parse(turn.at) - turn.durationMs >= 12 * 3600000;
  detail.state = completed ? detail.progress.total > 0 && !reviewExpired ? 'review' : 'ended' : 'running';
  if (detail.acceptances.some(a => a.turnId === detail.turnId && a.decision !== 'follow_up')) detail.state = 'accepted';
  if (turn?.phase === 'aborted' || !completed && ['lost','stalled','interrupted'].includes(session.status)) detail.state = 'stopped';
  if (session.status === 'needs_input' && session.statusSource === 'hook') detail.state = 'needs_input';
  if (goalStatus) detail.state = 'stopped';
  detail.title = session.title === 'Unknown task' ? session.parentSessionId ? `子任务 · ${session.sessionId.slice(-6)}` : `未命名会话 · ${basename(session.directory)}` : session.title;
  detail.parentSessionId = session.parentSessionId ? session.client === 'codex' && !session.parentSessionId.startsWith('codex_') ? `codex_${session.parentSessionId}` : session.parentSessionId : undefined;
  detail.lastActiveAt = session.lastActiveAt.toISOString();
  detail.statusReason = goalStatus ? `Codex 目标${goalStatus === 'blocked' ? '受阻' : goalStatus === 'budget_limited' ? '预算用尽' : '额度用尽'}` : session.statusReason;
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
    const { sessionId,agentSessionId,title,state,statusReason,possiblyWaiting,limited,lastActiveAt,turnId,acceptances,claims,progress,offPlan,parentSessionId,projectRoot } = detail;
    const summary: LedgerStatus = { sessionId,agentSessionId,title,state,statusReason,possiblyWaiting,limited,lastActiveAt,turnId,acceptances,claims,progress,offPlan,parentSessionId,projectRoot,turns: detail.turns.filter(t => t.id === turnId) };
    writeLedgerComputation(`scan:${session.sessionId}`,transcript,{ path: parsed.sourcePath,transcript,signature,summary,transcriptLimited: derived.facts.some(f => f.kind === 'limit'),statusKey: statusKey(session,currentGoalStatus(session)),lastTurn: [...derived.facts].reverse().find(f => f.kind === 'turn') });
  }
  return detail;
}
export function suggestAttribution(session: Session, ask: string) {
  return workItemRepository.findAll().filter(t => t.level !== 'goal' && t.kind === 'todo' && !['done','archived'].includes(t.status)).map(todo => {
    const reasons: string[] = []; let score = 0;
    if (todo.projectRoot === session.directory) { score += 5; reasons.push('同一项目'); }
    if (ask.toLowerCase().includes(todo.title.toLowerCase()) || ask.includes(todo.id)) { score += 6; reasons.push('原话提到这条待办'); }
    const words = anchorsFromText(`${todo.title} ${(todo.acceptance ?? []).map(c => c.text).join(' ')}`).keywords;
    const hits = words.filter(w => ask.toLowerCase().includes(w));
    if (hits.length) { score += Math.min(3,hits.length); reasons.push(`匹配关键词：${hits.slice(0,4).join(', ')}`); }
    if (todo.status === 'active') { score++; reasons.push('待办正在进行'); }
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
  const legacy = items.filter(item => item.source === 'user' && item.anchors.legacyCommands?.length);
  const rules = [...ledgerRepository.rules(agent.id),...legacy.map(item => ({
    itemId: item.id, matcher: { paths: [],commands: item.anchors.legacyCommands! },
  }))];
  // Unversioned saved user patterns still attribute steps, but need a literal
  // command decision before they can participate in automatic completion.
  const pending = new Map(items.filter(item => item.source === 'user' && item.anchors.commandFormat === 'legacy-unconfirmed').map(item => [item.id,item.anchors]));
  const matchingItems = items.map(item => pending.has(item.id) ? { ...item,anchors: { ...item.anchors,commands: [] } } : item);
  const matched = matchLedger(facts, matchingItems, ledgerRepository.corrections(agent.id), rules, cfg, agent.id);
  for (const item of matched.items) if (pending.has(item.id)) item.anchors = pending.get(item.id)!;
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
    available: facts.length > 0 && unknownRecords < Math.max(10,facts.length), unavailableReason: facts.length ? unknownRecords >= Math.max(10,facts.length) ? '无法识别记录格式，仅显示会话状态' : undefined : '暂时无法读取执行记录',
    asks: ledgerRepository.asks(agent.id), ...matched, turns, claims, acceptances, turnId, workItemId: link?.workItemId,
    lastActiveAt: session.lastActiveAt.toISOString(), attribution, limited: !!goalStatus || facts.some(f => f.kind === 'limit'),
    followUpSuggestions: followUpSuggestions(facts),
    activity: { ...latestActivity(facts),evidence: matched.evidence.filter(e => e.kind !== 'command' && e.kind !== 'verdict').slice(-3).reverse() },
  };
  refreshImportSuggestions(detail,session);
  if (goalStatus) detail.statusReason = `Codex 目标${goalStatus === 'blocked' ? '受阻' : goalStatus === 'budget_limited' ? '预算用尽' : '额度用尽'}`;
  if (acceptance?.decision === 'accepted_with_gaps') for (const item of detail.items) item.dropped = acceptance.droppedItemIds.includes(item.id);
  refreshStatus(detail,session,goalStatus,detail.limited ?? false);
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
  // Scanner parsers also load usage/pricing. The resident service only needs
  // them on this uncached lookup; keep that app-only graph out of startup.
  const parsed = session.client === 'codex'
    ? await (await import('../../adapters/codex/scanner.js')).getCodexSessionById(sessionId,false)
    : await (await import('../../adapters/claude/scanner.js')).getSessionById(sessionId,false);
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
  const all = sessionRepository.findAll().filter(ledgerEnabled);
  const byId = new Map(all.map(session => [session.sessionId,session]));
  const sessions = all.filter(s => s.lastActiveAt.getTime() >= Date.now() - hours * 3600000);
  const details = new Map<string,LedgerDetail>();
  for (const session of sessions) { const detail = await getLedger(session.sessionId); if (detail) details.set(detail.sessionId,detail); }
  const roots = new Map<string,LedgerDetail>();
  for (const child of [...details.values()]) {
    let root = child;
    const seen = new Set([child.sessionId]);
    while (root.parentSessionId && !seen.has(root.parentSessionId)) {
      seen.add(root.parentSessionId);
      const parentId = root.parentSessionId;
      let parent = details.get(parentId);
      if (!parent && byId.has(parentId)) { parent = await getLedger(parentId) ?? undefined; if (parent) details.set(parentId,parent); }
      if (!parent) break;
      root = parent;
    }
    roots.set(root.sessionId,root);
    if (root !== child) {
      attachSubagent(root,child);
    }
  }
  const urgency = (d: LedgerDetail) => ledgerNeedsAttention(d) ? 0 : d.state === 'running' ? 1 : 2;
  return [...roots.values()].sort((a,b) => urgency(a) - urgency(b) || Date.parse(b.lastActiveAt) - Date.parse(a.lastActiveAt));
}
function attachSubagent(parent: LedgerDetail,child: LedgerDetail) {
  parent.subagents ??= [];
  if (Date.parse(child.activity?.at ?? child.lastActiveAt) > Date.parse(parent.activity?.at ?? parent.lastActiveAt)) parent.activity = child.activity && { ...child.activity,action: `子任务 ${child.title}：${child.activity.action ?? '执行中'}` };
  if (Date.parse(child.lastActiveAt) > Date.parse(parent.lastActiveAt)) parent.lastActiveAt = child.lastActiveAt;
  parent.subagents.push({ sessionId: child.sessionId,title: child.title,state: child.state,lastActiveAt: child.lastActiveAt,activity: child.activity });
  if (child.state === 'needs_input') { parent.state = 'needs_input'; parent.statusReason = '子任务正在等待审批'; }
  else if (child.state === 'running' && ['ended','stopped','review'].includes(parent.state)) parent.state = 'running';
}
export async function ledgerDetail(sessionId: string) {
  const detail = await getLedger(sessionId);
  if (!detail) return null;
  const all = sessionRepository.findAll().filter(ledgerEnabled);
  const byId = new Map(all.map(session => [session.sessionId,session]));
  for (const session of all) {
    let parentId = session.parentSessionId;
    const seen = new Set([session.sessionId]);
    while (parentId) {
      if (session.client === 'codex' && !parentId.startsWith('codex_')) parentId = `codex_${parentId}`;
      if (seen.has(parentId)) break;
      if (parentId === sessionId) { const child = await getLedger(session.sessionId); if (child) attachSubagent(detail,child); break; }
      seen.add(parentId);parentId = byId.get(parentId)?.parentSessionId;
    }
  }
  return detail;
}
function latestActivity(facts: TranscriptFact[]): { action?: string; at?: string; lastMessage?: string } {
  let tool: Extract<TranscriptFact,{kind:'tool'}> | undefined;
  let message: Extract<TranscriptFact,{kind:'agent_message'}> | undefined;
  for (const fact of facts) {
    if (fact.kind === 'tool') tool = fact;
    if (fact.kind === 'agent_message') message = fact;
  }
  const input = tool?.input && typeof tool.input === 'object' ? tool.input as Record<string,unknown> : {};
  return { action: tool ? `${tool.name} ${String(input.command ?? input.cmd ?? input.file_path ?? input.path ?? '').slice(0,300)}`.trim() : undefined,
    at: tool?.at,lastMessage: message?.text.slice(0,600) };
}
export function followUpPrompt(detail: LedgerDetail, correctionRun?: string): string {
  if (correctionRun) {
    const run = detail.offPlan.find(r => r.id === correctionRun);
    if (!run) throw new LedgerInputError('Off-plan run not found');
    const steps = detail.trail.filter(s => run.callIds.includes(s.callId));
    return `请回到原定任务范围。\n\n原始要求：\n${detail.asks.map(a => a.text).join('\n\n')}\n\n请核对这些步骤：\n${steps.map(s => `${s.summary}\n${s.violations.join('\n')}`).join('\n')}\n\n说明这些步骤为什么必要，或纠正偏离。请提供可核验的证据。`;
  }
  const remaining = detail.items.filter(i => confirmedRequirement(i) && (i.status !== 'done' || !i.evidenceIds.length));
  return `请完成以下已确认要求，或补充证据：\n${remaining.map(i => `- ${i.title}${i.dropped ? '（先前已放弃）' : ''}`).join('\n')}\n\n需要补充证据的汇报：\n${detail.claims.filter(c => !c.evidenceIds.length).map(c => `- ${c.text}`).join('\n')}\n\n约束：\n${detail.items.flatMap(i => i.constraints.map(c => `- ${c.kind}: ${c.value ?? ''}`)).join('\n')}\n\n汇报实际执行的检查及结果；没有证据时不要宣称完成。`;
}
export async function acceptLedger(sessionId: string, input: Omit<LedgerAcceptance, 'turnId' | 'at'>) {
  const detail = await getLedger(sessionId);
  if (!detail || !detail.turnId || (!['review','accepted','ended'].includes(detail.state) || detail.progress.total === 0)) throw new LedgerInputError('Only a finished turn can be accepted');
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
  const remaining = detail.items.filter(i => confirmedRequirement(i) && (i.status !== 'done' || !i.evidenceIds.length));
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
  const items = previous.items.filter(i => !i.dropped).map((i,ordinal) => ({ ...i,id: randomUUID(),ordinal,source: 'user' as const,anchors: { ...i.anchors,commandFormat: i.anchors.commandFormat ?? 'literal-v2' as const },status: 'todo' as const,statusSource: 'rule' as const,evidenceIds: [],checklistId: undefined }));
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
    const pending = old?.source === 'user' && old.anchors.commandFormat === 'legacy-unconfirmed';
    const confirmed = pending && value.anchors.commandFormat === 'literal-v2';
    if (confirmed && !value.anchors.commands.some(command => command.trim())) throw new LedgerInputError('Enter a complete command before confirming legacy criteria');
    const anchors = {
      ...value.anchors,
      commandFormat: pending && !confirmed ? 'legacy-unconfirmed' as const : 'literal-v2' as const,
      // Original patterns are server-owned history. Omitting them in an old
      // client or editing their serialized copy cannot erase their attribution.
      legacyCommands: old?.source === 'user' ? old.anchors.legacyCommands : undefined,
      ...(pending && !confirmed ? { commands: old.anchors.commands } : {}),
    };
    return { ...value,anchors,id,ordinal,source: 'user' as const, statusSource: value.status !== old?.status ? 'user' as const : old?.statusSource ?? 'rule' as const, evidenceIds: (value.evidenceIds ?? []).filter(id => detail.evidence.some(e => e.id === id)), checklistId: old?.checklistId };
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
