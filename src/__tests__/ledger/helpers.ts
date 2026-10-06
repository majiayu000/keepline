import { beforeEach, afterEach } from 'bun:test';
import { resetDatabase } from '../../db/migrations.js';
import { closeDatabase } from '../../infrastructure/database/sqlite.js';
import { config } from '../../lib/config.js';
import { DEFAULT_LEDGER_CONFIG, type LedgerDetail, type TranscriptFact } from '../../domain/ledger/types.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { clearLedgerCache, ingestLedger } from '../../services/ledger/service.js';

export function setupLedgerTest() {
  beforeEach(() => { resetDatabase(); clearLedgerCache(); config.set('ledger',{ ...structuredClone(DEFAULT_LEDGER_CONFIG), nativeNotifications: false }); });
  afterEach(() => { clearLedgerCache(); closeDatabase(); });
}
export function sampleFacts(): TranscriptFact[] {
  const at = new Date().toISOString();
  return [
    { kind: 'turn', phase: 'started', turnId: 'turn-1', at },
    { kind: 'user_message',text: 'Run `bun test src/widget.test.ts` and fix src/widget.ts',turnId: 'turn-1',at },
    { kind: 'tool',callId: 'call-1',name: 'Bash',input: { command: 'bun test src/widget.test.ts' },at,turnId: 'turn-1',mutating: true,exitCode: 0,facts: [{ kind: 'command',value: 'bun test src/widget.test.ts',exitCode: 0 },{ kind: 'test',value: '3 pass',exitCode: 0 }] },
    { kind: 'agent_message',text: 'Completed: bun test src/widget.test.ts passed',final: true,turnId: 'turn-1',at },
    { kind: 'turn',phase: 'completed',turnId: 'turn-1',at },
  ];
}
export async function seededLedger(facts = sampleFacts(), sessionId = 'ledger-session-123') {
  const session = sessionRepository.upsert({ sessionId,client: 'claude',directory: '/project',title: 'Widget tests',initialPrompt: 'Fix widget',lastActiveAt: new Date(),status: 'idle' });
  return (await ingestLedger(session,{ sessionId,directory: '/project',lastActiveAt: new Date(),messageCount: 2,toolCount: 1 },facts))!;
}
export function blankDetail(): LedgerDetail {
  return { sessionId: 'session-123',agentSessionId: 'agent-123',title: 'Test',projectRoot: '/project',runtimeId: 'codex',state: 'running',possiblyWaiting: false,available: true,asks: [],items: [],evidence: [],trail: [],readOnlyCount: 0,turns: [],offPlan: [],claims: [],acceptances: [],progress: { done: 0,total: 0 },lastActiveAt: new Date().toISOString() };
}
